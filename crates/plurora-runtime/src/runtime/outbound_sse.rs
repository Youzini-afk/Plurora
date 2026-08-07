//! SSE (Server-Sent Events) parser for `kernel.v1.outbound.stream`.
//!
//! Parses `text/event-stream` responses per the WHATWG specification:
//! <https://html.spec.whatwg.org/multipage/server-sent-events.html>
//!
//! This is a strict-enough parser for OpenAI/Anthropic/etc. streaming
//! endpoints. It handles:
//! - `data:` field (multiple data lines joined with `\n`)
//! - `event:` field (event type)
//! - `id:` field (last event ID)
//! - `retry:` field (reconnection time)
//! - Events separated by blank lines (`\n\n`)
//! - Partial chunks split across `push()` calls

/// A parsed SSE event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` field value. Defaults to `"message"` if not specified.
    pub event: Option<String>,
    /// The `data:` field value. Multiple `data:` lines are joined with `\n`.
    pub data: String,
    /// The `id:` field value (last event ID).
    pub id: Option<String>,
    /// The `retry:` field value (reconnection time in milliseconds).
    pub retry: Option<u64>,
}

/// Incremental SSE parser that processes raw byte chunks.
///
/// Usage:
/// ```ignore
/// let mut parser = SseParser::new();
/// let events = parser.push(&chunk_bytes);
/// for event in events {
///     // handle event
/// }
/// ```
pub struct SseParser {
    /// Buffer for incomplete lines.
    buffer: String,
    /// Current event fields being accumulated.
    current_event: Option<IncompleteEvent>,
}

/// Incomplete event being built from multiple field lines.
#[derive(Default)]
struct IncompleteEvent {
    event_type: Option<String>,
    data_lines: Vec<String>,
    last_event_id: Option<String>,
    retry: Option<u64>,
}

impl SseParser {
    /// Create a new SSE parser.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            current_event: None,
        }
    }

    /// Push a chunk of bytes into the parser and return any complete events.
    ///
    /// Chunks may split in the middle of a line or even the middle of a
    /// field. The parser buffers incomplete data and returns only complete
    /// events.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        // Convert bytes to string, replacing invalid UTF-8 with replacement char.
        // SSE is specified as UTF-8.
        let text = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&text);
        self.process_buffer()
    }

    /// Process the buffer, extracting complete events.
    fn process_buffer(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();

        loop {
            // Look for a blank line (event separator). SSE spec says events
            // are separated by a blank line, which is either "\n\n" or "\r\n\r\n".
            let blank_pos = self.find_event_boundary();
            let Some(pos) = blank_pos else {
                break; // No complete event yet
            };

            // Extract the event text (everything before the blank line)
            let event_text = self.buffer[..pos].to_string();
            // Remove the event text + blank line from the buffer.
            // The blank line separator is "\n\n" (2 chars) or "\r\n\r\n" (4 chars).
            let separator_len = if self.buffer[pos..].starts_with("\r\n\r\n") {
                4
            } else {
                2
            };
            self.buffer = self.buffer[pos + separator_len..].to_string();

            // Parse the event fields
            if let Some(event) = self.parse_event_text(&event_text) {
                events.push(event);
            }
            // If parse_event_text returns None, it was an empty event
            // (just blank lines) which we silently skip.
        }

        events
    }

    /// Find the position of the next event boundary (blank line).
    ///
    /// Returns the position of the first `\n` in the `\n\n` sequence.
    fn find_event_boundary(&self) -> Option<usize> {
        // Look for \n\n
        let bytes = self.buffer.as_bytes();
        for i in 0..bytes.len().saturating_sub(1) {
            if bytes[i] == b'\n' {
                // Check for \n\n
                if bytes[i + 1] == b'\n' {
                    return Some(i);
                }
                // Check for \n\r\n (part of \r\n\r\n)
                if i + 2 < bytes.len() && bytes[i + 1] == b'\r' && bytes[i + 2] == b'\n' {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Parse event fields from text between blank lines.
    ///
    /// Returns None if the event has no data (empty event).
    fn parse_event_text(&mut self, text: &str) -> Option<SseEvent> {
        for line in text.lines() {
            // Strip optional \r from \r\n line endings
            let line = line.strip_suffix('\r').unwrap_or(line);

            // Skip empty lines within the event (shouldn't happen after
            // splitting on blank lines, but be defensive)
            if line.is_empty() {
                continue;
            }

            // Skip BOM at start of line (WHATWG spec)
            let line = line.strip_prefix('\u{feff}').unwrap_or(line);

            // Skip comment lines (starting with ':')
            if line.starts_with(':') {
                continue;
            }

            // Parse field: value
            if let Some(colon_pos) = line.find(':') {
                let field_name = &line[..colon_pos];
                let field_value = &line[colon_pos + 1..];
                // If value starts with a space, strip it (WHATWG spec)
                let field_value = field_value.strip_prefix(' ').unwrap_or(field_value);

                self.apply_field(field_name, field_value);
            } else {
                // Field with no colon: entire line is field name, empty value
                self.apply_field(line, "");
            }
        }

        // Flush the accumulated event
        self.flush_event()
    }

    /// Apply a single field to the current incomplete event.
    fn apply_field(&mut self, name: &str, value: &str) {
        let incomplete = self
            .current_event
            .get_or_insert_with(IncompleteEvent::default);

        match name {
            "event" => {
                incomplete.event_type = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "data" => {
                incomplete.data_lines.push(value.to_string());
            }
            "id" => {
                // Per spec, id field must not contain null
                if !value.contains('\0') {
                    incomplete.last_event_id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    incomplete.retry = Some(ms);
                }
                // If retry is not a valid integer, per spec, ignore it
            }
            _ => {
                // Per spec, ignore unknown field names
            }
        }
    }

    /// Flush the accumulated event, returning it if it has data.
    fn flush_event(&mut self) -> Option<SseEvent> {
        let incomplete = self.current_event.take()?;

        // Per spec: if data is empty, do not dispatch the event
        if incomplete.data_lines.is_empty() {
            return None;
        }

        // Join data lines with \n
        let data = incomplete.data_lines.join("\n");

        Some(SseEvent {
            event: incomplete.event_type,
            data,
            id: incomplete.last_event_id,
            retry: incomplete.retry,
        })
    }

    /// Drain any remaining buffered data that may constitute a partial event.
    ///
    /// Call this when the stream ends to emit any trailing event that wasn't
    /// terminated by a double newline. Per SSE spec, if the stream ends
    /// without a blank line, the last event should still be dispatched.
    pub fn flush_remaining(&mut self) -> Vec<SseEvent> {
        // First process any fully-delimited events already present.
        let mut result = self.process_buffer();

        // If the upstream closed with a trailing partial event (no final
        // blank line), parse the remaining buffered lines as one final event.
        // `parse_event_text` applies the normal dispatch rule via
        // `flush_event`: events with no data field are skipped.
        if !self.buffer.is_empty() {
            let event_text = std::mem::take(&mut self.buffer);
            if let Some(event) = self.parse_event_text(&event_text) {
                result.push(event);
            }
        }

        // Defensive cleanup: if an event was accumulated by another path,
        // dispatch it only if it has data and always reset internal state.
        if let Some(event) = self.flush_event() {
            result.push(event);
        }

        result
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_splits_on_double_newline() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: hello\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event, None);
    }

    #[test]
    fn sse_parser_handles_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: line1\ndata: line2\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn sse_parser_handles_event_field() {
        let mut parser = SseParser::new();
        let events = parser.push(b"event: custom\ndata: payload\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("custom"));
        assert_eq!(events[0].data, "payload");
    }

    #[test]
    fn sse_parser_handles_id_field() {
        let mut parser = SseParser::new();
        let events = parser.push(b"id: 42\ndata: payload\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id.as_deref(), Some("42"));
    }

    #[test]
    fn sse_parser_handles_retry_field() {
        let mut parser = SseParser::new();
        let events = parser.push(b"retry: 5000\ndata: payload\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn sse_parser_handles_partial_chunks() {
        let mut parser = SseParser::new();

        // First chunk: partial event
        let events = parser.push(b"data: hel");
        assert!(events.is_empty(), "partial data should not emit events");

        // Second chunk: completes the data line but no double newline yet
        let events = parser.push(b"lo\n");
        assert!(events.is_empty(), "no double newline yet");

        // Third chunk: double newline completes the event
        let events = parser.push(b"\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn sse_parser_handles_empty_lines() {
        let mut parser = SseParser::new();
        // Multiple events with various whitespace patterns
        let events = parser.push(b"data: first\n\ndata: second\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn sse_parser_handles_cr_lf_line_endings() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: hello\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn sse_parser_ignores_comments() {
        let mut parser = SseParser::new();
        let events = parser.push(b": this is a comment\ndata: real\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "real");
    }

    #[test]
    fn sse_parser_ignores_unknown_fields() {
        let mut parser = SseParser::new();
        let events = parser.push(b"unknown: field\ndata: real\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "real");
    }

    #[test]
    fn sse_parser_strips_space_after_colon() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: hello world\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
    }

    #[test]
    fn sse_parser_no_space_after_colon() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data:hello\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn sse_parser_empty_data_field() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data:\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "");
    }

    #[test]
    fn sse_parser_no_data_no_event() {
        let mut parser = SseParser::new();
        let events = parser.push(b"event: ping\n\n");
        // Per spec: if no data field, do not dispatch
        assert!(events.is_empty());
    }

    #[test]
    fn sse_parser_null_in_id_ignored() {
        let mut parser = SseParser::new();
        let events = parser.push(b"id: abc\0def\ndata: payload\n\n");
        assert_eq!(events.len(), 1);
        // id with null should be ignored per spec
        assert_eq!(events[0].id, None);
    }

    #[test]
    fn sse_parser_invalid_retry_ignored() {
        let mut parser = SseParser::new();
        let events = parser.push(b"retry: notanumber\ndata: payload\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, None);
    }

    #[test]
    fn sse_parser_flush_remaining() {
        let mut parser = SseParser::new();
        // Event without trailing double newline
        parser.push(b"data: trailing");
        let events = parser.flush_remaining();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "trailing");
    }

    #[test]
    fn sse_parser_multiple_events() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: event1\n\ndata: event2\n\ndata: event3\n\n");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, "event1");
        assert_eq!(events[1].data, "event2");
        assert_eq!(events[2].data, "event3");
    }

    #[test]
    fn sse_parser_default_event_type_is_none() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: test\n\n");
        assert_eq!(events.len(), 1);
        // event field is None when not specified (not "message")
        assert!(events[0].event.is_none());
    }

    #[test]
    fn sse_parser_three_data_lines() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: a\ndata: b\ndata: c\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "a\nb\nc");
    }
}
