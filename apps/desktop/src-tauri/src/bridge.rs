use crate::{
    db::{ACTIVE_WORKSPACE, DUPLICATE_URL, Database},
    events::EventLog,
    models::{BridgeCommand, SessionTab},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    io::Read,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::{Duration, Instant},
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub const BRIDGE_PORT: u16 = 47651;
/// The extension long-polls every few seconds; anything older than this was queued for a
/// browser that went away and must not run later (e.g. closing tabs hours after a switch).
const COMMAND_TTL: Duration = Duration::from_secs(20);
const CONNECTED_WINDOW: Duration = Duration::from_secs(45);
pub const CAPTURE_TIMEOUT: Duration = Duration::from_millis(2600);

struct ClientState {
    queue: VecDeque<(Instant, BridgeCommand)>,
    last_seen: Instant,
}

struct PendingCapture {
    expected_clients: HashSet<String>,
    sender: Sender<(String, Vec<SessionTab>)>,
}

pub struct BridgeState {
    clients: Mutex<HashMap<String, ClientState>>,
    queue_ready: Condvar,
    pending: Mutex<HashMap<String, PendingCapture>>,
    was_connected: AtomicBool,
    events: Arc<EventLog>,
}
impl BridgeState {
    pub fn new(events: Arc<EventLog>) -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            queue_ready: Condvar::new(),
            pending: Mutex::new(HashMap::new()),
            was_connected: AtomicBool::new(false),
            events,
        }
    }
    pub fn push(&self, cmd: BridgeCommand) {
        let clients = self.active_client_ids();
        self.push_to_clients(&clients, cmd);
    }
    fn push_to_clients(&self, client_ids: &HashSet<String>, cmd: BridgeCommand) {
        if let Ok(mut clients) = self.clients.lock() {
            let now = Instant::now();
            for client_id in client_ids {
                if let Some(client) = clients.get_mut(client_id) {
                    client.queue.push_back((now, cmd.clone()));
                }
            }
            self.queue_ready.notify_all();
        }
    }
    fn poll(&self, client_id: &str, timeout: Duration) -> Option<BridgeCommand> {
        let deadline = Instant::now() + timeout;
        let mut clients = self.clients.lock().ok()?;
        loop {
            while let Some((queued_at, cmd)) = clients.get_mut(client_id)?.queue.pop_front() {
                if queued_at.elapsed() <= COMMAND_TTL {
                    return Some(cmd);
                }
                self.events.record(
                    "BRIDGE_COMMAND_EXPIRED",
                    format!("type={} client={client_id}", cmd.kind()),
                );
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            clients = self.queue_ready.wait_timeout(clients, remaining).ok()?.0;
            if clients.get(client_id)?.queue.is_empty() && Instant::now() >= deadline {
                return None;
            }
        }
    }
    fn mark_seen(&self, client_id: &str) {
        if let Ok(mut clients) = self.clients.lock() {
            clients
                .entry(client_id.into())
                .and_modify(|client| client.last_seen = Instant::now())
                .or_insert_with(|| ClientState {
                    queue: VecDeque::new(),
                    last_seen: Instant::now(),
                });
        }
        if !self.was_connected.swap(true, Ordering::SeqCst) {
            self.events.record("EXTENSION_CONNECTED", "");
        }
    }
    fn active_client_ids(&self) -> HashSet<String> {
        self.clients
            .lock()
            .map(|clients| {
                clients
                    .iter()
                    .filter(|(_, client)| client.last_seen.elapsed() < CONNECTED_WINDOW)
                    .map(|(id, _)| id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn connected(&self) -> bool {
        let connected = !self.active_client_ids().is_empty();
        if !connected && self.was_connected.swap(false, Ordering::SeqCst) {
            self.events.record("EXTENSION_DISCONNECTED", "");
        }
        connected
    }
    pub fn client_count(&self) -> usize {
        self.active_client_ids().len()
    }
    pub fn last_seen_secs(&self) -> Option<u64> {
        self.clients.lock().ok().and_then(|clients| {
            clients
                .values()
                .map(|client| client.last_seen.elapsed().as_secs())
                .min()
        })
    }
    pub fn wait_for_capture(
        &self,
        workspace_id: &str,
        timeout: Duration,
    ) -> Option<Vec<SessionTab>> {
        let expected_clients = self.active_client_ids();
        if expected_clients.is_empty() {
            return None;
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending.lock().ok()?.insert(
            request_id.clone(),
            PendingCapture {
                expected_clients: expected_clients.clone(),
                sender: tx,
            },
        );
        self.push_to_clients(
            &expected_clients,
            BridgeCommand::CaptureTabs {
                request_id: request_id.clone(),
                workspace_id: workspace_id.into(),
            },
        );
        let deadline = Instant::now() + timeout;
        let mut responses = BTreeMap::new();
        while responses.len() < expected_clients.len() {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            match rx.recv_timeout(remaining) {
                Ok((client_id, tabs)) if expected_clients.contains(&client_id) => {
                    responses.entry(client_id).or_insert(tabs);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        self.pending.lock().ok()?.remove(&request_id);
        if responses.is_empty() {
            return None;
        }
        let mut seen = HashSet::new();
        Some(
            responses
                .into_values()
                .flatten()
                .filter(|tab| seen.insert(crate::db::url_key(&tab.url)))
                .collect(),
        )
    }
    #[cfg(test)]
    pub fn mark_seen_for_tests(&self) {
        self.mark_seen("test")
    }
    #[cfg(test)]
    pub fn queue_len_for_tests(&self) -> usize {
        self.clients
            .lock()
            .unwrap()
            .values()
            .map(|client| client.queue.len())
            .sum()
    }
    /// Hands captured tabs to a waiting switch. Returns false when nobody is waiting.
    fn deliver_capture(&self, client_id: &str, request_id: &str, tabs: Vec<SessionTab>) -> bool {
        let sender = self.pending.lock().ok().and_then(|map| {
            map.get(request_id).and_then(|pending| {
                pending
                    .expected_clients
                    .contains(client_id)
                    .then(|| pending.sender.clone())
            })
        });
        sender.is_some_and(|tx| tx.send((client_id.into(), tabs)).is_ok())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapturePayload {
    request_id: String,
    workspace_id: String,
    tabs: Vec<SessionTab>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddPayload {
    workspace_id: String,
    tabs: Vec<SessionTab>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    active_workspace_id: Option<String>,
    active_workspace_name: Option<String>,
    workspaces: Vec<WorkspaceItem>,
}
#[derive(Serialize)]
struct WorkspaceItem {
    id: String,
    name: String,
}

pub fn start(db: Arc<Database>, bridge: Arc<BridgeState>) {
    if start_on(BRIDGE_PORT, db, bridge.clone()).is_none() {
        bridge.events.record(
            "BRIDGE_UNAVAILABLE",
            format!("port={BRIDGE_PORT} is already in use"),
        );
    }
}

/// Binds the bridge (port 0 picks a free port for tests) and returns the bound port.
pub fn start_on(port: u16, db: Arc<Database>, bridge: Arc<BridgeState>) -> Option<u16> {
    let server = Server::http(("127.0.0.1", port)).ok()?;
    let bound = server.server_addr().to_ip()?.port();
    thread::spawn(move || {
        for request in server.incoming_requests() {
            let db = db.clone();
            let bridge = bridge.clone();
            thread::spawn(move || handle(request, &db, &bridge));
        }
    });
    Some(bound)
}

fn token_matches(supplied: &str, expected: &str) -> bool {
    // An unreadable token must never turn into "no token required".
    !expected.is_empty()
        && supplied.len() == expected.len()
        && supplied
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn client_id(request: &Request) -> Result<String, &'static str> {
    let value = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Context-Space-Client"))
        .map(|header| header.value.as_str().trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("legacy");
    if value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        Ok(value.into())
    } else {
        Err("Invalid bridge client id.")
    }
}

fn handle(mut request: Request, db: &Arc<Database>, bridge: &Arc<BridgeState>) {
    if request.method() == &Method::Options {
        return respond(request, 200, "{}");
    }
    let expected = db.state("bridge_token").ok().flatten().unwrap_or_default();
    let supplied = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("X-Context-Space-Token"))
        .map(|h| h.value.as_str())
        .unwrap_or("");
    if !token_matches(supplied, &expected) {
        return respond(request, 401, r#"{"error":"Invalid bridge token."}"#);
    }
    let client_id = match client_id(&request) {
        Ok(value) => value,
        Err(error) => return respond_error(request, 400, error),
    };
    bridge.mark_seen(&client_id);
    let path = request.url().split('?').next().unwrap_or(request.url());
    match (request.method().clone(), path) {
        (Method::Get, "/api/status") => {
            let all = db.list_workspaces().unwrap_or_default();
            let active = db.state(ACTIVE_WORKSPACE).ok().flatten();
            let name = active
                .as_ref()
                .and_then(|id| all.iter().find(|w| &w.id == id).map(|w| w.name.clone()));
            let body = serde_json::to_string(&StatusPayload {
                active_workspace_id: active,
                active_workspace_name: name,
                workspaces: all
                    .into_iter()
                    .map(|w| WorkspaceItem {
                        id: w.id,
                        name: w.name,
                    })
                    .collect(),
            })
            .unwrap_or_else(|_| "{}".into());
            respond(request, 200, &body)
        }
        (Method::Get, "/api/poll") => {
            let command = bridge.poll(&client_id, Duration::from_secs(25));
            respond(
                request,
                200,
                &serde_json::to_string(&command).unwrap_or_else(|_| "null".into()),
            )
        }
        (Method::Post, "/api/capture") => match read_json::<CapturePayload>(&mut request) {
            Ok(p) => {
                let count = p.tabs.len();
                if bridge.deliver_capture(&client_id, &p.request_id, p.tabs.clone()) {
                    return respond(request, 200, r#"{"ok":true}"#);
                }
                // The switch already timed out. Merge a late browser response with the
                // session already saved by other clients; never let an empty response wipe it.
                if !p
                    .tabs
                    .iter()
                    .any(|tab| crate::db::parse_web_url(&tab.url).is_some())
                {
                    return respond(request, 200, r#"{"ok":true}"#);
                }
                let mut merged = db
                    .workspace(&p.workspace_id)
                    .map(|workspace| workspace.last_session)
                    .unwrap_or_default();
                merged.extend(p.tabs);
                match db.save_captured_session(&p.workspace_id, &merged) {
                    Ok(saved) => {
                        if saved {
                            bridge.events.record(
                                "SESSION_SAVE",
                                format!("workspace_id={} tabs={count} late=true", p.workspace_id),
                            );
                        }
                        respond(request, 200, r#"{"ok":true}"#)
                    }
                    Err(e) => respond_error(request, 400, &e),
                }
            }
            Err(e) => respond_error(request, 400, &e),
        },
        (Method::Post, "/api/add-tabs") => match read_json::<AddPayload>(&mut request) {
            Ok(p) => {
                let mut added = 0usize;
                let mut duplicates = 0usize;
                for tab in p.tabs {
                    match db.add_browser(&p.workspace_id, &tab.title, &tab.url, false) {
                        Ok(()) => added += 1,
                        Err(e) if e == DUPLICATE_URL => duplicates += 1,
                        Err(e) => return respond_error(request, 400, &e),
                    }
                }
                respond(
                    request,
                    200,
                    &format!(r#"{{"ok":true,"added":{added},"duplicates":{duplicates}}}"#),
                )
            }
            Err(e) => respond_error(request, 400, &e),
        },
        _ => respond(request, 404, r#"{"error":"Not found."}"#),
    }
}
fn read_json<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T, String> {
    let mut body = String::new();
    request
        .as_reader()
        .take(1_000_000)
        .read_to_string(&mut body)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}
fn respond(request: Request, status: u16, body: &str) {
    let mut response = Response::from_string(body).with_status_code(StatusCode(status));
    for h in [
        ("Content-Type", "application/json"),
        ("Access-Control-Allow-Origin", "*"),
        (
            "Access-Control-Allow-Headers",
            "Content-Type, X-Context-Space-Token, X-Context-Space-Client",
        ),
        ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
    ] {
        if let Ok(header) = Header::from_bytes(h.0, h.1) {
            response.add_header(header);
        }
    }
    let _ = request.respond(response);
}

fn respond_error(request: Request, status: u16, error: &str) {
    respond(
        request,
        status,
        &format!(
            r#"{{"error":{}}}"#,
            serde_json::to_string(error).unwrap_or_else(|_| "\"error\"".into())
        ),
    );
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::models::WorkspaceDraft;
    use std::{io::Write, net::TcpStream};

    pub fn bridge() -> Arc<BridgeState> {
        Arc::new(BridgeState::new(Arc::new(EventLog::new())))
    }

    /// Minimal HTTP/1.1 client so the real HTTP boundary is tested, not just the structs.
    pub fn request(port: u16, method: &str, path: &str, token: &str, body: &str) -> (u16, String) {
        request_as(port, method, path, token, None, body)
    }

    fn request_as(
        port: u16,
        method: &str,
        path: &str,
        token: &str,
        client_id: Option<&str>,
        body: &str,
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let client_header = client_id
            .map(|id| format!("X-Context-Space-Client: {id}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Context-Space-Token: {token}\r\n{client_header}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let status = raw[9..12].parse().unwrap();
        let body = raw
            .split_once("\r\n\r\n")
            .map(|x| x.1)
            .unwrap_or("")
            .to_string();
        (status, body)
    }

    fn workspace(db: &Database, name: &str) -> String {
        db.save_workspace(WorkspaceDraft {
            id: None,
            name: name.into(),
            icon: "briefcase".into(),
            color: "#000".into(),
            browser_exit_behavior: "close".into(),
            app_exit_behavior: "keep".into(),
            save_session_on_exit: true,
            restore_session_on_launch: true,
        })
        .unwrap()
    }

    #[test]
    fn queued_command_wakes_long_poll_without_delay() {
        let bridge = bridge();
        bridge.mark_seen("legacy");
        let producer = bridge.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            producer.push(BridgeCommand::OpenUrls {
                workspace_id: "work".into(),
                urls: vec!["https://example.com".into()],
            });
        });
        let started = Instant::now();
        let command = bridge.poll("legacy", Duration::from_secs(1));
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(matches!(command, Some(BridgeCommand::OpenUrls { .. })));
    }

    #[test]
    fn empty_long_poll_times_out_cleanly() {
        let bridge = bridge();
        bridge.mark_seen("legacy");
        assert!(bridge.poll("legacy", Duration::from_millis(5)).is_none());
    }

    #[test]
    fn regression_bug_004_expired_commands_are_never_delivered() {
        let bridge = bridge();
        bridge.mark_seen("legacy");
        bridge
            .clients
            .lock()
            .unwrap()
            .get_mut("legacy")
            .unwrap()
            .queue
            .push_back((
                Instant::now() - COMMAND_TTL - Duration::from_secs(1),
                BridgeCommand::CloseUrls {
                    workspace_id: "work".into(),
                    urls: vec!["https://linkedin.com".into()],
                },
            ));
        assert!(bridge.poll("legacy", Duration::from_millis(10)).is_none());
        assert_eq!(
            bridge.events.names(),
            vec!["EXTENSION_CONNECTED", "BRIDGE_COMMAND_EXPIRED"]
        );
    }

    #[test]
    fn connection_state_transitions_are_logged_once() {
        let bridge = bridge();
        assert!(!bridge.connected());
        bridge.mark_seen("legacy");
        bridge.mark_seen("legacy");
        assert!(bridge.connected());
        bridge
            .clients
            .lock()
            .unwrap()
            .get_mut("legacy")
            .unwrap()
            .last_seen = Instant::now() - CONNECTED_WINDOW;
        assert!(!bridge.connected());
        assert!(!bridge.connected());
        assert_eq!(
            bridge.events.names(),
            vec!["EXTENSION_CONNECTED", "EXTENSION_DISCONNECTED"]
        );
    }

    #[test]
    fn regression_bug_028_each_browser_client_receives_every_command() {
        let bridge = bridge();
        bridge.mark_seen("chrome");
        bridge.mark_seen("edge");
        bridge.push(BridgeCommand::OpenUrls {
            workspace_id: "work".into(),
            urls: vec!["https://example.com".into()],
        });

        assert!(matches!(
            bridge.poll("chrome", Duration::from_millis(10)),
            Some(BridgeCommand::OpenUrls { .. })
        ));
        assert!(matches!(
            bridge.poll("edge", Duration::from_millis(10)),
            Some(BridgeCommand::OpenUrls { .. })
        ));
    }

    #[test]
    fn regression_bug_028_http_bridge_routes_commands_by_client_header() {
        let db = Arc::new(Database::memory());
        let bridge = bridge();
        let port = start_on(0, db.clone(), bridge.clone()).unwrap();
        let token = db.state("bridge_token").unwrap().unwrap();
        assert_eq!(
            request_as(port, "GET", "/api/status", &token, Some("chrome"), "").0,
            200
        );
        assert_eq!(
            request_as(port, "GET", "/api/status", &token, Some("edge"), "").0,
            200
        );
        assert_eq!(bridge.client_count(), 2);

        bridge.push(BridgeCommand::CloseUrls {
            workspace_id: "work".into(),
            urls: vec!["https://example.com".into()],
        });
        for client in ["chrome", "edge"] {
            let (_, body) = request_as(port, "GET", "/api/poll", &token, Some(client), "");
            let command: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(command["type"], "close_urls");
        }
    }

    #[test]
    fn regression_bug_028_capture_merges_responses_from_chrome_and_edge() {
        let bridge = bridge();
        bridge.mark_seen("chrome");
        bridge.mark_seen("edge");
        let waiter = {
            let bridge = bridge.clone();
            thread::spawn(move || bridge.wait_for_capture("work", Duration::from_secs(1)))
        };

        let chrome_command = bridge.poll("chrome", Duration::from_secs(1)).unwrap();
        let edge_command = bridge.poll("edge", Duration::from_secs(1)).unwrap();
        let request_id = match chrome_command {
            BridgeCommand::CaptureTabs { request_id, .. } => request_id,
            _ => panic!("expected capture command"),
        };
        assert!(matches!(
            edge_command,
            BridgeCommand::CaptureTabs {
                request_id: ref edge_request,
                ..
            } if edge_request == &request_id
        ));
        assert!(bridge.deliver_capture(
            "chrome",
            &request_id,
            vec![SessionTab {
                title: "A".into(),
                url: "https://a.test".into(),
            }],
        ));
        assert!(bridge.deliver_capture(
            "edge",
            &request_id,
            vec![
                SessionTab {
                    title: "A duplicate".into(),
                    url: "https://a.test/".into(),
                },
                SessionTab {
                    title: "B".into(),
                    url: "https://b.test".into(),
                },
            ],
        ));

        let tabs = waiter.join().unwrap().unwrap();
        assert_eq!(
            tabs.iter().map(|tab| tab.url.as_str()).collect::<Vec<_>>(),
            vec!["https://a.test", "https://b.test"]
        );
    }

    #[test]
    fn regression_bug_029_empty_or_wrong_tokens_are_rejected() {
        assert!(!token_matches("", ""));
        assert!(!token_matches("abc", "abd"));
        assert!(!token_matches("ab", "abc"));
        assert!(token_matches("abc", "abc"));
    }

    #[test]
    fn http_bridge_requires_token_and_serves_status() {
        let db = Arc::new(Database::memory());
        let work = workspace(&db, "Work");
        db.set_state(ACTIVE_WORKSPACE, Some(&work)).unwrap();
        let bridge = bridge();
        let port = start_on(0, db.clone(), bridge.clone()).unwrap();
        let token = db.state("bridge_token").unwrap().unwrap();

        let (status, _) = request(port, "GET", "/api/status", "wrong", "");
        assert_eq!(status, 401);
        assert!(
            !bridge.connected(),
            "a bad token must not count as connected"
        );

        let (status, body) = request(port, "GET", "/api/status", &token, "");
        assert_eq!(status, 200);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["activeWorkspaceName"], "Work");
        assert!(bridge.connected());

        let (status, _) = request(port, "GET", "/api/unknown", &token, "");
        assert_eq!(status, 404);
        let (status, body) = request(port, "POST", "/api/capture", &token, "{not json");
        assert_eq!(status, 400);
        assert!(body.contains("error"));

        let (status, _) = request_as(port, "GET", "/api/status", &token, Some("bad client!"), "");
        assert_eq!(status, 400);
    }

    #[test]
    fn http_capture_round_trip_reaches_the_waiting_switch() {
        let db = Arc::new(Database::memory());
        let work = workspace(&db, "Work");
        let bridge = bridge();
        let port = start_on(0, db.clone(), bridge.clone()).unwrap();
        let token = db.state("bridge_token").unwrap().unwrap();
        request(port, "GET", "/api/status", &token, "");

        let waiter = {
            let (bridge, work) = (bridge.clone(), work.clone());
            thread::spawn(move || bridge.wait_for_capture(&work, Duration::from_secs(5)))
        };
        let (_, body) = request(port, "GET", "/api/poll", &token, "");
        let command: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(command["type"], "capture_tabs");
        assert_eq!(command["workspace_id"], work.as_str());
        let payload = serde_json::json!({
            "requestId": command["request_id"],
            "workspaceId": work,
            "tabs": [{ "title": "LinkedIn", "url": "https://linkedin.com/jobs" }]
        });
        let (status, _) = request(port, "POST", "/api/capture", &token, &payload.to_string());
        assert_eq!(status, 200);
        let tabs = waiter.join().unwrap().expect("capture delivered");
        assert_eq!(tabs[0].url, "https://linkedin.com/jobs");
    }

    #[test]
    fn late_capture_is_saved_but_empty_late_capture_is_ignored() {
        let db = Arc::new(Database::memory());
        let work = workspace(&db, "Work");
        let bridge = bridge();
        let port = start_on(0, db.clone(), bridge.clone()).unwrap();
        let token = db.state("bridge_token").unwrap().unwrap();
        let late = serde_json::json!({ "requestId": "expired", "workspaceId": work,
            "tabs": [{ "title": "A", "url": "https://a.test" }] });
        assert_eq!(
            request(port, "POST", "/api/capture", &token, &late.to_string()).0,
            200
        );
        let empty = serde_json::json!({ "requestId": "expired", "workspaceId": work, "tabs": [] });
        assert_eq!(
            request(port, "POST", "/api/capture", &token, &empty.to_string()).0,
            200
        );
        assert_eq!(db.workspace(&work).unwrap().last_session.len(), 1);
        let edge = serde_json::json!({ "requestId": "expired", "workspaceId": work,
            "tabs": [{ "title": "B", "url": "https://b.test" }] });
        assert_eq!(
            request_as(
                port,
                "POST",
                "/api/capture",
                &token,
                Some("edge"),
                &edge.to_string()
            )
            .0,
            200
        );
        assert_eq!(db.workspace(&work).unwrap().last_session.len(), 2);
    }

    #[test]
    fn http_add_tabs_reports_duplicates_and_bad_urls() {
        let db = Arc::new(Database::memory());
        let work = workspace(&db, "Work");
        let port = start_on(0, db.clone(), bridge()).unwrap();
        let token = db.state("bridge_token").unwrap().unwrap();
        let body = serde_json::json!({ "workspaceId": work, "tabs": [
            { "title": "LinkedIn", "url": "https://linkedin.com/" },
            { "title": "LinkedIn again", "url": "https://linkedin.com#top" },
            { "title": "Jobs", "url": "https://linkedin.com/jobs" }
        ] });
        let (status, response) = request(port, "POST", "/api/add-tabs", &token, &body.to_string());
        assert_eq!(status, 200);
        assert!(response.contains(r#""added":2"#), "{response}");
        assert!(response.contains(r#""duplicates":1"#), "{response}");
        let missing = serde_json::json!({ "workspaceId": "deleted", "tabs": [{ "title": "A", "url": "https://a.test" }] });
        let (status, response) =
            request(port, "POST", "/api/add-tabs", &token, &missing.to_string());
        assert_eq!(status, 400);
        assert!(response.contains("no longer exists"), "{response}");
    }
}
