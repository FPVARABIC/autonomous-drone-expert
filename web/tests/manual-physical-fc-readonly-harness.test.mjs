import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const webRoot = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = path.dirname(webRoot);
const manualRoot = path.join(webRoot, "tests", "manual");
const manualHtml = fs.readFileSync(path.join(manualRoot, "physical-fc-readonly.html"), "utf8");
const manualJs = fs.readFileSync(path.join(manualRoot, "physical-fc-readonly.mjs"), "utf8");
const launcher = fs.readFileSync(
  path.join(repoRoot, "scripts", "run_physical_fc_readonly_harness.py"),
  "utf8",
);
const instructions = fs.readFileSync(
  path.join(repoRoot, "docs", "m2", "PHYSICAL-FC-READONLY-MANUAL.md"),
  "utf8",
);

const sha256 = (relative) =>
  crypto.createHash("sha256").update(fs.readFileSync(path.join(repoRoot, relative))).digest("hex");

test("manual page can orchestrate only the accepted production host surface", () => {
  const imports = [...manualJs.matchAll(/^import .* from "([^"]+)";$/gm)].map((match) => match[1]);
  assert.deepEqual(imports, [
    "/wasm/ade_web_readonly_serial_wasm_bridge.js",
    "/webserial-readonly-host.mjs",
  ]);
  assert.equal((manualJs.match(/new WebSerialReadonlyHost\(\)/g) ?? []).length, 1);
  assert.doesNotMatch(manualJs, /new WebSerialReadonlyHost\s*\(\s*\{/);
  assert.match(manualJs, /addEventListener\("click", async \(\) =>/);
  assert.match(manualJs, /await host\.selectPortFromUserGesture\(\)/);
  assert.match(manualJs, /await host\.discover\(\)/);
  assert.doesNotMatch(manualJs, /host\.discover\s*\(\s*[^)]/);
  const hostCalls = new Set(
    [...manualJs.matchAll(/\bhost\.([A-Za-z_$][\w$]*)\s*\(/g)].map((match) => match[1]),
  );
  assert.deepEqual(hostCalls, new Set(["selectPortFromUserGesture", "discover"]));
});

test("manual harness contains no command, frame, write, alternate hardware, or persistence authority", () => {
  const source = `${manualHtml}\n${manualJs}\n${launcher}`;
  for (const pattern of [
    /\bCommandId\b|\bMSP_[A-Z0-9_]+\b|buildMsp|encodeMsp|decodeMsp/i,
    /\b(sendRaw|writeRaw|writer\.write|writeCommand|WriteApproval|TransportEffect)\b/i,
    /\b(WebUSB|WebHID|USBDevice|HIDDevice)\b|navigator\s*\.\s*(usb|hid)/i,
    /\b(getPorts|getInfo)\s*\(/,
    /\b(localStorage|sessionStorage|indexedDB)\b/,
    /\b(fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon|analytics|telemetry)\b/i,
  ]) {
    assert.doesNotMatch(source, pattern);
  }
});

test("manual output is a fixed privacy-bounded typed allowlist", () => {
  const fields = manualJs.match(/const DISPLAY_FIELDS = Object\.freeze\(\[(?<body>.*?)\]\);/s);
  assert.ok(fields?.groups?.body);
  assert.deepEqual([...fields.groups.body.matchAll(/"([A-Za-z]+)"/g)].map((match) => match[1]), [
    "outcome",
    "apiVersion",
    "fcVariant",
    "fcVersion",
    "targetName",
    "scopeMismatchField",
    "failure",
  ]);
  assert.doesNotMatch(
    `${manualHtml}\n${manualJs}`,
    /serial.?number|usbVendorId|usbProductId|\bVID\b|\bPID\b|\bCOM\d*\b|deviceId|getInfo|hardwareObserved|raw.?frame/i,
  );
  assert.match(manualHtml, /PHYSICAL_TEST_SESSION = MANUAL_OWNER_OBSERVATION/);
});

test("manual safety copy and owner instructions are explicit", () => {
  for (const marker of [
    "READ-ONLY IDENTIFICATION TEST",
    "NO CONFIGURATION CHANGES",
    "NO SAVE",
    "NO REBOOT",
    "NO MOTOR COMMANDS",
  ]) {
    assert.ok(manualHtml.includes(marker));
    assert.ok(instructions.includes(marker));
  }
  for (const marker of [
    "REMOVE ALL PROPELLERS",
    "DISCONNECT THE LIPO BATTERY",
    "Use FC USB only",
    "Do not press BOOT",
    "Do not enter DFU mode",
    "Do not flash firmware",
    "Close Betaflight Configurator and SpeedyBee App",
    "Ensure no other program owns the serial port",
    "Do not connect an external USB-UART/FTDI adapter",
    "Do not connect battery power during this first observation",
  ]) {
    assert.ok(instructions.includes(marker), `missing owner instruction: ${marker}`);
  }
});

test("launcher is localhost-only, standard-library, and serves an explicit allowlist", () => {
  assert.match(launcher, /BIND_ADDRESS: Final = "127\.0\.0\.1"/);
  assert.match(launcher, /PORT: Final = 8765/);
  assert.doesNotMatch(launcher, /0\.0\.0\.0|SimpleHTTPRequestHandler|npm|npx|https:\/\//i);
  assert.deepEqual(launcher.match(/http:\/\/[^"\s]+/g), ["http://{BIND_ADDRESS}:{PORT}/"]);
  assert.match(launcher, /handler_for\(routes, f"\{BIND_ADDRESS\}:\{PORT\}"\)/);
  assert.match(launcher, /cargo[\s\S]*\+1\.85\.0[\s\S]*ade-web-readonly-serial-wasm-bridge/);
  assert.match(launcher, /tools\/wasm-bindgen-cli-support\/Cargo\.toml/);
  assert.match(launcher, /Cache-Control", "no-store"/);
  assert.match(launcher, /Content-Security-Policy/);
  assert.match(launcher, /script-src 'self' 'wasm-unsafe-eval'/);
});

test("accepted product UI and dependency locks remain byte-for-byte frozen", () => {
  const approved = new Map([
    ["web/index.html", "3bda0744c9dbcb5980c48b113e4107f3465d99c75984d2068042f0fd1d67af21"],
    ["web/src/App.tsx", "1d4d44c43832d9fd17d4e0f594114814426e44f6017b14d550e04e990b8c98f9"],
    ["web/src/main.tsx", "6a823fd5d9abbc54e1820db1d47999541f06098f03fd12f0b7844f8030239d22"],
    ["web/src/styles.css", "d74f07088a3b206fc66661eea4682f083f6b7a1b08dbaa5b399818672356a3c4"],
    ["web/src/pwa-register.ts", "d7965e1d787a3503ddb21ecd7dbb9285bec16733d19a452fb834f78fd361385b"],
    ["web/public/manifest.webmanifest", "39113329fa9f63c43f78bcb19ace91a5c71e685bc13eb6c8af7a42082cec2558"],
    ["web/public/sw.js", "e802c3d7878164711d125c0cb512082362722ad351ba42dd6e385eefdd383889"],
    ["web/public/favicon.svg", "e6d2e59b7b5bbb0342e0fb496dfc262decbfe4426bbb7b047aec8d467d1dc6f7"],
    ["web/package.json", "59c01b1db0bb904a1be453371cfaf8c27a883ebceb0b55bb07e65bc22388a586"],
    ["web/package-lock.json", "c3015e9454da094d307975921b8aa2c195a15b9dffe0498a9c758b57d922c05d"],
  ]);
  for (const [relative, expected] of approved) assert.equal(sha256(relative), expected, relative);

  const productEntrypoints = ["web/index.html", "web/src/App.tsx", "web/src/main.tsx"]
    .map((relative) => fs.readFileSync(path.join(repoRoot, relative), "utf8"))
    .join("\n");
  assert.doesNotMatch(productEntrypoints, /physical-fc-readonly|tests\/manual/i);
});
