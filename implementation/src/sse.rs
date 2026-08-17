// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Parsing and rewriting of `text/event-stream` bodies.
//!
//! A streamable-HTTP MCP server may answer a `tools/call` POST with SSE rather
//! than JSON. The response is still one logical answer, so its frames can be
//! annotated the same way a JSON body is — but only the `data:` payloads.
//! Every other line is reproduced verbatim, so event names, ids and retry
//! hints survive untouched, and a frame the caller does not rewrite is emitted
//! exactly as it arrived.
//!
//! Per the SSE grammar a frame's data field is the concatenation of its
//! `data:` lines joined by newlines, which is what gets parsed. A rewrite
//! re-serialises compactly onto a single `data:` line, because a JSON document
//! containing a raw newline cannot be valid.

use serde_json::Value;

/// A line of a frame, kept in arrival order so a rewrite preserves layout.
enum Line {
    /// The value of a `data:` field, with the optional leading space stripped.
    Data(String),
    /// Any other line: a field, a comment, or the blank separator.
    Verbatim(String),
}

struct Frame {
    lines: Vec<Line>,
    /// The data field parsed as JSON, when it is JSON at all.
    payload: Option<Value>,
    /// Set by the caller to have `render` emit `payload` instead of the
    /// original data lines.
    rewritten: bool,
}

pub struct Stream {
    frames: Vec<Frame>,
    newline: &'static str,
    /// The line ending the body itself ended with, if any.
    trailer: &'static str,
}

impl Stream {
    pub fn parse(body: &str) -> Stream {
        let newline = if body.contains("\r\n") { "\r\n" } else { "\n" };

        // Splitting on the final newline would yield a phantom empty line, and
        // so a phantom frame separator; it is held back and reattached on
        // render instead.
        let (content, trailer) = match body.strip_suffix('\n') {
            Some(rest) => (rest.strip_suffix('\r').unwrap_or(rest), newline),
            None => (body, ""),
        };

        let mut frames: Vec<Vec<Line>> = Vec::new();
        let mut current: Vec<Line> = Vec::new();

        for raw in content.split('\n') {
            let line = raw.strip_suffix('\r').unwrap_or(raw);

            if let Some(value) = line.strip_prefix("data:") {
                current.push(Line::Data(
                    value.strip_prefix(' ').unwrap_or(value).to_string(),
                ));
                continue;
            }

            current.push(Line::Verbatim(line.to_string()));

            // A blank line dispatches the event, ending the frame.
            if line.is_empty() {
                frames.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            frames.push(current);
        }

        let frames = frames
            .into_iter()
            .map(|lines| {
                let joined = lines
                    .iter()
                    .filter_map(|l| match l {
                        Line::Data(d) => Some(d.as_str()),
                        Line::Verbatim(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                // `[DONE]`-style sentinels and non-JSON payloads fall through
                // as `None` and are never eligible for rewriting.
                let payload = (!joined.is_empty())
                    .then(|| serde_json::from_str::<Value>(&joined).ok())
                    .flatten();

                Frame {
                    lines,
                    payload,
                    rewritten: false,
                }
            })
            .collect();

        Stream {
            frames,
            newline,
            trailer,
        }
    }

    /// Each frame's JSON payload, paired with the flag the caller sets when it
    /// changes one. Frames carrying no JSON are not yielded.
    pub fn payloads_mut(&mut self) -> impl Iterator<Item = (&mut Value, &mut bool)> {
        self.frames.iter_mut().filter_map(
            |Frame {
                 payload, rewritten, ..
             }| payload.as_mut().map(|value| (value, rewritten)),
        )
    }

    pub fn render(&self) -> String {
        let mut out = String::new();

        for frame in &self.frames {
            let payload = frame
                .rewritten
                .then_some(frame.payload.as_ref())
                .flatten()
                .and_then(|v| serde_json::to_string(v).ok());

            let mut emitted = false;
            for line in &frame.lines {
                match (line, &payload) {
                    // The rewritten payload takes the place of the first data
                    // line; the rest of the field collapses into it.
                    (Line::Data(_), Some(_)) if emitted => continue,
                    (Line::Data(_), Some(json)) => {
                        out.push_str("data: ");
                        out.push_str(json);
                        emitted = true;
                    }
                    (Line::Data(data), None) => {
                        out.push_str("data: ");
                        out.push_str(data);
                    }
                    (Line::Verbatim(text), _) => out.push_str(text),
                }
                out.push_str(self.newline);
            }
        }

        // Every line was emitted with a terminator; the last one carries only
        // the trailer the body actually ended with, if any.
        for _ in 0..self.newline.len() {
            out.pop();
        }
        out.push_str(self.trailer);
        out
    }
}
