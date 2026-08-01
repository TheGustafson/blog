use crate::Engine;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct UltimateSession {
    inner: Engine,
}

#[wasm_bindgen]
impl UltimateSession {
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

impl Default for UltimateSession {
    fn default() -> Self {
        Self::new()
    }
}
