//! Server-sent events over a blocking reader thread, reconnecting with the frontend's backoff
//! (`min(1000·2^n, 30000)` ms). The thread ends once the receiver is dropped.

use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;

use crate::api::Api;

#[derive(Debug, PartialEq, Eq)]
pub enum SseMessage {
    /// A named event (`event:` defaults to `message`) with its joined `data:` lines.
    Event { name: String, data: String },
    /// The connection dropped; consumers may poll until the next `Event`.
    Disconnected,
}

pub fn subscribe(api: Arc<Api>, endpoint: String) -> mpsc::UnboundedReceiver<SseMessage> {
    let (send, receive) = mpsc::unbounded();
    std::thread::spawn(move || {
        let mut failures = 0u32;
        loop {
            match api.stream(&endpoint) {
                Ok(reader) => {
                    failures = 0;
                    let mut parser = Parser::default();
                    for line in BufReader::new(reader).lines() {
                        let Ok(line) = line else { break };
                        if let Some(message) = parser.line(&line) {
                            if send.unbounded_send(message).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(error) => log::debug!("event stream {endpoint}: {error:#}"),
            }
            if send.unbounded_send(SseMessage::Disconnected).is_err() {
                return;
            }
            let delay = 1000u64.saturating_mul(1 << failures.min(5)).min(30_000);
            failures += 1;
            std::thread::sleep(Duration::from_millis(delay));
        }
    });
    receive
}

#[derive(Default)]
struct Parser {
    name: Option<String>,
    data: Vec<String>,
}

impl Parser {
    fn line(&mut self, line: &str) -> Option<SseMessage> {
        if line.is_empty() {
            let name = self.name.take();
            if self.data.is_empty() && name.is_none() {
                return None;
            }
            let data = std::mem::take(&mut self.data).join("\n");
            return Some(SseMessage::Event {
                name: name.unwrap_or_else(|| "message".into()),
                data,
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.name = Some(value.to_string()),
            "data" => self.data.push(value.to_string()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_events_and_skips_comments() {
        let mut parser = Parser::default();
        let lines = [
            ": heartbeat",
            "",
            "event: job",
            "data: {\"a\":",
            "data: 1}",
            "",
        ];
        let messages: Vec<_> = lines.iter().filter_map(|line| parser.line(line)).collect();
        assert_eq!(
            messages,
            [SseMessage::Event {
                name: "job".into(),
                data: "{\"a\":\n1}".into()
            }]
        );
    }
}
