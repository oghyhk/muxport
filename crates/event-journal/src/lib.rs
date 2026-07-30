use muxport_protocol::{Event, HostSnapshot};
use prost::Message;
use rusqlite::{params, Connection, ErrorCode};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JournalError {
    #[error("Database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("Protobuf encode error: {0}")]
    Encode(#[from] prost::EncodeError),
    #[error("Cursor gap detected: expected seq {expected}, found {found}")]
    GapDetected { expected: u64, found: u64 },
    #[error("Event id has already been journaled: {0}")]
    DuplicateEvent(String),
    #[error("Event id must not be empty")]
    EmptyEventId,
}

pub struct EventJournal {
    conn: Connection,
    boot_epoch: u64,
    current_sequence: u64,
}

impl EventJournal {
    pub fn open_in_memory(boot_epoch: u64) -> Result<Self, JournalError> {
        let conn = Connection::open_in_memory()?;
        let journal = Self {
            conn,
            boot_epoch,
            current_sequence: 0,
        };
        journal.init_tables()?;
        Ok(journal)
    }

    pub fn open_file(path: impl AsRef<Path>, boot_epoch: u64) -> Result<Self, JournalError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        let mut journal = Self {
            conn,
            boot_epoch,
            current_sequence: 0,
        };
        journal.init_tables()?;
        journal.load_max_sequence()?;
        Ok(journal)
    }

    fn init_tables(&self) -> Result<(), JournalError> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                boot_epoch INTEGER NOT NULL,
                event_id TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                payload BLOB NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sequence INTEGER NOT NULL,
                boot_epoch INTEGER NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                snapshot_blob BLOB NOT NULL
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS cursor_acks (
                client_id TEXT PRIMARY KEY,
                sequence INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_events_event_id ON events(event_id)",
            [],
        )?;
        Ok(())
    }

    fn load_max_sequence(&mut self) -> Result<(), JournalError> {
        let mut stmt = self.conn.prepare("SELECT COALESCE(MAX(sequence), 0) FROM events")?;
        let max_seq: u64 = stmt.query_row([], |r| r.get(0))?;
        self.current_sequence = max_seq;
        Ok(())
    }

    pub fn append_event(&mut self, event: &Event) -> Result<u64, JournalError> {
        if event.event_id.is_empty() {
            return Err(JournalError::EmptyEventId);
        }

        let mut payload = Vec::new();
        event.encode(&mut payload)?;

        if let Err(error) = self.conn.execute(
            "INSERT INTO events (boot_epoch, event_id, timestamp_ms, payload) VALUES (?1, ?2, ?3, ?4)",
            params![self.boot_epoch, event.event_id, event.timestamp_ms, payload],
        ) {
            let is_duplicate = match &error {
                rusqlite::Error::SqliteFailure(inner, _) => {
                    inner.code == ErrorCode::ConstraintViolation
                }
                _ => false,
            };
            if is_duplicate {
                return Err(JournalError::DuplicateEvent(event.event_id.clone()));
            }
            return Err(JournalError::Sql(error));
        }

        let seq = self.conn.last_insert_rowid() as u64;
        self.current_sequence = seq;
        Ok(seq)
    }

    pub fn get_events_after(
        &self,
        after_seq: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Event)>, JournalError> {
        let mut stmt = self.conn.prepare(
            "SELECT sequence, payload FROM events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_seq, limit as i64], |row| {
            let seq: u64 = row.get(0)?;
            let payload: Vec<u8> = row.get(1)?;
            Ok((seq, payload))
        })?;

        let mut result = Vec::new();
        let mut expected = after_seq.saturating_add(1);
        for r in rows {
            let (seq, payload) = r?;
            if seq != expected {
                return Err(JournalError::GapDetected {
                    expected,
                    found: seq,
                });
            }
            let event = Event::decode(&payload[..])?;
            result.push((seq, event));
            expected = seq.saturating_add(1);
        }
        Ok(result)
    }

    pub fn save_snapshot(&self, snapshot: &HostSnapshot) -> Result<(), JournalError> {
        let mut blob = Vec::new();
        snapshot.encode(&mut blob)?;
        let now_ms = chrono::Utc::now().timestamp_millis();

        self.conn.execute(
            "INSERT INTO snapshots (sequence, boot_epoch, timestamp_ms, snapshot_blob) VALUES (?1, ?2, ?3, ?4)",
            params![snapshot.snapshot_sequence, self.boot_epoch, now_ms, blob],
        )?;
        Ok(())
    }

    pub fn latest_snapshot(&self) -> Result<Option<HostSnapshot>, JournalError> {
        let mut statement = self.conn.prepare(
            "SELECT snapshot_blob FROM snapshots ORDER BY sequence DESC, id DESC LIMIT 1",
        )?;
        let mut rows = statement.query([])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let blob: Vec<u8> = row.get(0)?;
        Ok(Some(HostSnapshot::decode(&blob[..])?))
    }

    pub fn current_sequence(&self) -> u64 {
        self.current_sequence
    }

    pub fn record_cursor_ack(&self, client_id: &str, sequence: u64) -> Result<(), JournalError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT OR REPLACE INTO cursor_acks (client_id, sequence, updated_at_ms) VALUES (?1, ?2, ?3)",
            params![client_id, sequence, now_ms],
        )?;
        Ok(())
    }

    pub fn get_cursor_ack(&self, client_id: &str) -> Result<Option<u64>, JournalError> {
        let mut stmt = self.conn.prepare("SELECT sequence FROM cursor_acks WHERE client_id = ?1")?;
        let mut rows = stmt.query(params![client_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let seq: u64 = row.get(0)?;
        Ok(Some(seq))
    }

    pub fn compact_before_sequence(&self, before_seq: u64) -> Result<usize, JournalError> {
        let deleted = self.conn.execute(
            "DELETE FROM events WHERE sequence < ?1",
            params![before_seq],
        )?;
        Ok(deleted)
    }

    pub fn verify_integrity(&self) -> Result<bool, JournalError> {
        let mut stmt = self.conn.prepare("PRAGMA integrity_check")?;
        let result: String = stmt.query_row([], |r| r.get(0))?;
        Ok(result == "ok")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_journal_append_and_query() {
        let mut journal = EventJournal::open_in_memory(100).unwrap();

        let evt1 = Event {
            event_id: "evt-1".into(),
            timestamp_ms: 1000,
            inner: None,
        };

        let seq1 = journal.append_event(&evt1).unwrap();
        assert_eq!(seq1, 1);

        let events = journal.get_events_after(0, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1.event_id, "evt-1");
    }

    #[test]
    fn duplicate_event_is_rejected_without_creating_a_sequence_gap() {
        let mut journal = EventJournal::open_in_memory(100).unwrap();
        let event = Event {
            event_id: "evt-duplicate".into(),
            timestamp_ms: 1000,
            inner: None,
        };

        assert_eq!(journal.append_event(&event).unwrap(), 1);
        assert!(matches!(
            journal.append_event(&event),
            Err(JournalError::DuplicateEvent(id)) if id == "evt-duplicate"
        ));

        let next = Event {
            event_id: "evt-next".into(),
            timestamp_ms: 1001,
            inner: None,
        };
        assert_eq!(journal.append_event(&next).unwrap(), 2);
        assert_eq!(journal.get_events_after(0, 10).unwrap().len(), 2);
    }

    #[test]
    fn file_journal_recovers_sequence_and_latest_snapshot_after_reopen() {
        let file_name = format!(
            "muxport-event-journal-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(file_name);

        {
            let mut journal = EventJournal::open_file(&path, 100).unwrap();
            let event = Event {
                event_id: "evt-before-restart".into(),
                timestamp_ms: 1000,
                inner: None,
            };
            assert_eq!(journal.append_event(&event).unwrap(), 1);

            let snapshot = HostSnapshot {
                host_id: "host-1".into(),
                hostname: "test-host".into(),
                connector_state: 0,
                runtimes: vec![],
                credential_profiles: vec![],
                active_sessions: vec![],
                snapshot_sequence: 1,
            };
            journal.save_snapshot(&snapshot).unwrap();
        }

        {
            let mut journal = EventJournal::open_file(&path, 101).unwrap();
            assert_eq!(journal.current_sequence(), 1);
            assert_eq!(
                journal.latest_snapshot().unwrap().unwrap().host_id,
                "host-1"
            );

            let event = Event {
                event_id: "evt-after-restart".into(),
                timestamp_ms: 1001,
                inner: None,
            };
            assert_eq!(journal.append_event(&event).unwrap(), 2);
        }

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn cursor_ack_compaction_and_integrity_check_work() {
        let mut journal = EventJournal::open_in_memory(1).unwrap();
        assert!(journal.verify_integrity().unwrap());

        for i in 1..=5 {
            journal
                .append_event(&Event {
                    event_id: format!("evt-{i}"),
                    timestamp_ms: 1000 + i,
                    inner: None,
                })
                .unwrap();
        }

        journal.record_cursor_ack("phone-1", 3).unwrap();
        assert_eq!(journal.get_cursor_ack("phone-1").unwrap(), Some(3));

        let deleted = journal.compact_before_sequence(3).unwrap();
        assert_eq!(deleted, 2);

        let remaining = journal.get_events_after(2, 10).unwrap();
        assert_eq!(remaining.len(), 3);
        assert_eq!(remaining[0].0, 3);
    }
}
