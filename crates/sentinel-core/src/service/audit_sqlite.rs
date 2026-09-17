//! Durable audit log in SQLite (app data dir). Entries are stored as JSON so the schema follows the
//! contract types without migrations; id, timestamp and origin are columns for querying.

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};

use crate::action::Origin;
use crate::audit::{AuditEntry, AuditQuery, AuditStore, OriginFilter};
use crate::error::{CoreResult, SentinelError};

pub struct SqliteAuditStore {
    conn: Mutex<Connection>,
}

fn db_error(err: rusqlite::Error) -> SentinelError {
    SentinelError::Io {
        detail: format!("audit log database: {err}"),
        path: None,
    }
}

impl SqliteAuditStore {
    pub fn open(path: &Path) -> CoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| SentinelError::io(&err, Some(parent)))?;
        }
        let conn = Connection::open(path).map_err(db_error)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> CoreResult<Self> {
        Self::init(Connection::open_in_memory().map_err(db_error)?)
    }

    fn init(conn: Connection) -> CoreResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS audit_log (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts_ms INTEGER NOT NULL,
                 origin TEXT NOT NULL,
                 entry TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS audit_log_origin ON audit_log (origin, id);",
        )
        .map_err(db_error)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

fn origin_key(origin: &Origin) -> &'static str {
    match origin {
        Origin::User => "user",
        Origin::Agent { .. } => "agent",
    }
}

impl AuditStore for SqliteAuditStore {
    fn append(&self, mut entry: AuditEntry) -> CoreResult<i64> {
        entry.id = 0;
        let json = serde_json::to_string(&entry).map_err(SentinelError::internal)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO audit_log (ts_ms, origin, entry) VALUES (?1, ?2, ?3)",
            params![entry.ts_ms as i64, origin_key(&entry.origin), json],
        )
        .map_err(db_error)?;
        Ok(conn.last_insert_rowid())
    }

    fn query(&self, query: &AuditQuery) -> CoreResult<Vec<AuditEntry>> {
        let limit = i64::from(query.limit.clamp(1, 1000));
        let before = query.before_id.unwrap_or(i64::MAX);
        let origin = match query.origin {
            OriginFilter::Any => None,
            OriginFilter::User => Some("user"),
            OriginFilter::Agent => Some("agent"),
        };
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, entry FROM audit_log
                 WHERE id < ?1 AND (?2 IS NULL OR origin = ?2)
                 ORDER BY id DESC LIMIT ?3",
            )
            .map_err(db_error)?;
        let rows = stmt
            .query_map(params![before, origin, limit], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, json) = row.map_err(db_error)?;
            let mut entry: AuditEntry =
                serde_json::from_str(&json).map_err(SentinelError::internal)?;
            entry.id = id;
            entries.push(entry);
        }
        Ok(entries)
    }
}

impl SqliteAuditStore {
    pub fn get(&self, id: i64) -> CoreResult<Option<AuditEntry>> {
        let conn = self.conn.lock();
        let json: Option<String> = conn
            .query_row("SELECT entry FROM audit_log WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(db_error)?;
        json.map(|json| {
            serde_json::from_str::<AuditEntry>(&json)
                .map(|mut entry| {
                    entry.id = id;
                    entry
                })
                .map_err(SentinelError::internal)
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::audit::AuditStatus;

    fn entry(origin: Origin, ts_ms: u64) -> AuditEntry {
        AuditEntry {
            id: 0,
            ts_ms,
            trigger: match &origin {
                Origin::Agent { request, .. } => Some(request.clone()),
                Origin::User => None,
            },
            origin,
            action: Action::TrashPaths {
                paths: vec!["/tmp/x".into()],
            },
            title: "Move 1 item to Trash".into(),
            status: AuditStatus::Succeeded,
            summary: "Moved".into(),
            bytes_freed: Some(10),
            affected_paths: vec!["/tmp/x".into()],
            before: vec![],
            after: vec![],
        }
    }

    fn agent() -> Origin {
        Origin::Agent {
            conversation_id: "c".into(),
            plan_id: "p".into(),
            provider: "ollama".into(),
            model: "llama3.1:8b".into(),
            request: "free up space".into(),
        }
    }

    #[test]
    fn appends_and_pages_with_origin_filter() {
        let store = SqliteAuditStore::open_in_memory().unwrap();
        for i in 0..5 {
            let origin = if i % 2 == 0 { Origin::User } else { agent() };
            let id = store.append(entry(origin, i)).unwrap();
            assert_eq!(id, i as i64 + 1);
        }
        let all = store
            .query(&AuditQuery {
                limit: 3,
                before_id: None,
                origin: OriginFilter::Any,
            })
            .unwrap();
        assert_eq!(all.iter().map(|e| e.id).collect::<Vec<_>>(), vec![5, 4, 3]);
        let next = store
            .query(&AuditQuery {
                limit: 10,
                before_id: Some(3),
                origin: OriginFilter::Any,
            })
            .unwrap();
        assert_eq!(next.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 1]);
        let agents = store
            .query(&AuditQuery {
                limit: 10,
                before_id: None,
                origin: OriginFilter::Agent,
            })
            .unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].trigger.as_deref(), Some("free up space"));
        assert_eq!(store.get(4).unwrap().map(|e| e.id), Some(4));
    }

    #[test]
    fn persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.sqlite3");
        {
            let store = SqliteAuditStore::open(&path).unwrap();
            store.append(entry(Origin::User, 1)).unwrap();
        }
        let store = SqliteAuditStore::open(&path).unwrap();
        let entries = store
            .query(&AuditQuery {
                limit: 10,
                before_id: None,
                origin: OriginFilter::User,
            })
            .unwrap();
        assert_eq!(entries.len(), 1);
    }
}
