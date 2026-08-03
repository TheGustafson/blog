/* global importScripts, wasm_bindgen */

let sessionPromise;

function loadSession() {
  if (!sessionPromise) {
    sessionPromise = (async () => {
      importScripts("backgammon.js");
      await wasm_bindgen("backgammon_bg.wasm");
      return new wasm_bindgen.BackgammonSession();
    })();
  }
  return sessionPromise;
}

function secureDie() {
  const bytes = new Uint8Array(16);
  for (;;) {
    self.crypto.getRandomValues(bytes);
    for (const byte of bytes) {
      if (byte < 252) return (byte % 6) + 1;
    }
  }
}

function withDice(command) {
  if (command === "opening" || command === "roll") {
    return `${command} ${secureDie()} ${secureDie()}`;
  }
  return command;
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
    const output = session.command(withDice(command));
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
