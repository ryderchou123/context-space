use chrono::Utc;
use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex};

const CAPACITY: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: String,
    pub event: String,
    pub detail: String,
}

/// Local-only structured event log for debugging. Details carry ids, names and counts;
/// never page URLs or titles, and nothing is sent off the machine.
pub struct EventLog {
    entries: Mutex<VecDeque<LogEntry>>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(CAPACITY)),
        }
    }

    pub fn record(&self, event: &str, detail: impl Into<String>) {
        let detail = detail.into();
        log::info!(target: "context_space", "event={event} {detail}");
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() == CAPACITY {
                entries.pop_front();
            }
            entries.push_back(LogEntry {
                at: Utc::now().to_rfc3339(),
                event: event.into(),
                detail,
            });
        }
    }

    pub fn recent(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .map(|e| e.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    pub fn last_of(&self, events: &[&str]) -> Option<LogEntry> {
        self.entries
            .lock()
            .ok()?
            .iter()
            .rev()
            .find(|e| events.contains(&e.event.as_str()))
            .cloned()
    }

    #[cfg(test)]
    pub fn names(&self) -> Vec<String> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.event.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_most_recent_entries_newest_first() {
        let log = EventLog::new();
        for i in 0..CAPACITY + 5 {
            log.record("TAB_OPEN", format!("count={i}"));
        }
        let recent = log.recent();
        assert_eq!(recent.len(), CAPACITY);
        assert_eq!(recent[0].detail, format!("count={}", CAPACITY + 4));
    }
}
