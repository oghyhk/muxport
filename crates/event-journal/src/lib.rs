use muxport_protocol::{Event, HostSnapshot};
use prost::Message;
use rusqlite::{params, Connection};
use std::path::Path;
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
        Ok(())
    }

    fn load_max_sequence(&mut self) -> Result<(), JournalError> {
        let mut stmt = self.conn.prepare("SELECT COALESCE(MAX(sequence), 0) FROM events")?;
        let max_seq: u64 = stmt.query_row([], |r| r.get(0))?;
        self.current_sequence = max_seq;
        Ok(())
    }

    pub fn append_event(&mut self, event: &Event) -> Result<u64, JournalError> {
        self.current_sequence += 1;
        let seq = self.current_sequence;
        let mut payload = Vec::new();
        event.encode(&mut payload)?;

        self.conn.execute(
            "INSERT INTO events (sequence, boot_epoch, event_id, timestamp_ms, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![seq, self.boot_epoch, event.event_id, event.timestamp_ms, payload],
        )?;

        Ok(seq)
    }

    pub fn get_events_after(&self, after_seq: u64, limit: usize) -> Result<Vec<(u64, Event)>, JournalError> {
        let mut stmt = self.conn.prepare(
            "SELECT sequence, payload FROM events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_seq, limit as i64], |row| {
            let seq: u64 = row.get(0)?;
            let payload: Vec<u8> = row.get(1)?;
            Ok((seq, payload))
        })?;

        let mut result = Vec::new();
        for r in rows {
            let (seq, payload) = r?;
            let event = Event::decode(&payload[..])?;
            result.push((seq, event));
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
}
