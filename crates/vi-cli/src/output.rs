//! Human or JSON output.

use serde::Serialize;

/// Output mode.
#[derive(Debug, Clone, Copy)]
pub struct Output {
    json: bool,
}

impl Output {
    /// New output.
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    /// Whether JSON mode is on.
    pub fn json(&self) -> bool {
        self.json
    }

    /// Print a value: JSON when in JSON mode, else the given text.
    pub fn emit<T: Serialize>(&self, value: &T, text: impl FnOnce() -> String) {
        if self.json {
            match serde_json::to_string_pretty(value) {
                Ok(s) => println!("{s}"),
                Err(e) => eprintln!("error: could not serialise output: {e}"),
            }
        } else {
            println!("{}", text());
        }
    }

    /// Print one line of progress to stderr (human) or a JSON event line to
    /// stdout (JSON).
    pub fn event<T: Serialize>(&self, value: &T, text: impl FnOnce() -> String) {
        if self.json {
            if let Ok(s) = serde_json::to_string(value) {
                println!("{s}");
            }
        } else {
            eprintln!("{}", text());
        }
    }
}

/// Human-readable byte count.
pub fn bytes(n: u64) -> String {
    humansize::format_size(n, humansize::BINARY)
}
