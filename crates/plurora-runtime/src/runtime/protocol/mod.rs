use bytes::Bytes;
use plurora_core::{CapHandleId, PackageId, RedactionState};
use reqwest::header::{HeaderName, HeaderValue};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::{OpenSessionRequest, Runtime};
use crate::{
    EventListRequest, EventStore, OutboundFrameKind, OutboundStreamFrame, OutboundWebSocketFrame,
    ProtocolContext, ProtocolPrincipal, StreamEmitter, StreamRegistry, WebSocketEvent,
};

mod assets_projections;
mod audit;
mod capabilities;
mod installations;
mod local_exec;
mod outbound_dispatch;
mod packages;
mod permissions;
mod proposals;
mod runs;
mod sessions_events;
mod surface;

#[cfg(test)]
mod tests;
