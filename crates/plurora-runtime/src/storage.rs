//! Storage roadmap.
//!
//! Runtime starts with `InMemoryEventStore` so the event spine can stabilize
//! before database details harden too early.
//!
//! The next store should be SQLite with an append-only `events` table roughly:
//!
//! ```text
//! id
//! stream_id
//! session_id
//! turn_id
//! kind
//! schema_version
//! timestamp
//! payload_json
//! metadata_json
//! causation_id
//! correlation_id
//! ```
