use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use plurora_core::{EventEnvelope, EventKind, EventSequence, PackageId, SessionId};
use rusqlite::{params, Connection, TransactionBehavior, MAIN_DB};
use tokio::sync::Mutex;
use tokio::sync::{broadcast, RwLock};

/// Backend-neutral event spine contract.
///
/// `EventStore` is the kernel's append-only event log abstraction. It is
/// **not** a database abstraction: no SQL, table, vector, DSN, connection,
/// transaction-isolation, or vendor-specific concept belongs here. Every
/// backend implementation (in-memory, SQLite, future PostgreSQL, etc.)
/// must produce the same observable behaviour for the same sequence of
/// calls.
///
/// # Ordering semantics
///
/// Events within a single session are ordered by the composite key
/// `(session_id, sequence)`. Each `(session_id, sequence)` pair is unique
/// within a store instance. Cross-session ordering is best-effort
/// (timestamp-based); the contract does not guarantee global total order.
///
/// # Append paths
///
/// - **`append_with_sequence`** — the runtime-recommended append path.
///   It atomically allocates the next sequence number and appends the
///   event under the same lock/transaction, guaranteeing no duplicate or
///   gap under concurrent access to the same session.
///
/// - **`append` + `next_sequence`** — low-level / test / admin path.
///   The caller must coordinate sequence assignment. Under concurrent
///   access, separate `next_sequence` + `append` calls can produce
///   duplicates or gaps. Prefer `append_with_sequence` unless you
///   explicitly need manual control.
///
/// # Kind-prefix queries
///
/// `list_kind_prefix` and `list_session_kind_prefix` are event-semantic
/// queries: "find events whose kind starts with this prefix." They are
/// **not** SQL `LIKE`, not index product API, and not vector search.
/// Backend implementations may use pushdown (SQLite `LIKE` / range scan)
/// or in-memory filtering; the observable result must be identical.
///
/// # No database concepts
///
/// This trait must never expose SQL, table, DSN, connection string,
/// credentials, file path, WAL mode, isolation level, vector dimension,
/// ANN index, embedding model, or any other concept specific to a
/// particular storage product. Such details belong to backend
/// constructors (`SqliteEventStore::open`, future
/// `PostgresEventStore::connect`, etc.), not to the event spine contract.
#[async_trait]
pub trait EventStore: Send + Sync + 'static {
    /// Low-level append: store a pre-constructed event envelope.
    /// Prefer `append_with_sequence` for runtime use; this method
    /// exists for replay, admin tooling, and test fixtures.
    async fn append(&self, event: EventEnvelope) -> anyhow::Result<()>;

    /// Whether this backend can atomically append a pre-validated event batch.
    /// Portable imports use the stronger empty-session operation below.
    fn supports_atomic_batch_append(&self) -> bool {
        false
    }

    /// Atomically append pre-constructed envelopes or fail without appending any.
    /// Backends that do not implement a real transaction must leave the default
    /// fail-closed behavior in place.
    async fn append_batch_atomic(&self, _events: &[EventEnvelope]) -> anyhow::Result<()> {
        anyhow::bail!("event store does not support atomic batch append")
    }

    /// Whether this backend can atomically require a set of sessions to be empty
    /// and append a pre-validated batch in the same transaction.
    fn supports_atomic_empty_session_batch_append(&self) -> bool {
        false
    }

    /// Atomically reject when any required session already contains an event, or
    /// append the complete batch. Portable imports use this stronger operation so
    /// an emptiness check cannot race with another writer.
    async fn append_batch_atomic_if_sessions_empty(
        &self,
        _events: &[EventEnvelope],
        _required_empty_sessions: &[SessionId],
    ) -> anyhow::Result<()> {
        anyhow::bail!("event store does not support atomic empty-session batch append")
    }

    /// List all events across all sessions, ordered by
    /// `(timestamp, session_id, sequence)`.
    async fn list_all(&self) -> anyhow::Result<Vec<EventEnvelope>>;

    /// List all events within a session, ordered by sequence.
    async fn list_session(&self, session_id: &SessionId) -> anyhow::Result<Vec<EventEnvelope>>;

    /// List events within a session after a given sequence, with
    /// optional limit. This is the range-replay primitive.
    async fn list_session_range(
        &self,
        session_id: &SessionId,
        after_sequence: Option<EventSequence>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<EventEnvelope>>;

    /// Return the next sequence number for a session.
    /// Low-level: prefer `append_with_sequence` for concurrent safety.
    async fn next_sequence(&self, session_id: &SessionId) -> anyhow::Result<EventSequence>;

    /// Subscribe to a broadcast channel of newly appended events.
    /// Backend-neutral: works identically for in-memory and durable stores.
    fn subscribe(&self) -> broadcast::Receiver<EventEnvelope>;

    /// **Runtime-recommended append path.** Atomically append an event,
    /// assigning the next sequence number within the same lock/transaction.
    /// This guarantees that concurrent appends to the same session receive
    /// contiguous, non-repeating sequence numbers without requiring the
    /// caller to call `next_sequence` separately.
    ///
    /// # Ordering guarantee
    ///
    /// Per-session `(session_id, sequence)` uniqueness is guaranteed.
    /// No two events in the same session will share a sequence number,
    /// and sequences are contiguous from 0.
    ///
    /// # When to use vs `append` + `next_sequence`
    ///
    /// - Use `append_with_sequence` in all runtime paths where
    ///   concurrent access is possible.
    /// - Use `append` + `next_sequence` only for single-writer
    ///   replay, admin tooling, or test fixtures where you control
    ///   all writers.
    ///
    /// The default implementation falls back to `next_sequence` + `append`,
    /// which is correct for single-writer or low-contention use but does
    /// not guarantee atomicity under concurrent access. Backend
    /// implementations that support atomic operations (e.g. SQLite with
    /// its connection mutex) override this with a truly atomic path.
    async fn append_with_sequence(
        &self,
        session_id: SessionId,
        writer_package_id: PackageId,
        kind: EventKind,
        schema_version: u16,
        payload_json: serde_json::Value,
        metadata_json: serde_json::Value,
    ) -> anyhow::Result<EventEnvelope> {
        let sequence = self.next_sequence(&session_id).await?;
        let event = EventEnvelope {
            id: plurora_core::new_id("evt"),
            session_id,
            sequence,
            timestamp: chrono::Utc::now(),
            writer_package_id,
            kind,
            schema_version,
            payload: payload_json,
            metadata: metadata_json,
        };
        self.append(event.clone()).await?;
        Ok(event)
    }

    /// Atomically append only when the session's next sequence still equals
    /// `expected_next_sequence`.
    ///
    /// This is the event-spine compare-and-append primitive used by durable
    /// control planes. `Ok(None)` means another writer advanced the session;
    /// no event was appended. Backends must override this method with a real
    /// lock or transaction. The default fails closed rather than emulating a
    /// racy `next_sequence` followed by `append`.
    async fn append_with_sequence_if_next(
        &self,
        _session_id: SessionId,
        _expected_next_sequence: EventSequence,
        _writer_package_id: PackageId,
        _kind: EventKind,
        _schema_version: u16,
        _payload_json: serde_json::Value,
        _metadata_json: serde_json::Value,
    ) -> anyhow::Result<Option<EventEnvelope>> {
        anyhow::bail!("event store does not support atomic compare-and-append")
    }

    /// **Event-semantic kind-prefix query.** List events whose `kind`
    /// starts with `prefix`, across all sessions. Results are ordered
    /// by `(timestamp, session_id, sequence)`.
    ///
    /// This is an event-level query: "find all events matching this
    /// kind prefix." It is **not** a SQL `LIKE` query, not an index
    /// product API, and not a vector/embedding search. Backend
    /// implementations may use pushdown (range scan, index, etc.)
    /// for performance, but the observable result set must match
    /// the in-memory baseline exactly.
    async fn list_kind_prefix(&self, prefix: &str) -> anyhow::Result<Vec<EventEnvelope>> {
        let all = self.list_all().await?;
        Ok(all
            .into_iter()
            .filter(|e| e.kind.starts_with(prefix))
            .collect())
    }

    /// **Event-semantic kind-prefix query within a session.** List events
    /// within a session whose `kind` starts with `prefix`. Results are
    /// ordered by sequence.
    ///
    /// Same contract as `list_kind_prefix` but scoped to a single session.
    /// Backend implementations may use session-scoped pushdown; the
    /// observable result set must match the in-memory baseline.
    async fn list_session_kind_prefix(
        &self,
        session_id: &SessionId,
        prefix: &str,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let events = self.list_session(session_id).await?;
        Ok(events
            .into_iter()
            .filter(|e| e.kind.starts_with(prefix))
            .collect())
    }
}

#[derive(Clone)]
pub struct SqliteEventStore {
    conn: Arc<Mutex<Connection>>,
    tx: broadcast::Sender<EventEnvelope>,
}

impl SqliteEventStore {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        init_schema(&conn)?;
        let (tx, _) = broadcast::channel(256);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            tx,
        })
    }

    /// Create a transactionally consistent SQLite snapshot without exposing
    /// SQLite details through the backend-neutral [`EventStore`] contract.
    pub async fn backup_to(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        anyhow::ensure!(
            !path.exists(),
            "event store backup destination already exists"
        );
        self.conn.lock().await.backup(MAIN_DB, path, None)?;
        Ok(())
    }

    /// Verify that SQLite can read every page and index in this event store.
    pub async fn verify_integrity(&self) -> anyhow::Result<()> {
        let result: String =
            self.conn
                .lock()
                .await
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        anyhow::ensure!(result == "ok", "event store integrity check failed");
        Ok(())
    }
}

fn init_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS events (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          sequence INTEGER NOT NULL,
          timestamp TEXT NOT NULL,
          writer_package_id TEXT NOT NULL,
          kind TEXT NOT NULL,
          schema_version INTEGER NOT NULL,
          payload_json TEXT NOT NULL,
          metadata_json TEXT NOT NULL,
          UNIQUE(session_id, sequence)
        );
        CREATE INDEX IF NOT EXISTS idx_events_session_sequence ON events(session_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);
        CREATE INDEX IF NOT EXISTS idx_events_session_kind_sequence ON events(session_id, kind, sequence);
        "#,
    )?;
    Ok(())
}

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn append(&self, event: EventEnvelope) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&event.payload)?;
        let metadata = serde_json::to_string(&event.metadata)?;
        self.conn.lock().await.execute(
            "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.id,
                event.session_id,
                event.sequence as i64,
                event.timestamp.to_rfc3339(),
                event.writer_package_id,
                event.kind,
                event.schema_version as i64,
                payload,
                metadata,
            ],
        )?;
        let _ = self.tx.send(event);
        Ok(())
    }

    fn supports_atomic_batch_append(&self) -> bool {
        true
    }

    async fn append_batch_atomic(&self, events: &[EventEnvelope]) -> anyhow::Result<()> {
        self.append_batch_atomic_if_sessions_empty(events, &[])
            .await
    }

    fn supports_atomic_empty_session_batch_append(&self) -> bool {
        true
    }

    async fn append_batch_atomic_if_sessions_empty(
        &self,
        events: &[EventEnvelope],
        required_empty_sessions: &[SessionId],
    ) -> anyhow::Result<()> {
        let serialized = events
            .iter()
            .map(|event| {
                Ok((
                    event,
                    serde_json::to_string(&event.payload)?,
                    serde_json::to_string(&event.metadata)?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut conn = self.conn.lock().await;
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for session_id in required_empty_sessions {
            let contains_event: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE session_id = ?1)",
                params![session_id],
                |row| row.get(0),
            )?;
            anyhow::ensure!(
                !contains_event,
                "event batch requires empty session '{session_id}'"
            );
        }
        for (event, payload, metadata) in serialized {
            transaction.execute(
                "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    event.id,
                    event.session_id,
                    event.sequence as i64,
                    event.timestamp.to_rfc3339(),
                    event.writer_package_id,
                    event.kind,
                    event.schema_version as i64,
                    payload,
                    metadata,
                ],
            )?;
        }
        transaction.commit()?;
        for event in events {
            let _ = self.tx.send(event.clone());
        }
        Ok(())
    }

    async fn list_session(&self, session_id: &SessionId) -> anyhow::Result<Vec<EventEnvelope>> {
        self.list_session_range(session_id, None, None).await
    }

    async fn list_all(&self) -> anyhow::Result<Vec<EventEnvelope>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
             FROM events ORDER BY timestamp ASC, session_id ASC, sequence ASC",
        )?;
        let rows = stmt.query_map([], row_to_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    async fn list_session_range(
        &self,
        session_id: &SessionId,
        after_sequence: Option<EventSequence>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let conn = self.conn.lock().await;
        let after_sequence = after_sequence.map(|sequence| sequence as i64).unwrap_or(-1);
        let limit = limit.unwrap_or(1_000).min(10_000) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
             FROM events WHERE session_id = ?1 AND sequence > ?2 ORDER BY sequence ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![session_id, after_sequence, limit], row_to_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    async fn next_sequence(&self, session_id: &SessionId) -> anyhow::Result<EventSequence> {
        let next: i64 = self.conn.lock().await.query_row(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM events WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(next as EventSequence)
    }

    fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.tx.subscribe()
    }

    /// Atomically append: read max sequence and insert within the same
    /// connection mutex, guaranteeing no sequence gap or duplicate under
    /// concurrent access to the same session.
    async fn append_with_sequence(
        &self,
        session_id: SessionId,
        writer_package_id: PackageId,
        kind: EventKind,
        schema_version: u16,
        payload_json: serde_json::Value,
        metadata_json: serde_json::Value,
    ) -> anyhow::Result<EventEnvelope> {
        let conn = self.conn.lock().await;
        let next_seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM events WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        let event = EventEnvelope {
            id: plurora_core::new_id("evt"),
            session_id,
            sequence: next_seq as EventSequence,
            timestamp: chrono::Utc::now(),
            writer_package_id,
            kind,
            schema_version,
            payload: payload_json,
            metadata: metadata_json,
        };
        let payload = serde_json::to_string(&event.payload)?;
        let metadata = serde_json::to_string(&event.metadata)?;
        conn.execute(
            "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.id,
                event.session_id,
                event.sequence as i64,
                event.timestamp.to_rfc3339(),
                event.writer_package_id,
                event.kind,
                event.schema_version as i64,
                payload,
                metadata,
            ],
        )?;
        // Broadcast outside the lock is not strictly required for correctness,
        // but we hold the lock here to guarantee the event is visible before
        // subscribers are notified.
        let _ = self.tx.send(event.clone());
        Ok(event)
    }

    async fn append_with_sequence_if_next(
        &self,
        session_id: SessionId,
        expected_next_sequence: EventSequence,
        writer_package_id: PackageId,
        kind: EventKind,
        schema_version: u16,
        payload_json: serde_json::Value,
        metadata_json: serde_json::Value,
    ) -> anyhow::Result<Option<EventEnvelope>> {
        let expected_next_sequence = i64::try_from(expected_next_sequence)
            .map_err(|_| anyhow::anyhow!("expected event sequence exceeds backend range"))?;
        let mut conn = self.conn.lock().await;
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let next_seq: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM events WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        if next_seq != expected_next_sequence {
            return Ok(None);
        }
        let event = EventEnvelope {
            id: plurora_core::new_id("evt"),
            session_id,
            sequence: next_seq as EventSequence,
            timestamp: chrono::Utc::now(),
            writer_package_id,
            kind,
            schema_version,
            payload: payload_json,
            metadata: metadata_json,
        };
        let payload = serde_json::to_string(&event.payload)?;
        let metadata = serde_json::to_string(&event.metadata)?;
        transaction.execute(
            "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)\n             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.id,
                event.session_id,
                event.sequence as i64,
                event.timestamp.to_rfc3339(),
                event.writer_package_id,
                event.kind,
                event.schema_version as i64,
                payload,
                metadata,
            ],
        )?;
        transaction.commit()?;
        let _ = self.tx.send(event.clone());
        Ok(Some(event))
    }

    /// SQLite pushdown: uses `LIKE` with an upper bound to avoid a
    /// full-table scan when filtering by kind prefix.
    async fn list_kind_prefix(&self, prefix: &str) -> anyhow::Result<Vec<EventEnvelope>> {
        let conn = self.conn.lock().await;
        let prefix_end = kind_prefix_upper_bound(prefix);
        let mut stmt = if prefix_end.is_some() {
            conn.prepare(
                "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                 FROM events WHERE kind >= ?1 AND kind < ?2
                 ORDER BY timestamp ASC, session_id ASC, sequence ASC",
            )?
        } else {
            // Prefix with no upper bound (e.g. single-char or all-0xFF prefix);
            // fall back to LIKE-only.
            conn.prepare(
                "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                 FROM events WHERE kind LIKE ?1
                 ORDER BY timestamp ASC, session_id ASC, sequence ASC",
            )?
        };
        let rows = if let Some(end) = prefix_end {
            stmt.query_map(params![prefix, end], row_to_event)?
        } else {
            stmt.query_map(params![format!("{}%", prefix)], row_to_event)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// SQLite pushdown: uses `LIKE` with session + kind within a single
    /// session to avoid full-session scan.
    async fn list_session_kind_prefix(
        &self,
        session_id: &SessionId,
        prefix: &str,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let conn = self.conn.lock().await;
        let prefix_end = kind_prefix_upper_bound(prefix);
        let mut stmt = if prefix_end.is_some() {
            conn.prepare(
                "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                 FROM events WHERE session_id = ?1 AND kind >= ?2 AND kind < ?3
                 ORDER BY sequence ASC",
            )?
        } else {
            conn.prepare(
                "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                 FROM events WHERE session_id = ?1 AND kind LIKE ?2
                 ORDER BY sequence ASC",
            )?
        };
        let rows = if let Some(end) = prefix_end {
            stmt.query_map(params![session_id, prefix, end], row_to_event)?
        } else {
            stmt.query_map(params![session_id, format!("{}%", prefix)], row_to_event)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Compute an upper-bound string for a kind prefix so that a range
/// query `kind >= prefix AND kind < upper` is equivalent to a prefix
/// match without the `%` wild-card cost of LIKE.
///
/// Returns `None` if the upper bound cannot be computed (e.g. prefix
/// consists entirely of maximal chars `\xff`).
fn kind_prefix_upper_bound(prefix: &str) -> Option<String> {
    let mut bytes: Vec<u8> = prefix.as_bytes().to_vec();
    loop {
        match bytes.last_mut() {
            Some(b) if *b < 0xFF => {
                *b += 1;
                return Some(String::from_utf8(bytes).unwrap_or_default());
            }
            Some(_) => {
                bytes.pop();
            }
            None => return None,
        }
    }
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventEnvelope> {
    let timestamp: String = row.get(3)?;
    let payload_json: String = row.get(7)?;
    let metadata_json: String = row.get(8)?;
    let sequence: i64 = row.get(2)?;
    let schema_version: i64 = row.get(6)?;
    Ok(EventEnvelope {
        id: row.get(0)?,
        session_id: row.get(1)?,
        sequence: sequence as EventSequence,
        timestamp: chrono::DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
        writer_package_id: row.get::<_, PackageId>(4)?,
        kind: row.get(5)?,
        schema_version: schema_version as u16,
        payload: serde_json::from_str(&payload_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
        })?,
        metadata: serde_json::from_str(&metadata_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
        })?,
    })
}

#[cfg(test)]
mod sqlite_tests {
    use plurora_core::{new_id, EventEnvelope, KERNEL_PACKAGE_ID};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn sqlite_store_persists_and_replays_events() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("plurora-test-{}.db", new_id("sqlite")));
        let store = SqliteEventStore::open(&path)?;
        let session_id = "ses_test".to_string();
        store
            .append(EventEnvelope::new(
                new_id("evt"),
                session_id.clone(),
                0,
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/session.opened",
                json!({"ok": true}),
            ))
            .await?;
        drop(store);

        let reopened = SqliteEventStore::open(&path)?;
        let events = reopened.list_session(&session_id).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(reopened.next_sequence(&session_id).await?, 1);
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_concurrent_append_no_duplicate_sequences() -> anyhow::Result<()> {
        let path =
            std::env::temp_dir().join(format!("plurora-test-concurrent-{}.db", new_id("sqlite")));
        let store = SqliteEventStore::open(&path)?;
        let session_id = "ses_concurrent".to_string();

        // Open the session by appending a session.opened event
        store
            .append_with_sequence(
                session_id.clone(),
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/session.opened".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;

        let n = 50;
        let mut handles = Vec::new();
        for i in 0..n {
            let s = store.clone();
            let sid = session_id.clone();
            handles.push(tokio::spawn(async move {
                s.append_with_sequence(
                    sid,
                    KERNEL_PACKAGE_ID.to_string(),
                    format!("kernel/v1/test.concurrent.{}", i),
                    1,
                    json!({"i": i}),
                    json!({}),
                )
                .await
            }));
        }

        for h in handles {
            let _ = h.await.unwrap()?;
        }

        let events = store.list_session(&session_id).await?;
        let mut sequences: Vec<u64> = events.iter().map(|e| e.sequence).collect();
        sequences.sort();
        // No duplicates
        let dedup: Vec<u64> = sequences
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(
            dedup.len(),
            sequences.len(),
            "duplicate sequences found: {:?}",
            sequences
        );
        // Contiguous from 0
        for (i, seq) in sequences.iter().enumerate() {
            assert_eq!(
                *seq, i as u64,
                "non-contiguous sequence at index {}: {}",
                i, seq
            );
        }

        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_compare_and_append_rejects_stale_tail() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "plurora-test-compare-append-{}.db",
            new_id("sqlite")
        ));
        let store = SqliteEventStore::open(&path)?;
        let session_id = "ses_compare_append".to_string();

        let first = store
            .append_with_sequence_if_next(
                session_id.clone(),
                0,
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/test.compare".to_string(),
                1,
                json!({"writer": 1}),
                json!({}),
            )
            .await?;
        assert_eq!(first.as_ref().map(|event| event.sequence), Some(0));
        assert!(store
            .append_with_sequence_if_next(
                session_id.clone(),
                0,
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/test.stale".to_string(),
                1,
                json!({"writer": 2}),
                json!({}),
            )
            .await?
            .is_none());
        assert_eq!(store.list_session(&session_id).await?.len(), 1);

        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_kind_prefix_query_uses_pushdown() -> anyhow::Result<()> {
        let path =
            std::env::temp_dir().join(format!("plurora-test-prefix-{}.db", new_id("sqlite")));
        let store = SqliteEventStore::open(&path)?;
        let session_id = "ses_prefix".to_string();

        store
            .append_with_sequence(
                session_id.clone(),
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/permission.granted".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;
        store
            .append_with_sequence(
                session_id.clone(),
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/permission.denied".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;
        store
            .append_with_sequence(
                session_id.clone(),
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/session.opened".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;

        let perm_events = store.list_kind_prefix("kernel/v1/permission").await?;
        assert_eq!(perm_events.len(), 2);

        let session_perm = store
            .list_session_kind_prefix(&session_id, "kernel/v1/permission")
            .await?;
        assert_eq!(session_perm.len(), 2);

        let _ = std::fs::remove_file(path);
        Ok(())
    }
}

#[derive(Clone)]
pub struct InMemoryEventStore {
    events: Arc<RwLock<HashMap<SessionId, Vec<EventEnvelope>>>>,
    tx: broadcast::Sender<EventEnvelope>,
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            events: Arc::new(RwLock::new(HashMap::new())),
            tx,
        }
    }
}

fn in_memory_next_sequence(events: Option<&Vec<EventEnvelope>>) -> anyhow::Result<EventSequence> {
    events
        .and_then(|items| items.iter().map(|event| event.sequence).max())
        .map(|sequence| {
            sequence
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("event sequence overflow"))
        })
        .transpose()
        .map(|sequence| sequence.unwrap_or(0))
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append(&self, event: EventEnvelope) -> anyhow::Result<()> {
        let mut map = self.events.write().await;
        anyhow::ensure!(
            !map.values()
                .flatten()
                .any(|existing| existing.id == event.id),
            "duplicate event id '{}'",
            event.id
        );
        let session = map.entry(event.session_id.clone()).or_default();
        anyhow::ensure!(
            !session
                .iter()
                .any(|existing| existing.sequence == event.sequence),
            "duplicate event position '{}:{}'",
            event.session_id,
            event.sequence
        );
        session.push(event.clone());
        session.sort_by_key(|item| item.sequence);
        drop(map);
        let _ = self.tx.send(event);
        Ok(())
    }

    fn supports_atomic_batch_append(&self) -> bool {
        true
    }

    async fn append_batch_atomic(&self, events: &[EventEnvelope]) -> anyhow::Result<()> {
        self.append_batch_atomic_if_sessions_empty(events, &[])
            .await
    }

    fn supports_atomic_empty_session_batch_append(&self) -> bool {
        true
    }

    async fn append_batch_atomic_if_sessions_empty(
        &self,
        events: &[EventEnvelope],
        required_empty_sessions: &[SessionId],
    ) -> anyhow::Result<()> {
        let mut map = self.events.write().await;
        for session_id in required_empty_sessions {
            anyhow::ensure!(
                map.get(session_id).is_none_or(Vec::is_empty),
                "event batch requires empty session '{session_id}'"
            );
        }
        let mut candidate = map.clone();
        let mut ids = candidate
            .values()
            .flat_map(|events| events.iter().map(|event| event.id.clone()))
            .collect::<std::collections::BTreeSet<_>>();
        for event in events {
            anyhow::ensure!(
                ids.insert(event.id.clone()),
                "duplicate event id '{}'",
                event.id
            );
            let session = candidate.entry(event.session_id.clone()).or_default();
            anyhow::ensure!(
                !session
                    .iter()
                    .any(|existing| existing.sequence == event.sequence),
                "duplicate event position '{}:{}'",
                event.session_id,
                event.sequence
            );
            session.push(event.clone());
        }
        for events in candidate.values_mut() {
            events.sort_by_key(|event| event.sequence);
        }
        *map = candidate;
        drop(map);
        for event in events {
            let _ = self.tx.send(event.clone());
        }
        Ok(())
    }

    async fn list_session(&self, session_id: &SessionId) -> anyhow::Result<Vec<EventEnvelope>> {
        self.list_session_range(session_id, None, None).await
    }

    async fn list_all(&self) -> anyhow::Result<Vec<EventEnvelope>> {
        let mut events: Vec<_> = self
            .events
            .read()
            .await
            .values()
            .flat_map(|events| events.clone())
            .collect();
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.session_id.cmp(&b.session_id))
                .then(a.sequence.cmp(&b.sequence))
        });
        Ok(events)
    }

    async fn list_session_range(
        &self,
        session_id: &SessionId,
        after_sequence: Option<EventSequence>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let after_sequence = after_sequence;
        let mut events: Vec<_> = self
            .events
            .read()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| {
                after_sequence
                    .map(|sequence| event.sequence > sequence)
                    .unwrap_or(true)
            })
            .collect();
        events.sort_by_key(|event| event.sequence);
        if let Some(limit) = limit {
            events.truncate(limit);
        }
        Ok(events)
    }

    async fn next_sequence(&self, session_id: &SessionId) -> anyhow::Result<EventSequence> {
        let map = self.events.read().await;
        in_memory_next_sequence(map.get(session_id))
    }

    fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.tx.subscribe()
    }

    /// In-memory atomic append: allocate sequence and push within the
    /// same write lock, guaranteeing no duplicate sequences under
    /// concurrent access.
    async fn append_with_sequence(
        &self,
        session_id: SessionId,
        writer_package_id: PackageId,
        kind: EventKind,
        schema_version: u16,
        payload_json: serde_json::Value,
        metadata_json: serde_json::Value,
    ) -> anyhow::Result<EventEnvelope> {
        let mut map = self.events.write().await;
        let seq = in_memory_next_sequence(map.get(&session_id))?;
        let event = EventEnvelope {
            id: plurora_core::new_id("evt"),
            session_id,
            sequence: seq,
            timestamp: chrono::Utc::now(),
            writer_package_id,
            kind,
            schema_version,
            payload: payload_json,
            metadata: metadata_json,
        };
        map.entry(event.session_id.clone())
            .or_default()
            .push(event.clone());
        let _ = self.tx.send(event.clone());
        Ok(event)
    }

    async fn append_with_sequence_if_next(
        &self,
        session_id: SessionId,
        expected_next_sequence: EventSequence,
        writer_package_id: PackageId,
        kind: EventKind,
        schema_version: u16,
        payload_json: serde_json::Value,
        metadata_json: serde_json::Value,
    ) -> anyhow::Result<Option<EventEnvelope>> {
        let mut map = self.events.write().await;
        let next_sequence = in_memory_next_sequence(map.get(&session_id))?;
        if next_sequence != expected_next_sequence {
            return Ok(None);
        }
        let event = EventEnvelope {
            id: plurora_core::new_id("evt"),
            session_id,
            sequence: next_sequence,
            timestamp: chrono::Utc::now(),
            writer_package_id,
            kind,
            schema_version,
            payload: payload_json,
            metadata: metadata_json,
        };
        map.entry(event.session_id.clone())
            .or_default()
            .push(event.clone());
        drop(map);
        let _ = self.tx.send(event.clone());
        Ok(Some(event))
    }

    /// In-memory pushdown: filter by kind prefix within a single
    /// session read, avoiding full list_all().
    async fn list_session_kind_prefix(
        &self,
        session_id: &SessionId,
        prefix: &str,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let events = self
            .events
            .read()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.kind.starts_with(prefix))
            .collect();
        Ok(events)
    }
}

#[cfg(test)]
mod in_memory_compare_append_tests {
    use plurora_core::KERNEL_PACKAGE_ID;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn in_memory_compare_and_append_has_single_winner() -> anyhow::Result<()> {
        let store = InMemoryEventStore::default();
        let session_id = "ses_compare_append".to_string();
        let first = store
            .append_with_sequence_if_next(
                session_id.clone(),
                0,
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/test.compare".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;
        let stale = store
            .append_with_sequence_if_next(
                session_id.clone(),
                0,
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/test.stale".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;
        assert!(first.is_some());
        assert!(stale.is_none());
        assert_eq!(store.list_session(&session_id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn in_memory_compare_and_append_is_atomic_for_concurrent_contenders() -> anyhow::Result<()>
    {
        let store = InMemoryEventStore::default();
        let session_id = "ses_concurrent_compare_append".to_string();
        let append = |kind: &'static str| {
            let store = store.clone();
            let session_id = session_id.clone();
            async move {
                store
                    .append_with_sequence_if_next(
                        session_id,
                        0,
                        KERNEL_PACKAGE_ID.to_string(),
                        kind.to_string(),
                        1,
                        json!({}),
                        json!({}),
                    )
                    .await
            }
        };
        let (left, right) = tokio::join!(
            append("kernel/v1/test.left"),
            append("kernel/v1/test.right")
        );
        let winners = [left?, right?].into_iter().filter(Option::is_some).count();
        assert_eq!(winners, 1);
        assert_eq!(store.list_session(&session_id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn in_memory_compare_and_append_uses_maximum_sequence_tail() -> anyhow::Result<()> {
        let store = InMemoryEventStore::default();
        let session_id = "ses_sparse_compare_append".to_string();
        store
            .append(EventEnvelope {
                id: "evt-sparse-tail".to_string(),
                session_id: session_id.clone(),
                sequence: 10,
                timestamp: chrono::Utc::now(),
                writer_package_id: KERNEL_PACKAGE_ID.to_string(),
                kind: "kernel/v1/test.sparse".to_string(),
                schema_version: 1,
                payload: json!({}),
                metadata: json!({}),
            })
            .await?;
        assert_eq!(store.next_sequence(&session_id).await?, 11);
        assert!(store
            .append_with_sequence_if_next(
                session_id,
                1,
                KERNEL_PACKAGE_ID.to_string(),
                "kernel/v1/test.stale".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?
            .is_none());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PostgresEventStore (feature-gated, opt-in)
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
mod postgres_backend {
    use super::*;

    use deadpool_postgres::Pool;
    use tokio_postgres::types::ToSql;

    /// PostgreSQL-backed `EventStore` implementation.
    ///
    /// This is an **opt-in** backend. It is not compiled unless the
    /// `postgres` feature is enabled, and it never affects the default
    /// build. Connection details (DSN, user, password) are accepted by
    /// the constructor only and are never written to events, proposals,
    /// logs, or public diagnostics.
    ///
    /// # Concurrent sequence guarantee
    ///
    /// `append_with_sequence` uses `pg_advisory_xact_lock` on a hash of
    /// the session_id to serialise per-session appends within a
    /// transaction, then selects `max(sequence)+1` and inserts —
    /// guaranteeing contiguous, non-repeating sequence numbers without
    /// relying on PostgreSQL sequences (which do not guarantee gapless
    /// numbering).
    #[derive(Clone)]
    pub struct PostgresEventStore {
        pool: Pool,
        tx: broadcast::Sender<EventEnvelope>,
    }

    impl PostgresEventStore {
        /// Connect to a PostgreSQL database and initialise the event
        /// store schema. `database_url` is consumed here and never
        /// stored beyond the pool internals; it is never written to
        /// events, proposals, logs, or public output.
        pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
            let pg_config: tokio_postgres::Config = database_url
                .parse()
                .map_err(|_e| anyhow::anyhow!("postgres config parse error (details redacted)"))?;
            let mgr = deadpool_postgres::Manager::new(pg_config, tokio_postgres::NoTls);
            let pool = Pool::builder(mgr)
                .max_size(5)
                .build()
                .map_err(|_e| anyhow::anyhow!("postgres pool create error (details redacted)"))?;

            // Verify connectivity and init schema
            let conn = pool
                .get()
                .await
                .map_err(|_e| anyhow::anyhow!("postgres connect error (details redacted)"))?;
            conn.batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS events (
                  id TEXT PRIMARY KEY,
                  session_id TEXT NOT NULL,
                  sequence BIGINT NOT NULL,
                  timestamp TEXT NOT NULL,
                  writer_package_id TEXT NOT NULL,
                  kind TEXT NOT NULL,
                  schema_version INTEGER NOT NULL,
                  payload_json JSONB NOT NULL,
                  metadata_json JSONB NOT NULL,
                  UNIQUE(session_id, sequence)
                );
                CREATE INDEX IF NOT EXISTS idx_events_session_sequence ON events(session_id, sequence);
                CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);
                CREATE INDEX IF NOT EXISTS idx_events_session_kind_sequence ON events(session_id, kind, sequence);
                "#,
            )
            .await
            .map_err(|_e| {
                anyhow::anyhow!("postgres schema init error (details redacted)")
            })?;

            let (tx, _) = broadcast::channel(256);
            Ok(Self { pool, tx })
        }
    }

    /// Helper to redact all postgres errors.
    fn redact_pg(_e: impl std::fmt::Debug) -> anyhow::Error {
        anyhow::anyhow!("postgres error (details redacted)")
    }

    /// Map a row to EventEnvelope.
    fn row_to_event(row: &tokio_postgres::Row) -> anyhow::Result<EventEnvelope> {
        let id: String = row.try_get(0)?;
        let session_id: String = row.try_get(1)?;
        let sequence: i64 = row.try_get(2)?;
        let timestamp_str: String = row.try_get(3)?;
        let writer_package_id: String = row.try_get(4)?;
        let kind: String = row.try_get(5)?;
        let schema_version: i32 = row.try_get(6)?;
        let payload_json: serde_json::Value = row.try_get(7)?;
        let metadata_json: serde_json::Value = row.try_get(8)?;

        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_e| anyhow::anyhow!("timestamp parse error"))?;

        Ok(EventEnvelope {
            id,
            session_id,
            sequence: sequence as EventSequence,
            timestamp,
            writer_package_id,
            kind,
            schema_version: schema_version as u16,
            payload: payload_json,
            metadata: metadata_json,
        })
    }

    #[async_trait]
    impl EventStore for PostgresEventStore {
        async fn append(&self, event: EventEnvelope) -> anyhow::Result<()> {
            let mut conn = self.pool.get().await.map_err(redact_pg)?;
            let tx = conn.transaction().await.map_err(redact_pg)?;
            tx.execute(
                "SELECT pg_advisory_xact_lock(hashtext($1))",
                &[&event.session_id],
            )
            .await
            .map_err(redact_pg)?;
            let payload = serde_json::to_string(&event.payload)?;
            let metadata = serde_json::to_string(&event.metadata)?;
            tx.execute(
                "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::jsonb)",
                &[
                    &event.id as &(dyn ToSql + Sync),
                    &event.session_id,
                    &(event.sequence as i64),
                    &event.timestamp.to_rfc3339(),
                    &event.writer_package_id,
                    &event.kind,
                    &(event.schema_version as i32),
                    &payload,
                    &metadata,
                ],
            )
            .await
            .map_err(redact_pg)?;
            tx.commit().await.map_err(redact_pg)?;
            let _ = self.tx.send(event);
            Ok(())
        }

        async fn list_session(&self, session_id: &SessionId) -> anyhow::Result<Vec<EventEnvelope>> {
            self.list_session_range(session_id, None, None).await
        }

        async fn list_all(&self) -> anyhow::Result<Vec<EventEnvelope>> {
            let conn = self.pool.get().await.map_err(redact_pg)?;
            let rows = conn
                .query(
                    "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                     FROM events ORDER BY timestamp ASC, session_id ASC, sequence ASC",
                    &[],
                )
                .await
                .map_err(redact_pg)?;
            rows.iter().map(|r| row_to_event(r)).collect()
        }

        async fn list_session_range(
            &self,
            session_id: &SessionId,
            after_sequence: Option<EventSequence>,
            limit: Option<usize>,
        ) -> anyhow::Result<Vec<EventEnvelope>> {
            let after = after_sequence.map(|s| s as i64).unwrap_or(-1);
            let lim = limit.unwrap_or(1_000).min(10_000) as i64;
            let conn = self.pool.get().await.map_err(redact_pg)?;
            let rows = conn
                .query(
                    "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                     FROM events WHERE session_id = $1 AND sequence > $2 ORDER BY sequence ASC LIMIT $3",
                    &[session_id, &after, &lim],
                )
                .await
                .map_err(redact_pg)?;
            rows.iter().map(|r| row_to_event(r)).collect()
        }

        async fn next_sequence(&self, session_id: &SessionId) -> anyhow::Result<EventSequence> {
            let conn = self.pool.get().await.map_err(redact_pg)?;
            let row = conn
                .query_one(
                    "SELECT COALESCE(MAX(sequence) + 1, 0) FROM events WHERE session_id = $1",
                    &[session_id],
                )
                .await
                .map_err(redact_pg)?;
            let next: i64 = row.try_get(0)?;
            Ok(next as EventSequence)
        }

        fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
            self.tx.subscribe()
        }

        /// Atomically append: take a session-scoped advisory transaction
        /// lock, read `max(sequence)+1`, and insert within the same
        /// transaction. This guarantees no duplicate or gap in per-session
        /// sequence numbers under concurrent access.
        async fn append_with_sequence(
            &self,
            session_id: SessionId,
            writer_package_id: PackageId,
            kind: EventKind,
            schema_version: u16,
            payload_json: serde_json::Value,
            metadata_json: serde_json::Value,
        ) -> anyhow::Result<EventEnvelope> {
            let mut conn = self.pool.get().await.map_err(redact_pg)?;
            let tx = conn.transaction().await.map_err(redact_pg)?;

            // Acquire session-scoped advisory lock (released on commit/rollback)
            tx.execute("SELECT pg_advisory_xact_lock(hashtext($1))", &[&session_id])
                .await
                .map_err(redact_pg)?;

            let row = tx
                .query_one(
                    "SELECT COALESCE(MAX(sequence) + 1, 0) FROM events WHERE session_id = $1",
                    &[&session_id],
                )
                .await
                .map_err(redact_pg)?;
            let next_seq: i64 = row.try_get(0)?;

            let event = EventEnvelope {
                id: plurora_core::new_id("evt"),
                session_id,
                sequence: next_seq as EventSequence,
                timestamp: chrono::Utc::now(),
                writer_package_id,
                kind,
                schema_version,
                payload: payload_json,
                metadata: metadata_json,
            };

            let payload = serde_json::to_string(&event.payload)?;
            let metadata = serde_json::to_string(&event.metadata)?;

            tx.execute(
                "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::jsonb)",
                &[
                    &event.id as &(dyn ToSql + Sync),
                    &event.session_id,
                    &(event.sequence as i64),
                    &event.timestamp.to_rfc3339(),
                    &event.writer_package_id,
                    &event.kind,
                    &(event.schema_version as i32),
                    &payload,
                    &metadata,
                ],
            )
            .await
            .map_err(redact_pg)?;

            tx.commit().await.map_err(redact_pg)?;

            let _ = self.tx.send(event.clone());
            Ok(event)
        }

        async fn append_with_sequence_if_next(
            &self,
            session_id: SessionId,
            expected_next_sequence: EventSequence,
            writer_package_id: PackageId,
            kind: EventKind,
            schema_version: u16,
            payload_json: serde_json::Value,
            metadata_json: serde_json::Value,
        ) -> anyhow::Result<Option<EventEnvelope>> {
            let expected_next_sequence = i64::try_from(expected_next_sequence)
                .map_err(|_| anyhow::anyhow!("expected event sequence exceeds backend range"))?;
            let mut conn = self.pool.get().await.map_err(redact_pg)?;
            let tx = conn.transaction().await.map_err(redact_pg)?;
            tx.execute("SELECT pg_advisory_xact_lock(hashtext($1))", &[&session_id])
                .await
                .map_err(redact_pg)?;
            let row = tx
                .query_one(
                    "SELECT COALESCE(MAX(sequence) + 1, 0) FROM events WHERE session_id = $1",
                    &[&session_id],
                )
                .await
                .map_err(redact_pg)?;
            let next_seq: i64 = row.try_get(0)?;
            if next_seq != expected_next_sequence {
                return Ok(None);
            }
            let event = EventEnvelope {
                id: plurora_core::new_id("evt"),
                session_id,
                sequence: next_seq as EventSequence,
                timestamp: chrono::Utc::now(),
                writer_package_id,
                kind,
                schema_version,
                payload: payload_json,
                metadata: metadata_json,
            };
            let payload = serde_json::to_string(&event.payload)?;
            let metadata = serde_json::to_string(&event.metadata)?;
            tx.execute(
                "INSERT INTO events (id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json)\n                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::jsonb)",
                &[
                    &event.id as &(dyn ToSql + Sync),
                    &event.session_id,
                    &(event.sequence as i64),
                    &event.timestamp.to_rfc3339(),
                    &event.writer_package_id,
                    &event.kind,
                    &(event.schema_version as i32),
                    &payload,
                    &metadata,
                ],
            )
            .await
            .map_err(redact_pg)?;
            tx.commit().await.map_err(redact_pg)?;
            let _ = self.tx.send(event.clone());
            Ok(Some(event))
        }

        /// PostgreSQL pushdown: range scan on `kind` with optional upper
        /// bound to avoid full-table scan.
        async fn list_kind_prefix(&self, prefix: &str) -> anyhow::Result<Vec<EventEnvelope>> {
            let conn = self.pool.get().await.map_err(redact_pg)?;
            let prefix_end = kind_prefix_upper_bound(prefix);
            let rows = if let Some(end) = prefix_end {
                conn.query(
                    "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                     FROM events WHERE kind >= $1 AND kind < $2
                     ORDER BY timestamp ASC, session_id ASC, sequence ASC",
                    &[&prefix, &end],
                )
                .await
            } else {
                conn.query(
                    "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                     FROM events WHERE kind LIKE $1
                     ORDER BY timestamp ASC, session_id ASC, sequence ASC",
                    &[&format!("{}%", prefix)],
                )
                .await
            }
            .map_err(redact_pg)?;
            rows.iter().map(|r| row_to_event(r)).collect()
        }

        /// PostgreSQL pushdown: session + kind range scan.
        async fn list_session_kind_prefix(
            &self,
            session_id: &SessionId,
            prefix: &str,
        ) -> anyhow::Result<Vec<EventEnvelope>> {
            let conn = self.pool.get().await.map_err(redact_pg)?;
            let prefix_end = kind_prefix_upper_bound(prefix);
            let rows = if let Some(end) = prefix_end {
                conn.query(
                    "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                     FROM events WHERE session_id = $1 AND kind >= $2 AND kind < $3
                     ORDER BY sequence ASC",
                    &[session_id, &prefix, &end],
                )
                .await
            } else {
                conn.query(
                    "SELECT id, session_id, sequence, timestamp, writer_package_id, kind, schema_version, payload_json, metadata_json
                     FROM events WHERE session_id = $1 AND kind LIKE $2
                     ORDER BY sequence ASC",
                    &[session_id, &format!("{}%", prefix)],
                )
                .await
            }
            .map_err(redact_pg)?;
            rows.iter().map(|r| row_to_event(r)).collect()
        }
    }

    #[cfg(test)]
    mod postgres_tests {
        use super::*;
        use plurora_core::KERNEL_PACKAGE_ID;
        use serde_json::json;

        /// Helper: connect to PG if `PLURORA_POSTGRES_TEST_DATABASE_URL` is set,
        /// otherwise skip the test.
        async fn connect_or_skip() -> Option<PostgresEventStore> {
            let url = std::env::var("PLURORA_POSTGRES_TEST_DATABASE_URL").ok()?;
            match PostgresEventStore::connect(&url).await {
                Ok(store) => Some(store),
                Err(e) => {
                    eprintln!("NOTE: PLURORA_POSTGRES_TEST_DATABASE_URL set but connection failed (skipping): {e}");
                    None
                }
            }
        }

        #[tokio::test]
        async fn postgres_store_basic_contract() -> anyhow::Result<()> {
            let Some(store) = connect_or_skip().await else {
                return Ok(());
            };
            let session_id = format!("ses_pg_test_{}", plurora_core::new_id("pg"));
            store
                .append_with_sequence(
                    session_id.clone(),
                    KERNEL_PACKAGE_ID.to_string(),
                    "kernel/v1/session.opened".to_string(),
                    1,
                    json!({}),
                    json!({}),
                )
                .await?;

            let events = store.list_session(&session_id).await?;
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].sequence, 0);

            let next = store.next_sequence(&session_id).await?;
            assert_eq!(next, 1);
            Ok(())
        }

        #[tokio::test]
        async fn postgres_concurrent_append_no_duplicate_sequences() -> anyhow::Result<()> {
            let Some(store) = connect_or_skip().await else {
                return Ok(());
            };
            let session_id = format!("ses_pg_concurrent_{}", plurora_core::new_id("pg"));
            let store = Arc::new(store);

            store
                .append_with_sequence(
                    session_id.clone(),
                    KERNEL_PACKAGE_ID.to_string(),
                    "kernel/v1/session.opened".to_string(),
                    1,
                    json!({}),
                    json!({}),
                )
                .await?;

            let n = 20;
            let mut handles = Vec::new();
            for i in 0..n {
                let s = store.clone();
                let sid = session_id.clone();
                handles.push(tokio::spawn(async move {
                    s.append_with_sequence(
                        sid,
                        KERNEL_PACKAGE_ID.to_string(),
                        format!("test/concurrent.{}", i),
                        1,
                        json!({"i": i}),
                        json!({}),
                    )
                    .await
                }));
            }
            for h in handles {
                let _ = h.await.unwrap()?;
            }

            let events = store.list_session(&session_id).await?;
            let mut sequences: Vec<u64> = events.iter().map(|e| e.sequence).collect();
            sequences.sort();
            let dedup: std::collections::HashSet<u64> = sequences.iter().copied().collect();
            assert_eq!(
                dedup.len(),
                sequences.len(),
                "duplicate sequences found: {:?}",
                sequences
            );
            for (i, seq) in sequences.iter().enumerate() {
                assert_eq!(*seq, i as u64, "non-contiguous at index {}: {}", i, seq);
            }
            Ok(())
        }

        #[tokio::test]
        async fn postgres_compare_and_append_rejects_stale_tail() -> anyhow::Result<()> {
            let Some(store) = connect_or_skip().await else {
                return Ok(());
            };
            let session_id = format!("ses_pg_compare_{}", plurora_core::new_id("pg"));
            let first = store
                .append_with_sequence_if_next(
                    session_id.clone(),
                    0,
                    KERNEL_PACKAGE_ID.to_string(),
                    "test/compare.first".to_string(),
                    1,
                    json!({}),
                    json!({}),
                )
                .await?;
            let stale = store
                .append_with_sequence_if_next(
                    session_id.clone(),
                    0,
                    KERNEL_PACKAGE_ID.to_string(),
                    "test/compare.stale".to_string(),
                    1,
                    json!({}),
                    json!({}),
                )
                .await?;
            assert!(first.is_some());
            assert!(stale.is_none());
            assert_eq!(store.list_session(&session_id).await?.len(), 1);
            Ok(())
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_backend::PostgresEventStore;
