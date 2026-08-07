//! Storage Backend Neutrality conformance cases (S1).
//!
//! These cases prove that `InMemoryEventStore` and `SqliteEventStore`
//! exhibit the same observable behaviour for the EventStore contract.
//! No timestamp exact-match comparisons; no network; no external services.
//! Temporary files/databases are cleaned up after each case.

use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

use plurora_core::{EventEnvelope, SessionId, KERNEL_PACKAGE_ID};
use plurora_runtime::{EventStore, InMemoryEventStore, SqliteEventStore};
use serde_json::json;

use plurora_core::new_id;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a temporary SQLite database path, unique per invocation.
fn temp_db_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "plurora-storage-s1-{}-{}.db",
        label,
        std::process::id()
    ))
}

/// Build a simple event envelope with a given session, sequence, and kind.
fn make_event(session_id: &str, sequence: u64, kind: &str) -> EventEnvelope {
    EventEnvelope::new(
        new_id("evt"),
        session_id.to_string(),
        sequence,
        KERNEL_PACKAGE_ID.to_string(),
        kind,
        json!({}),
    )
}

/// Compare two event lists by (session_id, sequence, kind), ignoring
/// timestamps and event IDs.
fn events_match_by_semantic_key(a: &[EventEnvelope], b: &[EventEnvelope]) -> bool {
    let key_of = |e: &EventEnvelope| (e.session_id.clone(), e.sequence, e.kind.clone());
    let keys_a: HashSet<_> = a.iter().map(key_of).collect();
    let keys_b: HashSet<_> = b.iter().map(key_of).collect();
    keys_a == keys_b && a.len() == b.len()
}

/// Collect kind strings in order.
fn kind_strings(events: &[EventEnvelope]) -> Vec<String> {
    events.iter().map(|e| e.kind.clone()).collect()
}

// ---------------------------------------------------------------------------
// a) in_memory_event_store_contract_append_range
// ---------------------------------------------------------------------------

/// InMemoryEventStore satisfies the basic EventStore contract:
/// append → list_session, list_all, list_session_range, next_sequence.
pub(crate) async fn in_memory_event_store_contract_append_range() -> anyhow::Result<()> {
    let store = InMemoryEventStore::default();
    let sid: SessionId = "ses_im_contract".to_string();

    // Append events via low-level append
    store.append(make_event(&sid, 0, "test/alpha")).await?;
    store.append(make_event(&sid, 1, "test/beta")).await?;
    store.append(make_event(&sid, 2, "test/gamma")).await?;

    // list_session
    let session_events = store.list_session(&sid).await?;
    anyhow::ensure!(
        session_events.len() == 3,
        "expected 3 session events, got {}",
        session_events.len()
    );

    // list_all
    let all_events = store.list_all().await?;
    anyhow::ensure!(
        all_events.len() == 3,
        "expected 3 total events, got {}",
        all_events.len()
    );

    // list_session_range: after_sequence=0 → events 1 and 2
    let range = store.list_session_range(&sid, Some(0), None).await?;
    anyhow::ensure!(
        range.len() == 2,
        "expected 2 events after seq 0, got {}",
        range.len()
    );
    anyhow::ensure!(range[0].sequence == 1);
    anyhow::ensure!(range[1].sequence == 2);

    // list_session_range with limit
    let range_limited = store.list_session_range(&sid, None, Some(1)).await?;
    anyhow::ensure!(range_limited.len() == 1, "expected 1 event with limit 1");
    anyhow::ensure!(range_limited[0].sequence == 0);

    // next_sequence
    let next = store.next_sequence(&sid).await?;
    anyhow::ensure!(next == 3, "expected next_sequence=3, got {}", next);

    // next_sequence for unknown session
    let unknown_sid: SessionId = "ses_unknown".to_string();
    let unknown_next = store.next_sequence(&unknown_sid).await?;
    anyhow::ensure!(
        unknown_next == 0,
        "expected next_sequence=0 for unknown session, got {}",
        unknown_next
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// b) sqlite_event_store_contract_append_range
// ---------------------------------------------------------------------------

/// SqliteEventStore satisfies the same basic EventStore contract as in-memory.
pub(crate) async fn sqlite_event_store_contract_append_range() -> anyhow::Result<()> {
    let path = temp_db_path("sqlite_contract");
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    let store = SqliteEventStore::open(&path)?;
    let sid: SessionId = "ses_sqlite_contract".to_string();

    // Append events via low-level append
    store.append(make_event(&sid, 0, "test/alpha")).await?;
    store.append(make_event(&sid, 1, "test/beta")).await?;
    store.append(make_event(&sid, 2, "test/gamma")).await?;

    // list_session
    let session_events = store.list_session(&sid).await?;
    anyhow::ensure!(
        session_events.len() == 3,
        "expected 3 session events, got {}",
        session_events.len()
    );

    // list_all
    let all_events = store.list_all().await?;
    anyhow::ensure!(
        all_events.len() == 3,
        "expected 3 total events, got {}",
        all_events.len()
    );

    // list_session_range: after_sequence=0 → events 1 and 2
    let range = store.list_session_range(&sid, Some(0), None).await?;
    anyhow::ensure!(
        range.len() == 2,
        "expected 2 events after seq 0, got {}",
        range.len()
    );
    anyhow::ensure!(range[0].sequence == 1);
    anyhow::ensure!(range[1].sequence == 2);

    // list_session_range with limit
    let range_limited = store.list_session_range(&sid, None, Some(1)).await?;
    anyhow::ensure!(range_limited.len() == 1, "expected 1 event with limit 1");
    anyhow::ensure!(range_limited[0].sequence == 0);

    // next_sequence
    let next = store.next_sequence(&sid).await?;
    anyhow::ensure!(next == 3, "expected next_sequence=3, got {}", next);

    // next_sequence for unknown session
    let unknown_sid: SessionId = "ses_unknown".to_string();
    let unknown_next = store.next_sequence(&unknown_sid).await?;
    anyhow::ensure!(
        unknown_next == 0,
        "expected next_sequence=0 for unknown session, got {}",
        unknown_next
    );

    let _ = fs::remove_file(path);
    Ok(())
}

// ---------------------------------------------------------------------------
// c) backend_parity_kind_prefix
// ---------------------------------------------------------------------------

/// InMemory and SQLite produce identical kind-prefix query results.
pub(crate) async fn backend_parity_kind_prefix() -> anyhow::Result<()> {
    let mem = InMemoryEventStore::default();

    let path = temp_db_path("parity_prefix");
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    let sqlite = SqliteEventStore::open(&path)?;
    let sid: SessionId = "ses_prefix_parity".to_string();

    let kinds = [
        "kernel/v1/permission.granted",
        "kernel/v1/permission.denied",
        "kernel/v1/session.opened",
        "test/custom.event",
    ];

    // Populate both stores with the same events via append_with_sequence
    for kind in &kinds {
        mem.append_with_sequence(
            sid.clone(),
            KERNEL_PACKAGE_ID.to_string(),
            kind.to_string(),
            1,
            json!({}),
            json!({}),
        )
        .await?;
        sqlite
            .append_with_sequence(
                sid.clone(),
                KERNEL_PACKAGE_ID.to_string(),
                kind.to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;
    }

    // Cross-session kind-prefix query
    let mem_perm = mem.list_kind_prefix("kernel/v1/permission").await?;
    let sqlite_perm = sqlite.list_kind_prefix("kernel/v1/permission").await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_perm, &sqlite_perm),
        "kind_prefix mismatch: in-memory has {} events, sqlite has {}",
        mem_perm.len(),
        sqlite_perm.len()
    );
    anyhow::ensure!(
        mem_perm.len() == 2,
        "expected 2 permission events, got {}",
        mem_perm.len()
    );

    // Session-scoped kind-prefix query
    let mem_session_perm = mem
        .list_session_kind_prefix(&sid, "kernel/v1/permission")
        .await?;
    let sqlite_session_perm = sqlite
        .list_session_kind_prefix(&sid, "kernel/v1/permission")
        .await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_session_perm, &sqlite_session_perm),
        "session kind_prefix mismatch"
    );
    anyhow::ensure!(mem_session_perm.len() == 2);

    // Empty prefix match
    let mem_all = mem.list_kind_prefix("").await?;
    let sqlite_all = sqlite.list_kind_prefix("").await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_all, &sqlite_all),
        "empty prefix mismatch"
    );
    anyhow::ensure!(mem_all.len() == 4, "expected 4 events with empty prefix");

    let _ = fs::remove_file(path);
    Ok(())
}

// ---------------------------------------------------------------------------
// d) backend_parity_concurrent_append
// ---------------------------------------------------------------------------

/// Both backends guarantee no duplicate sequences under concurrent append.
pub(crate) async fn backend_parity_concurrent_append() -> anyhow::Result<()> {
    // --- In-memory ---
    {
        let store = Arc::new(InMemoryEventStore::default());
        let sid: SessionId = "ses_concurrent_mem".to_string();
        store
            .append_with_sequence(
                sid.clone(),
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
            let session_id = sid.clone();
            handles.push(tokio::spawn(async move {
                s.append_with_sequence(
                    session_id,
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

        let events = store.list_session(&sid).await?;
        let mut sequences: Vec<u64> = events.iter().map(|e| e.sequence).collect();
        sequences.sort();
        let dedup: HashSet<u64> = sequences.iter().copied().collect();
        anyhow::ensure!(
            dedup.len() == sequences.len(),
            "in-memory: duplicate sequences found"
        );
        for (i, seq) in sequences.iter().enumerate() {
            anyhow::ensure!(
                *seq == i as u64,
                "in-memory: non-contiguous at index {}: {}",
                i,
                seq
            );
        }
    }

    // --- SQLite ---
    {
        let path = temp_db_path("parity_concurrent");
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        let store = Arc::new(SqliteEventStore::open(&path)?);
        let sid: SessionId = "ses_concurrent_sqlite".to_string();
        store
            .append_with_sequence(
                sid.clone(),
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
            let session_id = sid.clone();
            handles.push(tokio::spawn(async move {
                s.append_with_sequence(
                    session_id,
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

        let events = store.list_session(&sid).await?;
        let mut sequences: Vec<u64> = events.iter().map(|e| e.sequence).collect();
        sequences.sort();
        let dedup: HashSet<u64> = sequences.iter().copied().collect();
        anyhow::ensure!(
            dedup.len() == sequences.len(),
            "sqlite: duplicate sequences found"
        );
        for (i, seq) in sequences.iter().enumerate() {
            anyhow::ensure!(
                *seq == i as u64,
                "sqlite: non-contiguous at index {}: {}",
                i,
                seq
            );
        }

        let _ = fs::remove_file(path);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// e) backend_parity_subscription
// ---------------------------------------------------------------------------

/// Both backends broadcast newly appended events through subscribe().
pub(crate) async fn backend_parity_subscription() -> anyhow::Result<()> {
    // --- In-memory ---
    {
        let store = InMemoryEventStore::default();
        let mut rx = store.subscribe();
        let sid: SessionId = "ses_sub_mem".to_string();
        store
            .append_with_sequence(
                sid,
                KERNEL_PACKAGE_ID.to_string(),
                "test/sub.alpha".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;

        let received = rx.try_recv();
        anyhow::ensure!(received.is_ok(), "in-memory: expected broadcast event");
        let event = received.unwrap();
        anyhow::ensure!(event.kind == "test/sub.alpha", "in-memory: kind mismatch");
    }

    // --- SQLite ---
    {
        let path = temp_db_path("parity_sub");
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        let store = SqliteEventStore::open(&path)?;
        let mut rx = store.subscribe();
        let sid: SessionId = "ses_sub_sqlite".to_string();
        store
            .append_with_sequence(
                sid,
                KERNEL_PACKAGE_ID.to_string(),
                "test/sub.beta".to_string(),
                1,
                json!({}),
                json!({}),
            )
            .await?;

        let received = rx.try_recv();
        anyhow::ensure!(received.is_ok(), "sqlite: expected broadcast event");
        let event = received.unwrap();
        anyhow::ensure!(event.kind == "test/sub.beta", "sqlite: kind mismatch");

        let _ = fs::remove_file(path);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// f) storage_backend_rehydrate_parity
// ---------------------------------------------------------------------------

/// InMemory and SQLite backends both support rehydrate parity:
/// after populating events and reconstructing a runtime from the same
/// event store, the substrate (assets, branches, projections, permissions)
/// should contain the same observable state.
///
/// This case focuses on the event-store level: both backends can replay
/// events and produce the same event sequence. Full runtime rehydrate
/// parity is already covered by `substrate.sqlite_rehydrate` and
/// `substrate.permission_grant_rehydrate`; this case proves the
/// event-store backbone parity that those cases depend on.
pub(crate) async fn storage_backend_rehydrate_parity() -> anyhow::Result<()> {
    // Populate in-memory store
    let mem = InMemoryEventStore::default();
    let sid: SessionId = "ses_rehydrate_parity".to_string();
    for i in 0..5 {
        mem.append_with_sequence(
            sid.clone(),
            KERNEL_PACKAGE_ID.to_string(),
            format!("test/rehydrate.{}", i),
            1,
            json!({"idx": i}),
            json!({}),
        )
        .await?;
    }

    // Populate SQLite store with the same events
    let path = temp_db_path("rehydrate_parity");
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    let sqlite = SqliteEventStore::open(&path)?;
    for i in 0..5 {
        sqlite
            .append_with_sequence(
                sid.clone(),
                KERNEL_PACKAGE_ID.to_string(),
                format!("test/rehydrate.{}", i),
                1,
                json!({"idx": i}),
                json!({}),
            )
            .await?;
    }

    // Compare full event content (ignoring timestamps and IDs)
    let mem_events = mem.list_session(&sid).await?;
    let sqlite_events = sqlite.list_session(&sid).await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_events, &sqlite_events),
        "rehydrate parity: event mismatch between in-memory and sqlite"
    );

    // Both stores report the same next_sequence
    let mem_next = mem.next_sequence(&sid).await?;
    let sqlite_next = sqlite.next_sequence(&sid).await?;
    anyhow::ensure!(
        mem_next == sqlite_next,
        "next_sequence mismatch: mem={}, sqlite={}",
        mem_next,
        sqlite_next
    );

    // Kind-prefix parity on the rehydrated event set
    let mem_prefix = mem.list_kind_prefix("test/rehydrate").await?;
    let sqlite_prefix = sqlite.list_kind_prefix("test/rehydrate").await?;
    anyhow::ensure!(
        kind_strings(&mem_prefix) == kind_strings(&sqlite_prefix),
        "rehydrate kind_prefix mismatch"
    );
    anyhow::ensure!(mem_prefix.len() == 5, "expected 5 rehydrate events");

    let _ = fs::remove_file(path);
    Ok(())
}

// ---------------------------------------------------------------------------
// PostgreSQL opt-in conformance cases
// ---------------------------------------------------------------------------
// These cases only run when:
//   1. `cfg(feature="postgres")` is enabled, AND
//   2. `PLURORA_POSTGRES_TEST_DATABASE_URL` is set in the environment.
// Otherwise they silently skip (return Ok).
// Default CI/conformance is unaffected.

#[cfg(feature = "postgres")]
pub(crate) async fn postgres_event_store_contract_append_range() -> anyhow::Result<()> {
    let database_url = match std::env::var("PLURORA_POSTGRES_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => return Ok(()),
    };
    let store = plurora_runtime::PostgresEventStore::connect(&database_url).await?;
    let sid: SessionId = format!("ses_pg_contract_{}", plurora_core::new_id("pg"));

    store.append(make_event(&sid, 0, "test/alpha")).await?;
    store.append(make_event(&sid, 1, "test/beta")).await?;
    store.append(make_event(&sid, 2, "test/gamma")).await?;

    let session_events = store.list_session(&sid).await?;
    anyhow::ensure!(
        session_events.len() == 3,
        "expected 3 session events, got {}",
        session_events.len()
    );

    let all_events = store.list_all().await?;
    anyhow::ensure!(
        all_events.len() >= 3,
        "expected at least 3 total events, got {}",
        all_events.len()
    );

    let range = store.list_session_range(&sid, Some(0), None).await?;
    anyhow::ensure!(
        range.len() == 2,
        "expected 2 events after seq 0, got {}",
        range.len()
    );
    anyhow::ensure!(range[0].sequence == 1);
    anyhow::ensure!(range[1].sequence == 2);

    let range_limited = store.list_session_range(&sid, None, Some(1)).await?;
    anyhow::ensure!(range_limited.len() == 1, "expected 1 event with limit 1");
    anyhow::ensure!(range_limited[0].sequence == 0);

    let next = store.next_sequence(&sid).await?;
    anyhow::ensure!(next == 3, "expected next_sequence=3, got {}", next);

    let unknown_sid: SessionId = format!("ses_pg_unknown_{}", plurora_core::new_id("pg"));
    let unknown_next = store.next_sequence(&unknown_sid).await?;
    anyhow::ensure!(
        unknown_next == 0,
        "expected next_sequence=0 for unknown session, got {}",
        unknown_next
    );

    Ok(())
}

#[cfg(feature = "postgres")]
pub(crate) async fn postgres_backend_parity_kind_prefix() -> anyhow::Result<()> {
    let database_url = match std::env::var("PLURORA_POSTGRES_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => return Ok(()),
    };
    let pg = plurora_runtime::PostgresEventStore::connect(&database_url).await?;
    let mem = InMemoryEventStore::default();
    let sid: SessionId = format!("ses_pg_prefix_{}", plurora_core::new_id("pg"));

    let kinds = [
        "kernel/v1/permission.granted",
        "kernel/v1/permission.denied",
        "kernel/v1/session.opened",
        "test/custom.event",
    ];

    for kind in &kinds {
        mem.append_with_sequence(
            sid.clone(),
            KERNEL_PACKAGE_ID.to_string(),
            kind.to_string(),
            1,
            json!({}),
            json!({}),
        )
        .await?;
        pg.append_with_sequence(
            sid.clone(),
            KERNEL_PACKAGE_ID.to_string(),
            kind.to_string(),
            1,
            json!({}),
            json!({}),
        )
        .await?;
    }

    let mem_perm = mem.list_kind_prefix("kernel/v1/permission").await?;
    let pg_perm = pg.list_kind_prefix("kernel/v1/permission").await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_perm, &pg_perm),
        "kind_prefix mismatch: in-memory has {} events, postgres has {}",
        mem_perm.len(),
        pg_perm.len()
    );
    anyhow::ensure!(
        pg_perm.len() == 2,
        "expected 2 permission events, got {}",
        pg_perm.len()
    );

    let mem_session_perm = mem
        .list_session_kind_prefix(&sid, "kernel/v1/permission")
        .await?;
    let pg_session_perm = pg
        .list_session_kind_prefix(&sid, "kernel/v1/permission")
        .await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_session_perm, &pg_session_perm),
        "session kind_prefix mismatch"
    );

    Ok(())
}

#[cfg(feature = "postgres")]
pub(crate) async fn postgres_backend_parity_concurrent_append() -> anyhow::Result<()> {
    let database_url = match std::env::var("PLURORA_POSTGRES_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => return Ok(()),
    };
    let pg = Arc::new(plurora_runtime::PostgresEventStore::connect(&database_url).await?);
    let sid: SessionId = format!("ses_pg_concurrent_{}", plurora_core::new_id("pg"));

    pg.append_with_sequence(
        sid.clone(),
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
        let s = pg.clone();
        let session_id = sid.clone();
        handles.push(tokio::spawn(async move {
            s.append_with_sequence(
                session_id,
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

    let events = pg.list_session(&sid).await?;
    let mut sequences: Vec<u64> = events.iter().map(|e| e.sequence).collect();
    sequences.sort();
    let dedup: HashSet<u64> = sequences.iter().copied().collect();
    anyhow::ensure!(
        dedup.len() == sequences.len(),
        "postgres: duplicate sequences found"
    );
    for (i, seq) in sequences.iter().enumerate() {
        anyhow::ensure!(
            *seq == i as u64,
            "postgres: non-contiguous at index {}: {}",
            i,
            seq
        );
    }

    Ok(())
}

#[cfg(feature = "postgres")]
pub(crate) async fn postgres_backend_parity_subscription() -> anyhow::Result<()> {
    let database_url = match std::env::var("PLURORA_POSTGRES_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => return Ok(()),
    };
    let store = plurora_runtime::PostgresEventStore::connect(&database_url).await?;
    let mut rx = store.subscribe();
    let sid: SessionId = format!("ses_pg_sub_{}", plurora_core::new_id("pg"));
    store
        .append_with_sequence(
            sid,
            KERNEL_PACKAGE_ID.to_string(),
            "test/sub.pg".to_string(),
            1,
            json!({}),
            json!({}),
        )
        .await?;

    let received = rx.try_recv();
    anyhow::ensure!(received.is_ok(), "postgres: expected broadcast event");
    let event = received.unwrap();
    anyhow::ensure!(event.kind == "test/sub.pg", "postgres: kind mismatch");

    Ok(())
}

#[cfg(feature = "postgres")]
pub(crate) async fn postgres_rehydrate_parity() -> anyhow::Result<()> {
    let database_url = match std::env::var("PLURORA_POSTGRES_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => return Ok(()),
    };
    let pg = plurora_runtime::PostgresEventStore::connect(&database_url).await?;
    let mem = InMemoryEventStore::default();
    let sid: SessionId = format!("ses_pg_rehydrate_{}", plurora_core::new_id("pg"));

    for i in 0..5 {
        mem.append_with_sequence(
            sid.clone(),
            KERNEL_PACKAGE_ID.to_string(),
            format!("test/rehydrate.{}", i),
            1,
            json!({"idx": i}),
            json!({}),
        )
        .await?;
        pg.append_with_sequence(
            sid.clone(),
            KERNEL_PACKAGE_ID.to_string(),
            format!("test/rehydrate.{}", i),
            1,
            json!({"idx": i}),
            json!({}),
        )
        .await?;
    }

    let mem_events = mem.list_session(&sid).await?;
    let pg_events = pg.list_session(&sid).await?;
    anyhow::ensure!(
        events_match_by_semantic_key(&mem_events, &pg_events),
        "rehydrate parity: event mismatch between in-memory and postgres"
    );

    let mem_next = mem.next_sequence(&sid).await?;
    let pg_next = pg.next_sequence(&sid).await?;
    anyhow::ensure!(
        mem_next == pg_next,
        "next_sequence mismatch: mem={}, postgres={}",
        mem_next,
        pg_next
    );

    let mem_prefix = mem.list_kind_prefix("test/rehydrate").await?;
    let pg_prefix = pg.list_kind_prefix("test/rehydrate").await?;
    anyhow::ensure!(
        kind_strings(&mem_prefix) == kind_strings(&pg_prefix),
        "rehydrate kind_prefix mismatch"
    );
    anyhow::ensure!(pg_prefix.len() == 5, "expected 5 rehydrate events");

    Ok(())
}
