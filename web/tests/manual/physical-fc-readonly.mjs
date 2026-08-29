import initWasm from "/wasm/ade_web_readonly_serial_wasm_bridge.js";
import { WebSerialReadonlyHost } from "/webserial-readonly-host.mjs";

const DISPLAY_FIELDS = Object.freeze([
  "outcome",
  "apiVersion",
  "fcVariant",
  "fcVersion",
  "targetName",
  "scopeMismatchField",
  "failure",
]);

const button = document.querySelector("#select-and-read");
const status = document.querySelector("#status");
const resultList = document.querySelector("#result");
let host;

function renderTypedResult(result) {
  resultList.replaceChildren();
  for (const field of DISPLAY_FIELDS) {
    const value = result[field];
    if (value === undefined || value === null || value === "") continue;
    const term = document.createElement("dt");
    const description = document.createElement("dd");
    term.textContent = field;
    description.textContent = String(value);
    resultList.append(term, description);
  }
}

button.addEventListener("click", async () => {
  button.disabled = true;
  status.textContent = "Waiting for explicit owner port selection…";
  resultList.replaceChildren();
  try {
    const selection = await host.selectPortFromUserGesture();
    if (!selection.ok) {
      renderTypedResult({ outcome: "failed", failure: selection.failure });
      status.textContent = "Selection stopped without reading the FC.";
      return;
    }
    status.textContent = "Reading the four Rust-authorised identity responses…";
    const result = await host.discover();
    renderTypedResult(result);
    status.textContent = "Read-only identity attempt finished and the port was closed.";
  } catch {
    renderTypedResult({ outcome: "failed", failure: "HarnessFailure" });
    status.textContent = "The attempt stopped fail-closed.";
  } finally {
    button.disabled = false;
  }
});

try {
  await initWasm({
    module_or_path: new URL(
      "/wasm/ade_web_readonly_serial_wasm_bridge_bg.wasm",
      globalThis.location.href,
    ),
  });
  host = new WebSerialReadonlyHost();
  button.disabled = false;
  status.textContent = "Ready. Follow the safety checklist before selecting the FC.";
} catch {
  status.textContent = "Harness unavailable: the audited Rust bridge did not initialize.";
}
