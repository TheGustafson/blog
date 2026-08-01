/* global importScripts, wasm_bindgen */

let sessionPromise;

function loadSession() {
  if (!sessionPromise) {
    sessionPromise = (async () => {
      importScripts("ultimate-tictactoe.js");
      await wasm_bindgen("ultimate-tictactoe_bg.wasm");
      return new wasm_bindgen.UltimateSession();
    })();
  }
  return sessionPromise;
}

self.onmessage = async (event) => {
  const { id, command } = event.data ?? {};
  if (typeof id !== "number" || typeof command !== "string") {
    self.postMessage({
      id,
      ok: false,
      error: "worker requests need a numeric id and string command",
    });
    return;
  }

  try {
    const session = await loadSession();
    const output = session.command(command);
    const snapshot = JSON.parse(session.snapshot());
    self.postMessage({ id, ok: true, output, snapshot });
  } catch (error) {
    self.postMessage({
      id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
