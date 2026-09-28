use crate::models::*;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(err)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(err)?;
        conn.execute_batch(SCHEMA).map_err(err)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.ensure_bridge_token()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.ensure_bridge_token().unwrap();
        db
    }

    fn ensure_bridge_token(&self) -> Result<(), String> {
        self.conn
            .lock()
            .map_err(err)?
            .execute(
                "INSERT OR IGNORE INTO app_state(key,value) VALUES('bridge_token',?1)",
                [Uuid::new_v4().to_string()],
            )
            .map_err(err)?;
        Ok(())
    }

    pub fn state(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .lock()
            .map_err(err)?
            .query_row("SELECT value FROM app_state WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)
    }

    pub fn set_state(&self, key: &str, value: Option<&str>) -> Result<(), String> {
        let conn = self.conn.lock().map_err(err)?;
        match value {
            Some(v) => {
                conn.execute("INSERT INTO app_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,v]).map_err(err)?;
            }
            None => {
                conn.execute("DELETE FROM app_state WHERE key=?1", [key])
                    .map_err(err)?;
            }
        }
        Ok(())
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, String> {
        let conn = self.conn.lock().map_err(err)?;
        let mut stmt = conn.prepare("SELECT id,name,icon,color,position,browser_exit_behavior,app_exit_behavior,save_session_on_exit,restore_session_on_launch,created_at,updated_at FROM workspaces ORDER BY position,name").map_err(err)?;
        let base = stmt
            .query_map([], |r| {
                Ok(Workspace {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    icon: r.get(2)?,
                    color: r.get(3)?,
                    position: r.get(4)?,
                    browser_exit_behavior: r.get(5)?,
                    app_exit_behavior: r.get(6)?,
                    save_session_on_exit: r.get(7)?,
                    restore_session_on_launch: r.get(8)?,
                    browser_resources: vec![],
                    app_resources: vec![],
                    last_session: vec![],
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        base.into_iter()
            .map(|mut w| {
                w.browser_resources = browser_resources(&conn, &w.id)?;
                w.app_resources = app_resources(&conn, &w.id)?;
                w.last_session = last_session(&conn, &w.id)?;
                Ok(w)
            })
            .collect()
    }

    pub fn workspace(&self, id: &str) -> Result<Workspace, String> {
        self.list_workspaces()?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or_else(|| "Workspace no longer exists.".into())
    }

    pub fn save_workspace(&self, d: WorkspaceDraft) -> Result<String, String> {
        let name = d.name.trim();
        if name.is_empty() {
            return Err("Workspace name is required.".into());
        }
        if !matches!(d.browser_exit_behavior.as_str(), "keep" | "close")
            || !matches!(
                d.app_exit_behavior.as_str(),
                "keep" | "minimize" | "safe_close"
            )
        {
            return Err("Invalid workspace behavior.".into());
        }
        let conn = self.conn.lock().map_err(err)?;
        let now = Utc::now().to_rfc3339();
        let id = d.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let position: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position),-1)+1 FROM workspaces",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        conn.execute("INSERT INTO workspaces(id,name,icon,color,position,browser_exit_behavior,app_exit_behavior,save_session_on_exit,restore_session_on_launch,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10) ON CONFLICT(id) DO UPDATE SET name=excluded.name,icon=excluded.icon,color=excluded.color,browser_exit_behavior=excluded.browser_exit_behavior,app_exit_behavior=excluded.app_exit_behavior,save_session_on_exit=excluded.save_session_on_exit,restore_session_on_launch=excluded.restore_session_on_launch,updated_at=excluded.updated_at", params![id,name,d.icon,d.color,position,d.browser_exit_behavior,d.app_exit_behavior,d.save_session_on_exit,d.restore_session_on_launch,now]).map_err(err)?;
        Ok(id)
    }

    pub fn delete_workspace(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(err)?;
        conn.execute("DELETE FROM workspaces WHERE id=?1", [id])
            .map_err(err)?;
        if self
            .state_unlocked(&conn, "active_workspace_id")?
            .as_deref()
            == Some(id)
        {
            conn.execute("DELETE FROM app_state WHERE key='active_workspace_id'", [])
                .map_err(err)?;
        }
        Ok(())
    }
    fn state_unlocked(&self, conn: &Connection, key: &str) -> Result<Option<String>, String> {
        conn.query_row("SELECT value FROM app_state WHERE key=?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .map_err(err)
    }

    pub fn reorder(&self, id: &str, direction: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(err)?;
        let current: i64 = conn
            .query_row("SELECT position FROM workspaces WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(err)?;
        let target = current + direction;
        let other: Option<String> = conn
            .query_row(
                "SELECT id FROM workspaces WHERE position=?1",
                [target],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if let Some(other_id) = other {
            conn.execute(
                "UPDATE workspaces SET position=?1 WHERE id=?2",
                params![current, other_id],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE workspaces SET position=?1 WHERE id=?2",
                params![target, id],
            )
            .map_err(err)?;
        }
        Ok(())
    }

    pub fn add_browser(
        &self,
        workspace_id: &str,
        title: &str,
        url: &str,
        pinned: bool,
    ) -> Result<(), String> {
        let parsed =
            url::Url::parse(url).map_err(|_| "Enter a valid HTTP or HTTPS URL.".to_string())?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Only HTTP and HTTPS URLs are supported.".into());
        }
        let conn = self.conn.lock().map_err(err)?;
        let normalized = normalize_url(parsed);
        let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM browser_resources WHERE workspace_id=?1 AND lower(url)=lower(?2))",params![workspace_id,normalized],|r|r.get(0)).map_err(err)?;
        if exists {
            return Err("That URL is already in this workspace.".into());
        }
        let position: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position),-1)+1 FROM browser_resources WHERE workspace_id=?1",
                [workspace_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        conn.execute("INSERT INTO browser_resources(id,workspace_id,title,url,pinned,enabled,position) VALUES(?1,?2,?3,?4,?5,1,?6)",params![Uuid::new_v4().to_string(),workspace_id,title.trim(),normalized,pinned,position]).map_err(err)?;
        Ok(())
    }
    pub fn remove_browser(&self, id: &str) -> Result<(), String> {
        self.conn
            .lock()
            .map_err(err)?
            .execute("DELETE FROM browser_resources WHERE id=?1", [id])
            .map_err(err)?;
        Ok(())
    }
    pub fn toggle_browser(&self, id: &str, enabled: bool, pinned: bool) -> Result<(), String> {
        self.conn
            .lock()
            .map_err(err)?
            .execute(
                "UPDATE browser_resources SET enabled=?1,pinned=?2 WHERE id=?3",
                params![enabled, pinned, id],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn add_app(
        &self,
        workspace_id: &str,
        display_name: &str,
        path: &str,
        process_name: &str,
        args: &str,
    ) -> Result<(), String> {
        if !Path::new(path).exists() {
            return Err("The selected executable no longer exists.".into());
        }
        let conn = self.conn.lock().map_err(err)?;
        conn.execute("INSERT INTO app_resources(id,workspace_id,display_name,executable_path,process_name,launch_args,enabled) VALUES(?1,?2,?3,?4,?5,?6,1)",params![Uuid::new_v4().to_string(),workspace_id,display_name,path,process_name,args]).map_err(err)?;
        Ok(())
    }
    pub fn remove_app(&self, id: &str) -> Result<(), String> {
        self.conn
            .lock()
            .map_err(err)?
            .execute("DELETE FROM app_resources WHERE id=?1", [id])
            .map_err(err)?;
        Ok(())
    }

    pub fn save_session(&self, workspace_id: &str, tabs: &[SessionTab]) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(err)?;
        let tx = conn.transaction().map_err(err)?;
        tx.execute(
            "DELETE FROM workspace_sessions WHERE workspace_id=?1",
            [workspace_id],
        )
        .map_err(err)?;
        let sid = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO workspace_sessions(id,workspace_id,created_at) VALUES(?1,?2,?3)",
            params![sid, workspace_id, Utc::now().to_rfc3339()],
        )
        .map_err(err)?;
        for (position, tab) in tabs.iter().enumerate() {
            if let Ok(url) = url::Url::parse(&tab.url) {
                if matches!(url.scheme(), "http" | "https") {
                    tx.execute("INSERT INTO session_tabs(id,session_id,title,url,position) VALUES(?1,?2,?3,?4,?5)",params![Uuid::new_v4().to_string(),sid,tab.title,normalize_url(url),position as i64]).map_err(err)?;
                }
            }
        }
        tx.commit().map_err(err)
    }
}

fn browser_resources(conn: &Connection, id: &str) -> Result<Vec<BrowserResource>, String> {
    let mut s=conn.prepare("SELECT id,workspace_id,title,url,pinned,enabled,position FROM browser_resources WHERE workspace_id=?1 ORDER BY position").map_err(err)?;
    s.query_map([id], |r| {
        Ok(BrowserResource {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            title: r.get(2)?,
            url: r.get(3)?,
            pinned: r.get(4)?,
            enabled: r.get(5)?,
            position: r.get(6)?,
        })
    })
    .map_err(err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(err)
}
fn app_resources(conn: &Connection, id: &str) -> Result<Vec<AppResource>, String> {
    let mut s=conn.prepare("SELECT id,workspace_id,display_name,executable_path,process_name,launch_args,enabled,exit_behavior_override FROM app_resources WHERE workspace_id=?1 ORDER BY rowid").map_err(err)?;
    s.query_map([id], |r| {
        Ok(AppResource {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            display_name: r.get(2)?,
            executable_path: r.get(3)?,
            process_name: r.get(4)?,
            launch_args: r.get(5)?,
            enabled: r.get(6)?,
            exit_behavior_override: r.get(7)?,
        })
    })
    .map_err(err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(err)
}
fn last_session(conn: &Connection, id: &str) -> Result<Vec<SessionTab>, String> {
    let sid:Option<String>=conn.query_row("SELECT id FROM workspace_sessions WHERE workspace_id=?1 ORDER BY created_at DESC LIMIT 1",[id],|r|r.get(0)).optional().map_err(err)?;
    let Some(sid) = sid else { return Ok(vec![]) };
    let mut s = conn
        .prepare("SELECT title,url FROM session_tabs WHERE session_id=?1 ORDER BY position")
        .map_err(err)?;
    s.query_map([sid], |r| {
        Ok(SessionTab {
            title: r.get(0)?,
            url: r.get(1)?,
        })
    })
    .map_err(err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(err)
}
fn normalize_url(mut url: url::Url) -> String {
    url.set_fragment(None);
    if (url.scheme() == "https" && url.port() == Some(443))
        || (url.scheme() == "http" && url.port() == Some(80))
    {
        let _ = url.set_port(None);
    }
    let mut s = url.to_string();
    if url.path() == "/" && url.query().is_none() {
        s.truncate(s.len() - 1)
    }
    s
}
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY,name TEXT NOT NULL,icon TEXT NOT NULL,color TEXT NOT NULL,position INTEGER NOT NULL,browser_exit_behavior TEXT NOT NULL DEFAULT 'close',app_exit_behavior TEXT NOT NULL DEFAULT 'minimize',save_session_on_exit INTEGER NOT NULL DEFAULT 1,restore_session_on_launch INTEGER NOT NULL DEFAULT 1,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS browser_resources(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,title TEXT NOT NULL,url TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 1,enabled INTEGER NOT NULL DEFAULT 1,position INTEGER NOT NULL DEFAULT 0,UNIQUE(workspace_id,url));
CREATE TABLE IF NOT EXISTS app_resources(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,display_name TEXT NOT NULL,executable_path TEXT NOT NULL,process_name TEXT NOT NULL,launch_args TEXT NOT NULL DEFAULT '',enabled INTEGER NOT NULL DEFAULT 1,exit_behavior_override TEXT);
CREATE TABLE IF NOT EXISTS workspace_sessions(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS session_tabs(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES workspace_sessions(id) ON DELETE CASCADE,title TEXT NOT NULL,url TEXT NOT NULL,position INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS app_state(key TEXT PRIMARY KEY,value TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_browser_workspace ON browser_resources(workspace_id);CREATE INDEX IF NOT EXISTS idx_apps_workspace ON app_resources(workspace_id);CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON workspace_sessions(workspace_id);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    fn draft(name: &str) -> WorkspaceDraft {
        WorkspaceDraft {
            id: None,
            name: name.into(),
            icon: "briefcase".into(),
            color: "#0D9488".into(),
            browser_exit_behavior: "close".into(),
            app_exit_behavior: "minimize".into(),
            save_session_on_exit: true,
            restore_session_on_launch: true,
        }
    }
    #[test]
    fn crud_and_session_roundtrip() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Work")).unwrap();
        assert_eq!(db.list_workspaces().unwrap().len(), 1);
        db.add_browser(&id, "Example", "https://example.com/", true)
            .unwrap();
        assert!(
            db.add_browser(&id, "Duplicate", "https://example.com", true)
                .is_err()
        );
        db.save_session(
            &id,
            &[SessionTab {
                title: "Page".into(),
                url: "https://example.com/path#part".into(),
            }],
        )
        .unwrap();
        let w = db.workspace(&id).unwrap();
        assert_eq!(w.browser_resources[0].url, "https://example.com");
        assert_eq!(w.last_session[0].url, "https://example.com/path");
        db.delete_workspace(&id).unwrap();
        assert!(db.list_workspaces().unwrap().is_empty());
    }
}
