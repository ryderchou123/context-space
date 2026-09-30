use crate::models::*;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use std::{collections::HashSet, path::Path, sync::Mutex, time::Duration};
use uuid::Uuid;

pub const ACTIVE_WORKSPACE: &str = "active_workspace_id";
/// Set when the app starts with an active workspace left over from a previous run.
/// While set, switching away must not close tabs or apps based on that stale state.
pub const ACTIVE_RECOVERED: &str = "active_workspace_recovered";
pub const MISSING_WORKSPACE: &str = "Workspace no longer exists.";
pub const DUPLICATE_URL: &str = "That URL is already in this workspace.";

pub struct Database {
    conn: Mutex<Connection>,
    path: String,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(err)?;
        // Never repair or recreate a damaged file: report it and leave the bytes untouched.
        let check: String = conn
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(|e| format!("The database at {} cannot be read: {e}", path.display()))?;
        if check != "ok" {
            return Err(format!(
                "The database at {} is damaged ({check}). It was left unchanged.",
                path.display()
            ));
        }
        migrate(&conn)?;
        let db = Self {
            conn: Mutex::new(conn),
            path: path.display().to_string(),
        };
        db.ensure_bridge_token()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrate(&conn).unwrap();
        let db = Self {
            conn: Mutex::new(conn),
            path: ":memory:".into(),
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

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn schema_version(&self) -> Result<i64, String> {
        self.conn
            .lock()
            .map_err(err)?
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)
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

    /// Called once at startup: an active workspace that survived a restart is stale.
    pub fn mark_active_as_recovered(&self) -> Result<bool, String> {
        if self.state(ACTIVE_WORKSPACE)?.is_some() {
            self.set_state(ACTIVE_RECOVERED, Some("1"))?;
            return Ok(true);
        }
        Ok(false)
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
            .ok_or_else(|| MISSING_WORKSPACE.into())
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
        // An editor left open on a deleted workspace must not silently recreate it.
        if let Some(id) = &d.id
            && !workspace_exists(&conn, id)?
        {
            return Err(MISSING_WORKSPACE.into());
        }
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
        let mut conn = self.conn.lock().map_err(err)?;
        let tx = conn.transaction().map_err(err)?;
        let was_active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM app_state WHERE key=?1 AND value=?2)",
                params![ACTIVE_WORKSPACE, id],
                |r| r.get(0),
            )
            .map_err(err)?;
        tx.execute("DELETE FROM workspaces WHERE id=?1", [id])
            .map_err(err)?;
        if was_active {
            tx.execute(
                "DELETE FROM app_state WHERE key IN (?1,?2)",
                params![ACTIVE_WORKSPACE, ACTIVE_RECOVERED],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    /// Moves a workspace by list index and renumbers every position, so gaps left by
    /// deleted workspaces never make a move silently do nothing.
    pub fn reorder(&self, id: &str, direction: i64) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(err)?;
        let tx = conn.transaction().map_err(err)?;
        let mut ids: Vec<String> = tx
            .prepare("SELECT id FROM workspaces ORDER BY position,name")
            .map_err(err)?
            .query_map([], |r| r.get(0))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        let index = ids
            .iter()
            .position(|w| w == id)
            .ok_or_else(|| MISSING_WORKSPACE.to_string())?;
        let target = index as i64 + direction;
        if target < 0 || target >= ids.len() as i64 {
            return Ok(());
        }
        ids.swap(index, target as usize);
        for (position, workspace_id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE workspaces SET position=?1 WHERE id=?2",
                params![position as i64, workspace_id],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    pub fn add_browser(
        &self,
        workspace_id: &str,
        title: &str,
        url: &str,
        pinned: bool,
    ) -> Result<(), String> {
        let parsed = url::Url::parse(url.trim())
            .map_err(|_| "Enter a valid HTTP or HTTPS URL.".to_string())?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Only HTTP and HTTPS URLs are supported.".into());
        }
        let conn = self.conn.lock().map_err(err)?;
        if !workspace_exists(&conn, workspace_id)? {
            return Err(MISSING_WORKSPACE.into());
        }
        let normalized = normalize_url(parsed);
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM browser_resources WHERE workspace_id=?1 AND url=?2)",
                params![workspace_id, normalized],
                |r| r.get(0),
            )
            .map_err(err)?;
        if exists {
            return Err(DUPLICATE_URL.into());
        }
        let title = match title.trim() {
            "" => normalized.as_str(),
            t => t,
        };
        let position: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position),-1)+1 FROM browser_resources WHERE workspace_id=?1",
                [workspace_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        conn.execute("INSERT INTO browser_resources(id,workspace_id,title,url,pinned,enabled,position) VALUES(?1,?2,?3,?4,?5,1,?6)",params![Uuid::new_v4().to_string(),workspace_id,title,normalized,pinned,position]).map_err(err)?;
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
        let (display_name, path) = (display_name.trim(), path.trim());
        if display_name.is_empty() {
            return Err("Application name is required.".into());
        }
        if !Path::new(path).is_file() {
            return Err("The selected executable does not exist or is not a file.".into());
        }
        let process_name = process_name.trim().trim_end_matches(".exe");
        let conn = self.conn.lock().map_err(err)?;
        if !workspace_exists(&conn, workspace_id)? {
            return Err(MISSING_WORKSPACE.into());
        }
        conn.execute("INSERT INTO app_resources(id,workspace_id,display_name,executable_path,process_name,launch_args,enabled) VALUES(?1,?2,?3,?4,?5,?6,1)",params![Uuid::new_v4().to_string(),workspace_id,display_name,path,process_name,args.trim()]).map_err(err)?;
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
        if !workspace_exists(&conn, workspace_id)? {
            return Err(MISSING_WORKSPACE.into());
        }
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
        let mut seen = HashSet::new();
        let mut position = 0i64;
        for (title, url) in session_urls(tabs) {
            if seen.insert(url.clone()) {
                tx.execute("INSERT INTO session_tabs(id,session_id,title,url,position) VALUES(?1,?2,?3,?4,?5)",params![Uuid::new_v4().to_string(),sid,title,url,position]).map_err(err)?;
                position += 1;
            }
        }
        tx.commit().map_err(err)
    }

    /// Saves tabs captured by the extension. An empty capture never replaces an existing
    /// Last Session (the browser may have restarted and lost tab ownership), so this
    /// returns `false` and keeps the previous session.
    pub fn save_captured_session(
        &self,
        workspace_id: &str,
        tabs: &[SessionTab],
    ) -> Result<bool, String> {
        if session_urls(tabs).next().is_none() {
            return Ok(false);
        }
        self.save_session(workspace_id, tabs)?;
        Ok(true)
    }
}

fn workspace_exists(conn: &Connection, id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
        [id],
        |r| r.get(0),
    )
    .map_err(err)
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

fn session_urls(tabs: &[SessionTab]) -> impl Iterator<Item = (&str, String)> {
    tabs.iter()
        .filter_map(|t| parse_web_url(&t.url).map(|u| (t.title.as_str(), normalize_url(u))))
}

pub fn parse_web_url(input: &str) -> Option<url::Url> {
    url::Url::parse(input.trim())
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
}

/// URL identity shared with core.ts and the extension (see tests/fixtures/url-normalization.json).
/// Drops the fragment and a trailing slash when there is no query (the parser already drops
/// default ports and lowercases the host). Scheme, www, path case and query stay significant
/// so similar pages are never treated as the same tab.
pub fn normalize_url(mut url: url::Url) -> String {
    url.set_fragment(None);
    let mut s = url.to_string();
    if url.query().is_none() && s.ends_with('/') {
        s.pop();
    }
    s
}

/// Normalizes any string, falling back to the trimmed input when it is not a web URL.
pub fn url_key(input: &str) -> String {
    parse_web_url(input)
        .map(normalize_url)
        .unwrap_or_else(|| input.trim().to_string())
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Ordered schema migrations. `MIGRATIONS[n]` upgrades `user_version` n to n+1.
/// Never edit a released entry and never drop user tables: append a new step instead.
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY,name TEXT NOT NULL,icon TEXT NOT NULL,color TEXT NOT NULL,position INTEGER NOT NULL,browser_exit_behavior TEXT NOT NULL DEFAULT 'close',app_exit_behavior TEXT NOT NULL DEFAULT 'minimize',save_session_on_exit INTEGER NOT NULL DEFAULT 1,restore_session_on_launch INTEGER NOT NULL DEFAULT 1,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS browser_resources(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,title TEXT NOT NULL,url TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 1,enabled INTEGER NOT NULL DEFAULT 1,position INTEGER NOT NULL DEFAULT 0,UNIQUE(workspace_id,url));
CREATE TABLE IF NOT EXISTS app_resources(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,display_name TEXT NOT NULL,executable_path TEXT NOT NULL,process_name TEXT NOT NULL,launch_args TEXT NOT NULL DEFAULT '',enabled INTEGER NOT NULL DEFAULT 1,exit_behavior_override TEXT);
CREATE TABLE IF NOT EXISTS workspace_sessions(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS session_tabs(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES workspace_sessions(id) ON DELETE CASCADE,title TEXT NOT NULL,url TEXT NOT NULL,position INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS app_state(key TEXT PRIMARY KEY,value TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_browser_workspace ON browser_resources(workspace_id);CREATE INDEX IF NOT EXISTS idx_apps_workspace ON app_resources(workspace_id);CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON workspace_sessions(workspace_id);
"#];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(err)?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "This database was created by a newer Context Space version (schema {version}). It was left unchanged."
        ));
    }
    for (index, step) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        // Each step and its version bump commit together, so a failed upgrade rolls back.
        let tx = conn.unchecked_transaction().map_err(err)?;
        tx.execute_batch(step).map_err(err)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)
            .map_err(err)?;
        tx.commit().map_err(err)?;
    }
    Ok(())
}

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
    fn tab(url: &str) -> SessionTab {
        SessionTab {
            title: url.into(),
            url: url.into(),
        }
    }
    fn count(db: &Database, table: &str) -> i64 {
        db.conn
            .lock()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("context-space-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
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
            &[
                SessionTab {
                    title: "Page".into(),
                    url: "https://example.com/path#part".into(),
                },
                SessionTab {
                    title: "Duplicate".into(),
                    url: "https://example.com/path".into(),
                },
            ],
        )
        .unwrap();
        let w = db.workspace(&id).unwrap();
        assert_eq!(w.browser_resources[0].url, "https://example.com");
        assert_eq!(w.last_session[0].url, "https://example.com/path");
        assert_eq!(w.last_session.len(), 1);
        db.delete_workspace(&id).unwrap();
        assert!(db.list_workspaces().unwrap().is_empty());
    }

    #[test]
    fn update_reorder_settings_and_empty_session_persist() {
        let db = Database::memory();
        let first = db.save_workspace(draft("Work")).unwrap();
        let second = db.save_workspace(draft("Games")).unwrap();
        let mut renamed = draft("Focused Work");
        renamed.id = Some(first.clone());
        renamed.browser_exit_behavior = "keep".into();
        renamed.app_exit_behavior = "safe_close".into();
        renamed.save_session_on_exit = false;
        db.save_workspace(renamed).unwrap();
        db.reorder(&second, -1).unwrap();
        db.save_session(&first, &[]).unwrap();
        let all = db.list_workspaces().unwrap();
        assert_eq!(all[0].id, second);
        let work = db.workspace(&first).unwrap();
        assert_eq!(work.name, "Focused Work");
        assert_eq!(work.browser_exit_behavior, "keep");
        assert_eq!(work.app_exit_behavior, "safe_close");
        assert!(!work.save_session_on_exit);
        assert!(work.last_session.is_empty());
    }

    #[test]
    fn delete_workspace_cascades_resources_sessions_and_active_state() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Temporary")).unwrap();
        let keep = db.save_workspace(draft("Keep")).unwrap();
        db.add_browser(&id, "Example", "https://example.com", true)
            .unwrap();
        db.add_browser(&keep, "Example", "https://example.com", true)
            .unwrap();
        db.save_session(&id, &[tab("https://example.com")]).unwrap();
        db.set_state(ACTIVE_WORKSPACE, Some(&id)).unwrap();
        db.set_state(ACTIVE_RECOVERED, Some("1")).unwrap();
        db.delete_workspace(&id).unwrap();
        assert_eq!(db.list_workspaces().unwrap().len(), 1);
        assert_eq!(db.state(ACTIVE_WORKSPACE).unwrap(), None);
        assert_eq!(db.state(ACTIVE_RECOVERED).unwrap(), None);
        // No orphans: only the other workspace's resource remains.
        assert_eq!(count(&db, "browser_resources"), 1);
        assert_eq!(count(&db, "workspace_sessions"), 0);
        assert_eq!(count(&db, "session_tabs"), 0);
    }

    #[test]
    fn deleting_inactive_workspace_keeps_active_state() {
        let db = Database::memory();
        let work = db.save_workspace(draft("Work")).unwrap();
        let temp = db.save_workspace(draft("Temp")).unwrap();
        db.set_state(ACTIVE_WORKSPACE, Some(&work)).unwrap();
        db.delete_workspace(&temp).unwrap();
        assert_eq!(db.state(ACTIVE_WORKSPACE).unwrap(), Some(work));
    }

    #[test]
    fn regression_bug_011_reorder_works_after_a_workspace_was_deleted() {
        let db = Database::memory();
        let a = db.save_workspace(draft("A")).unwrap();
        let b = db.save_workspace(draft("B")).unwrap();
        let c = db.save_workspace(draft("C")).unwrap();
        db.delete_workspace(&b).unwrap(); // positions are now 0 and 2
        db.reorder(&c, -1).unwrap();
        let order: Vec<_> = db
            .list_workspaces()
            .unwrap()
            .into_iter()
            .map(|w| w.id)
            .collect();
        assert_eq!(order, vec![c.clone(), a.clone()]);
        // Moving past either edge is a no-op, not an error.
        db.reorder(&c, -1).unwrap();
        db.reorder(&a, 1).unwrap();
        assert_eq!(db.list_workspaces().unwrap()[0].id, c);
    }

    #[test]
    fn regression_bug_012_saving_a_deleted_workspace_does_not_resurrect_it() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Work")).unwrap();
        db.delete_workspace(&id).unwrap();
        let mut stale = draft("Work");
        stale.id = Some(id);
        assert_eq!(db.save_workspace(stale).unwrap_err(), MISSING_WORKSPACE);
        assert!(db.list_workspaces().unwrap().is_empty());
    }

    #[test]
    fn resources_for_missing_workspace_return_a_readable_error() {
        let db = Database::memory();
        assert_eq!(
            db.add_browser("missing", "", "https://example.com", true)
                .unwrap_err(),
            MISSING_WORKSPACE
        );
        assert_eq!(
            db.save_session("missing", &[tab("https://example.com")])
                .unwrap_err(),
            MISSING_WORKSPACE
        );
    }

    #[test]
    fn browser_resources_validate_and_default_title() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Work")).unwrap();
        assert!(db.add_browser(&id, "", "ftp://example.com", true).is_err());
        assert!(db.add_browser(&id, "", "not a url", true).is_err());
        db.add_browser(&id, "  ", "https://example.com/jobs/", true)
            .unwrap();
        // Different path case is a different page, not a duplicate.
        db.add_browser(&id, "Jobs", "https://example.com/Jobs", true)
            .unwrap();
        let w = db.workspace(&id).unwrap();
        assert_eq!(w.browser_resources[0].title, "https://example.com/jobs");
        assert_eq!(w.browser_resources.len(), 2);
    }

    #[test]
    fn app_resources_require_an_existing_file() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Work")).unwrap();
        let dir = std::env::temp_dir();
        assert!(
            db.add_app(&id, "Dir", dir.to_str().unwrap(), "", "")
                .is_err(),
            "a directory is not an executable"
        );
        assert!(
            db.add_app(&id, "Missing", "C:/definitely/missing.exe", "", "")
                .is_err()
        );
        let exe = std::env::current_exe().unwrap();
        db.add_app(&id, " Tool ", exe.to_str().unwrap(), "tool.exe", "")
            .unwrap();
        let app = &db.workspace(&id).unwrap().app_resources[0];
        assert_eq!(app.display_name, "Tool");
        assert_eq!(app.process_name, "tool");
    }

    #[test]
    fn regression_bug_009_empty_capture_keeps_previous_last_session() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Work")).unwrap();
        assert!(
            db.save_captured_session(&id, &[tab("https://linkedin.com")])
                .unwrap()
        );
        assert!(!db.save_captured_session(&id, &[]).unwrap());
        assert!(
            !db.save_captured_session(&id, &[tab("chrome://settings")])
                .unwrap()
        );
        assert_eq!(db.workspace(&id).unwrap().last_session.len(), 1);
        // A real capture still overwrites the previous session.
        db.save_captured_session(&id, &[tab("https://a.test"), tab("https://b.test")])
            .unwrap();
        let session = db.workspace(&id).unwrap().last_session;
        assert_eq!(
            session.iter().map(|t| t.url.as_str()).collect::<Vec<_>>(),
            vec!["https://a.test", "https://b.test"]
        );
    }

    #[test]
    fn regression_bug_010_restart_marks_active_workspace_as_recovered() {
        let db = Database::memory();
        assert!(!db.mark_active_as_recovered().unwrap());
        let id = db.save_workspace(draft("Work")).unwrap();
        db.set_state(ACTIVE_WORKSPACE, Some(&id)).unwrap();
        assert!(db.mark_active_as_recovered().unwrap());
        assert_eq!(db.state(ACTIVE_RECOVERED).unwrap().as_deref(), Some("1"));
        assert_eq!(db.state(ACTIVE_WORKSPACE).unwrap(), Some(id));
    }

    #[test]
    fn url_normalization_matches_shared_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/url-normalization.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = case["input"].as_str().unwrap();
            let expected = case["expected"].as_str().unwrap();
            assert_eq!(
                normalize_url(parse_web_url(input).unwrap()),
                expected,
                "{input}"
            );
        }
        for pair in fixture["distinct"].as_array().unwrap() {
            let (a, b) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
            assert_ne!(url_key(a), url_key(b), "{a} vs {b}");
        }
        for input in fixture["rejected"].as_array().unwrap() {
            assert!(parse_web_url(input.as_str().unwrap()).is_none(), "{input}");
        }
    }

    #[test]
    fn migration_sets_schema_version_without_destroying_existing_data() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute("INSERT INTO workspaces(id,name,icon,color,position,browser_exit_behavior,app_exit_behavior,save_session_on_exit,restore_session_on_launch,created_at,updated_at) VALUES('1','Work','briefcase','#000',0,'close','minimize',1,1,'now','now')", []).unwrap();
        migrate(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM workspaces", [], |r| r.get(0))
            .unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn v1_database_file_reopens_with_all_user_data() {
        let path = temp_path("v1.sqlite");
        let (work, token) = {
            let db = Database::open(&path).unwrap();
            let work = db.save_workspace(draft("Work")).unwrap();
            db.add_browser(&work, "LinkedIn", "https://linkedin.com", true)
                .unwrap();
            db.add_app(
                &work,
                "Tool",
                std::env::current_exe().unwrap().to_str().unwrap(),
                "",
                "",
            )
            .unwrap();
            db.save_session(&work, &[tab("https://linkedin.com/jobs")])
                .unwrap();
            db.set_state(ACTIVE_WORKSPACE, Some(&work)).unwrap();
            (work, db.state("bridge_token").unwrap())
        };
        let db = Database::open(&path).unwrap();
        let w = db.workspace(&work).unwrap();
        assert_eq!(w.browser_resources.len(), 1);
        assert_eq!(w.app_resources.len(), 1);
        assert_eq!(w.last_session.len(), 1);
        assert_eq!(db.state(ACTIVE_WORKSPACE).unwrap(), Some(work));
        assert_eq!(
            db.state("bridge_token").unwrap(),
            token,
            "token must survive restart"
        );
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn regression_bug_013_corrupted_database_is_reported_and_left_untouched() {
        let path = temp_path("corrupt.sqlite");
        let garbage = b"this is not a sqlite database, but it is the user's file".repeat(200);
        std::fs::write(&path, &garbage).unwrap();
        let error = Database::open(&path).err().expect("must not open");
        assert!(
            error.contains("cannot be read") || error.contains("damaged"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            garbage,
            "file must be unchanged"
        );
    }

    #[test]
    fn newer_schema_is_refused_without_changes() {
        let path = temp_path("future.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE future(x); PRAGMA user_version=99;")
                .unwrap();
        }
        let error = Database::open(&path).err().expect("must refuse");
        assert!(error.contains("newer"), "{error}");
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 99);
    }

    #[test]
    fn locked_database_waits_then_reports_instead_of_corrupting() {
        let path = temp_path("locked.sqlite");
        let db = Database::open(&path).unwrap();
        let blocker = Connection::open(&path).unwrap();
        blocker.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        db.conn
            .lock()
            .unwrap()
            .busy_timeout(Duration::from_millis(50))
            .unwrap();
        assert!(db.save_workspace(draft("Blocked")).is_err());
        blocker.execute_batch("ROLLBACK;").unwrap();
        db.save_workspace(draft("Unblocked")).unwrap();
        assert_eq!(db.list_workspaces().unwrap().len(), 1);
    }
}
