use crate::protocol::Engine;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct OthelloSession {
    inner: Engine,
}

#[wasm_bindgen]
impl OthelloSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Engine::new(),
        }
    }

    pub fn command(&mut self, line: &str) -> String {
        self.inner.command(line).join("\n")
    }

    pub fn snapshot(&self) -> String {
        self.inner.snapshot_json()
    }
}

impl Default for OthelloSession {
    fn default() -> Self {
        Self::new()
    }
}
