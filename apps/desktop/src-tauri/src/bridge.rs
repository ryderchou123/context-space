use crate::{
    db::Database,
    models::{BridgeCommand, SessionTab},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    sync::{Arc, Mutex, mpsc::SyncSender},
    thread,
    time::{Duration, Instant},
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub const BRIDGE_PORT: u16 = 47651;

pub struct BridgeState {
    queue: Mutex<VecDeque<BridgeCommand>>,
    pending: Mutex<HashMap<String, SyncSender<Vec<SessionTab>>>>,
    last_seen: Mutex<Option<Instant>>,
}
impl BridgeState {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            pending: Mutex::new(HashMap::new()),
            last_seen: Mutex::new(None),
        }
    }
    pub fn push(&self, cmd: BridgeCommand) {
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(cmd)
        }
    }
    pub fn connected(&self) -> bool {
        self.last_seen
            .lock()
            .ok()
            .and_then(|v| *v)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(12))
    }
    pub fn wait_for_capture(&self, workspace_id: &str) -> Option<Vec<SessionTab>> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pending.lock().ok()?.insert(request_id.clone(), tx);
        self.push(BridgeCommand::CaptureTabs {
            request_id: request_id.clone(),
            workspace_id: workspace_id.into(),
        });
        let result = rx.recv_timeout(Duration::from_millis(2600)).ok();
        self.pending.lock().ok()?.remove(&request_id);
        result
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
    thread::spawn(move || {
        let Ok(server) = Server::http(("127.0.0.1", BRIDGE_PORT)) else {
            log::error!("browser bridge port {BRIDGE_PORT} is unavailable");
            return;
        };
        for request in server.incoming_requests() {
            handle(request, &db, &bridge);
        }
    });
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
    if supplied != expected {
        return respond(request, 401, r#"{"error":"Invalid bridge token."}"#);
    }
    if let Ok(mut seen) = bridge.last_seen.lock() {
        *seen = Some(Instant::now())
    }
    match (request.method().clone(), request.url()) {
        (Method::Get, "/api/status") => {
            let all = db.list_workspaces().unwrap_or_default();
            let active = db.state("active_workspace_id").ok().flatten();
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
            .unwrap();
            respond(request, 200, &body)
        }
        (Method::Get, "/api/poll") => {
            let command = bridge.queue.lock().ok().and_then(|mut q| q.pop_front());
            respond(request, 200, &serde_json::to_string(&command).unwrap())
        }
        (Method::Post, "/api/capture") => match read_json::<CapturePayload>(&mut request) {
            Ok(p) => {
                let _ = db.save_session(&p.workspace_id, &p.tabs);
                if let Some(tx) = bridge
                    .pending
                    .lock()
                    .ok()
                    .and_then(|mut map| map.remove(&p.request_id))
                {
                    let _ = tx.send(p.tabs);
                }
                respond(request, 200, r#"{"ok":true}"#)
            }
            Err(e) => respond(
                request,
                400,
                &format!(r#"{{"error":{}}}"#, serde_json::to_string(&e).unwrap()),
            ),
        },
        (Method::Post, "/api/add-tabs") => match read_json::<AddPayload>(&mut request) {
            Ok(p) => {
                for tab in p.tabs {
                    let _ = db.add_browser(&p.workspace_id, &tab.title, &tab.url, false);
                }
                respond(request, 200, r#"{"ok":true}"#)
            }
            Err(e) => respond(
                request,
                400,
                &format!(r#"{{"error":{}}}"#, serde_json::to_string(&e).unwrap()),
            ),
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
        response.add_header(Header::from_bytes(h.0, h.1).unwrap());
    }
    let _ = request.respond(response);
}
