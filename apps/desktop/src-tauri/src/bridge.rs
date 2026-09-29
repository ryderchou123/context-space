use crate::{
    db::{ACTIVE_WORKSPACE, DUPLICATE_URL, Database},
    events::EventLog,
    models::{BridgeCommand, SessionTab},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
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

pub struct BridgeState {
    queue: Mutex<VecDeque<(Instant, BridgeCommand)>>,
    queue_ready: Condvar,
    pending: Mutex<HashMap<String, SyncSender<Vec<SessionTab>>>>,
    last_seen: Mutex<Option<Instant>>,
    was_connected: AtomicBool,
    events: Arc<EventLog>,
}
impl BridgeState {
    pub fn new(events: Arc<EventLog>) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            queue_ready: Condvar::new(),
            pending: Mutex::new(HashMap::new()),
            last_seen: Mutex::new(None),
            was_connected: AtomicBool::new(false),
            events,
        }
    }
    pub fn push(&self, cmd: BridgeCommand) {
        if let Ok(mut q) = self.queue.lock() {
            q.push_back((Instant::now(), cmd));
            self.queue_ready.notify_one();
        }
    }
    fn poll(&self, timeout: Duration) -> Option<BridgeCommand> {
        let deadline = Instant::now() + timeout;
        let mut queue = self.queue.lock().ok()?;
        loop {
            while let Some((queued_at, cmd)) = queue.pop_front() {
                if queued_at.elapsed() <= COMMAND_TTL {
                    return Some(cmd);
                }
                self.events
                    .record("BRIDGE_COMMAND_EXPIRED", format!("type={}", cmd.kind()));
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            queue = self.queue_ready.wait_timeout(queue, remaining).ok()?.0;
            if queue.is_empty() && Instant::now() >= deadline {
                return None;
            }
        }
    }
    fn mark_seen(&self) {
        if let Ok(mut seen) = self.last_seen.lock() {
            *seen = Some(Instant::now())
        }
        if !self.was_connected.swap(true, Ordering::SeqCst) {
            self.events.record("EXTENSION_CONNECTED", "");
        }
    }
    pub fn connected(&self) -> bool {
        let connected = self
            .last_seen
            .lock()
            .ok()
            .and_then(|v| *v)
            .is_some_and(|t| t.elapsed() < CONNECTED_WINDOW);
        if !connected && self.was_connected.swap(false, Ordering::SeqCst) {
            self.events.record("EXTENSION_DISCONNECTED", "");
        }
        connected
    }
    pub fn last_seen_secs(&self) -> Option<u64> {
        self.last_seen
            .lock()
            .ok()
            .and_then(|v| *v)
            .map(|t| t.elapsed().as_secs())
    }
    pub fn wait_for_capture(
        &self,
        workspace_id: &str,
        timeout: Duration,
    ) -> Option<Vec<SessionTab>> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pending.lock().ok()?.insert(request_id.clone(), tx);
        self.push(BridgeCommand::CaptureTabs {
            request_id: request_id.clone(),
            workspace_id: workspace_id.into(),
        });
        let result = rx.recv_timeout(timeout).ok();
        self.pending.lock().ok()?.remove(&request_id);
        result
    }
    #[cfg(test)]
    pub fn mark_seen_for_tests(&self) {
        self.mark_seen()
    }
    #[cfg(test)]
    pub fn queue_len_for_tests(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
    /// Hands captured tabs to a waiting switch. Returns false when nobody is waiting.
    fn deliver_capture(&self, request_id: &str, tabs: Vec<SessionTab>) -> bool {
        let sender = self
            .pending
            .lock()
            .ok()
            .and_then(|mut map| map.remove(request_id));
        sender.is_some_and(|tx| tx.send(tabs).is_ok())
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
    bridge.mark_seen();
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
            let command = bridge.poll(Duration::from_secs(25));
            respond(
                request,
                200,
                &serde_json::to_string(&command).unwrap_or_else(|_| "null".into()),
            )
        }
        (Method::Post, "/api/capture") => match read_json::<CapturePayload>(&mut request) {
            Ok(p) => {
                let count = p.tabs.len();
                if bridge.deliver_capture(&p.request_id, p.tabs.clone()) {
                    return respond(request, 200, r#"{"ok":true}"#);
                }
                // The switch already timed out; keep the late capture, but never let an
                // empty one wipe the previous session.
                match db.save_captured_session(&p.workspace_id, &p.tabs) {
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
            "Content-Type, X-Context-Space-Token",
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
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Context-Space-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
        let producer = bridge.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            producer.push(BridgeCommand::OpenUrls {
                workspace_id: "work".into(),
                urls: vec!["https://example.com".into()],
            });
        });
        let started = Instant::now();
        let command = bridge.poll(Duration::from_secs(1));
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(matches!(command, Some(BridgeCommand::OpenUrls { .. })));
    }

    #[test]
    fn empty_long_poll_times_out_cleanly() {
        assert!(bridge().poll(Duration::from_millis(5)).is_none());
    }

    #[test]
    fn regression_bug_004_expired_commands_are_never_delivered() {
        let bridge = bridge();
        bridge.queue.lock().unwrap().push_back((
            Instant::now() - COMMAND_TTL - Duration::from_secs(1),
            BridgeCommand::CloseUrls {
                workspace_id: "work".into(),
                urls: vec!["https://linkedin.com".into()],
            },
        ));
        assert!(bridge.poll(Duration::from_millis(10)).is_none());
        assert_eq!(bridge.events.names(), vec!["BRIDGE_COMMAND_EXPIRED"]);
    }

    #[test]
    fn connection_state_transitions_are_logged_once() {
        let bridge = bridge();
        assert!(!bridge.connected());
        bridge.mark_seen();
        bridge.mark_seen();
        assert!(bridge.connected());
        *bridge.last_seen.lock().unwrap() = Some(Instant::now() - CONNECTED_WINDOW);
        assert!(!bridge.connected());
        assert!(!bridge.connected());
        assert_eq!(
            bridge.events.names(),
            vec!["EXTENSION_CONNECTED", "EXTENSION_DISCONNECTED"]
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
    }

    #[test]
    fn http_capture_round_trip_reaches_the_waiting_switch() {
        let db = Arc::new(Database::memory());
        let work = workspace(&db, "Work");
        let bridge = bridge();
        let port = start_on(0, db.clone(), bridge.clone()).unwrap();
        let token = db.state("bridge_token").unwrap().unwrap();

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
