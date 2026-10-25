use crate::Engine;
use wasm_bindgen::prelude::*;

/// WebAssembly session wrapper exposing the Crosshair protocol engine to browser workers.
#[wasm_bindgen]
pub struct CrosshairSession {
    inner: Engine,
}

#[wasm_bindgen]
impl CrosshairSession {
    /// Creates a new browser engine session at the starting position.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Engine::new(),
        }
    }

    /// Executes a line-oriented command and returns newline-joined output lines.
    pub fn command(&mut self, line: &str) -> String {
        self.inner.command(line).join("\n")
    }

    /// Returns the complete board and search state serialized as JSON.
    pub fn snapshot(&self) -> String {
        self.inner.snapshot_json()
    }
}

impl Default for CrosshairSession {
    fn default() -> Self {
        Self::new()
    }
}
