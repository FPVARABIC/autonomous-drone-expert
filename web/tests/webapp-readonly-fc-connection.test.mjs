import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const webRoot = fileURLToPath(new URL("..", import.meta.url));
const read = (relative) => fs.readFileSync(path.join(webRoot, relative), "utf8");
const app = read("src/App.tsx");
const facade = read("src/connection/readonly-fc-connection.mjs");
const facadeTypes = read("src/connection/readonly-fc-connection.d.mts");
const host = read("src/transport/webserial-readonly-host.mjs");
const hostTypes = read("src/transport/webserial-readonly-host.d.mts");
const diagnostic = read("src/diagnostics/readonly-trace.mjs");
const diagnosticTypes = read("src/diagnostics/readonly-trace.d.mts");
const viteConfig = read("vite.config.ts");
const acceptedBrowser = read("tests/browser/webserial-readonly-smoke.mjs");
const productionBrowser = read("tests/webapp-readonly-fc-browser-smoke.mjs");
const sha256 = (value) => crypto.createHash("sha256").update(value).digest("hex");

function numericFixture(source, declaration) {
  const fixture = source.match(
    new RegExp(`const ${declaration} = (\\[[\\s\\S]*?\\])\\.map`),
  );
  assert.ok(fixture?.[1], `missing numeric fixture: ${declaration}`);
  return [...fixture[1].matchAll(/\[[\d,\s]+\]/g)].map((entry) =>
    JSON.parse(entry[0]),
  );
}

test("the bounded production host and declaration are byte-locked", () => {
  assert.equal(sha256(host), "54b73f51a4f5c30e632e9cf8c76a7d1828d240c6af56a784372b079db0719903");
  assert.equal(
    sha256(hostTypes),
    "6006c87ab766a56b34d99a262373a4c799583f7e70332139ff96e2cb5387cabc",
  );
});

test("React has only the narrow prepared connection facade", () => {
  assert.match(
    app,
    /import \{ prepareReadonlyFcConnection \} from "\.\/connection\/readonly-fc-connection\.mjs"/,
  );
  assert.doesNotMatch(app, /webserial-readonly-host|WasmReadonlySerial|navigator\s*\.\s*serial/i);
  assert.doesNotMatch(app, /\b(requestPort|getPorts|getInfo)\s*\(/);
  assert.doesNotMatch(
    app,
    /\b(?:MSP_[A-Z0-9_]+|CommandId|WriteApproval|TransportEffect|SerialPort)\b|\b(?:build|encode|decode)Msp/i,
  );
  assert.doesNotMatch(app, /writer\s*\.\s*write|sendRaw|writeRaw|commandId|payload/i);
  assert.match(app, /void prepareReadonlyFcConnection\(\)/);
  assert.match(app, /readonlyFcConnection\.current = prepared;\s*setConnection\(\{ phase: "ready" \}\)/s);
  assert.match(app, /disabled=\{!connectionEnabled\}/);
});

test("the click path selects first and discovers with zero arguments", () => {
  const clickFlow = app.match(/async function connectReadonlyFc\(\) \{(?<body>.*?)^  \}/ms);
  assert.ok(clickFlow?.groups?.body);
  const body = clickFlow.groups.body;
  const selection = body.indexOf("await prepared.selectPortFromUserGesture()");
  const discovery = body.indexOf("await prepared.discover()");
  assert.ok(selection >= 0 && discovery > selection);
  assert.doesNotMatch(body.slice(0, selection), /\bawait\b/);
  assert.match(facade, /const selection = await this\.#host\.selectPortFromUserGesture\(\)/);
  assert.match(facade, /await this\.#host\.discover\(\)/);
  assert.doesNotMatch(facade, /\.discover\s*\(\s*[^)]/);
  assert.match(facadeTypes, /discover\(\): Promise<PrivacyBoundedIdentityResult>/);
});

test("facade exports no command, payload, raw transport, or replaceable host authority", () => {
  assert.deepEqual(
    [...facade.matchAll(/^import .* from "([^"]+)";$/gm)].map((match) => match[1]),
    [
      "virtual:ade-web-readonly-serial-wasm",
      "../transport/webserial-readonly-host.mjs",
    ],
  );
  assert.doesNotMatch(
    `${facade}\n${facadeTypes}`,
    /\b(?:MSP_[A-Z0-9_]+|CommandId|WriteApproval|TransportEffect|sendRaw|writeRaw|commandId|payload)\b/i,
  );
  assert.doesNotMatch(facadeTypes, /constructor|serial\?:|host\?:|bindings|factory/i);
  assert.doesNotMatch(facade, /navigator\s*\.\s*serial|requestPort|getPorts|getInfo/i);
  assert.match(facade, /class PreparedReadonlyFcConnection/);
  assert.doesNotMatch(facade, /export\s+(?:class|\{[^}]*PreparedReadonlyFcConnection)/);
});

test("Vite resolves the generated module only through the normalized deployment base", () => {
  assert.match(viteConfig, /const READONLY_WASM_MODULE_ID = "virtual:ade-web-readonly-serial-wasm"/);
  assert.match(viteConfig, /const base = normalizedBasePath\(process\.env\.ADE_WEB_BASE_PATH\)/);
  assert.match(viteConfig, /readonlyWasmGlue = `\$\{base\}wasm\/ade_web_readonly_serial_wasm_bridge\.js`/);
  assert.match(viteConfig, /readonlyWasmBinary = `\$\{base\}wasm\/ade_web_readonly_serial_wasm_bridge_bg\.wasm`/);
  assert.match(viteConfig, /module_or_path: new URL/);
  assert.match(viteConfig, /external: true/);
  assert.doesNotMatch(viteConfig, /external:\s*\["\/wasm\//);
  assert.doesNotMatch(facade, /["']\/wasm\/ade_web_readonly_serial_wasm_bridge/);
});

test("production browser gate uses a trusted native gesture rather than DOM click injection", () => {
  assert.match(productionBrowser, /Input\.dispatchKeyEvent/);
  assert.match(productionBrowser, /navigator\.userActivation\?\.isActive === true/);
  assert.doesNotMatch(productionBrowser, /\.click\s*\(/);
});

test("production App reuses the exact accepted read-only response fixtures", () => {
  assert.deepEqual(
    numericFixture(productionBrowser, "inScopeReplies"),
    numericFixture(acceptedBrowser, "IN_SCOPE_REPLIES"),
  );
});

test("UI output and state are privacy bounded and make no hardware claim", () => {
  const uiFields = [
    "apiVersion",
    "fcVariant",
    "fcVersion",
    "targetName",
    "scopeMismatchField",
    "failure",
    "failureOrigin",
    "failureStage",
    "failureReason",
  ];
  const internalSelectionEvidence = [
    "readProfileId",
    "readProfileWriteAuthority",
    "capabilityStatus",
    "capabilityPackId",
    "capabilityTrust",
    "capabilityWritePolicy",
  ];
  const internalSnapshotEvidence = [
    "snapshotStatus",
    "beeperOffFlags",
    "dshotBeaconTone",
    "dshotBeaconOffFlags",
    "systemInitDisabled",
  ];
  assert.deepEqual(
    [...app.matchAll(/data-identity-field="([A-Za-z]+)"/g)].map((match) => match[1]),
    uiFields,
  );
  assert.deepEqual(
    [...facade.matchAll(/^    ([A-Za-z]+): result\./gm)].map((match) => match[1]),
    ["outcome", ...uiFields, ...internalSelectionEvidence, ...internalSnapshotEvidence],
  );
  const source = `${app}\n${facade}`;
  assert.doesNotMatch(
    source,
    /serial.?number|\bCOM\d+\b|usbVendorId|usbProductId|\bVID\b|\bPID\b|getInfo|deviceId|unique.?id|raw.?frame/i,
  );
  assert.doesNotMatch(source, /localStorage|sessionStorage|indexedDB|document\.cookie/);
  assert.match(host, /failureStage: discovery\.failureStage \?\? undefined/);
  assert.match(host, /failureReason: discovery\.failureReason \?\? undefined/);
  for (const field of internalSelectionEvidence) {
    assert.match(host, new RegExp(`${field}: discovery\\.${field} \\?\\? undefined`));
  }
  for (const field of internalSnapshotEvidence) {
    assert.match(host, new RegExp(`${field}: discovery\\.${field} \\?\\? undefined`));
  }
  assert.match(
    facadeTypes,
    /"API_VERSION"[\s\S]*"FC_VARIANT"[\s\S]*"FC_VERSION"[\s\S]*"BOARD_INFO"[\s\S]*"BEEPER_CONFIG"/,
  );
  assert.match(
    facadeTypes,
    /"WrongCommand"[\s\S]*"WrongDirection"[\s\S]*"ErrorReply"[\s\S]*"ReplyMisclassified"[\s\S]*"WrongLength"[\s\S]*"FieldOverrun"[\s\S]*"TrailingPayload"[\s\S]*"InvalidUtf8"/,
  );
  assert.doesNotMatch(`${facade}\n${facadeTypes}`, /expectedBytes|foundBytes|rawPayload|signature|uid|serialNumber/i);
  assert.match(facade, /result\.hardwareObserved !== false/);
  assert.doesNotMatch(app, /hardwareObserved/);
  assert.doesNotMatch(
    app,
    /readProfileId|readProfileWriteAuthority|capabilityStatus|capabilityPackId|capabilityTrust|capabilityWritePolicy|snapshotStatus|beeperOffFlags|dshotBeaconTone|dshotBeaconOffFlags|systemInitDisabled/,
  );
  assert.match(facadeTypes, /"never-authorizes-writes"/);
  assert.match(facadeTypes, /"review-only-match"[\s\S]*"no-reviewed-match"/);
  assert.doesNotMatch(facadeTypes, /"not-reviewed"/);
  assert.match(facadeTypes, /capabilityWritePolicy\?: "writes-blocked"/);
  assert.match(facadeTypes, /snapshotStatus\?: "beeper-config-complete"/);
  assert.match(facadeTypes, /systemInitDisabled\?: boolean/);
  assert.match(host, /case "exchange-identification-read":[\s\S]*case "exchange-snapshot-read":/);
  assert.doesNotMatch(app, /\b(?:CONNECTED|SUPPORTED|VALIDATED)\b/);
  for (const phase of [
    "preparing",
    "ready",
    "selecting",
    "reading-identity",
    "read-complete",
    "scope-mismatch",
    "read-only-complete",
    "read-profile-unsupported",
    "api-unsupported",
    "cancelled",
    "unavailable",
    "failed",
  ]) {
    assert.ok(app.includes(`"${phase}"`), `missing bounded UI phase: ${phase}`);
  }
  assert.match(app, /"read-only-complete": "اكتملت قراءة الهوية — هذا الإصدار مدعوم للقراءة فقط"/);
  assert.match(app, /"api-unsupported": "إصدار واجهة وحدة التحكم غير مدعوم حاليًا"/);
  assert.match(app, /connection\.phase !== "api-unsupported"/);
  assert.match(app, /connection\.phase !== "read-profile-unsupported"/);
});

test("diagnostic UI is collapsed, RAM-only, owner-copyable, and owner-clearable", () => {
  assert.match(app, /<details className="diagnostic-trace" data-diagnostic-trace="temporary">/);
  assert.doesNotMatch(app, /<details[^>]*\bopen(?:=|\s|>)/);
  assert.match(app, /prepared\.safeDiagnosticTraceText\(\)/);
  assert.match(app, /readonlyFcConnection\.current\?\.clearDiagnosticTrace\(\)/);
  assert.match(host, /discovery\.takeTraceEvent\(\)/);
  assert.match(diagnostic, /DIAGNOSTIC_TRACE_CAPACITY = 200/);
  assert.match(diagnostic, /capacity < 100 \|\| capacity > 250/);
  assert.match(diagnostic, /this\.#events\.splice\(0, this\.#events\.length - this\.#capacity\)/);
  assert.doesNotMatch(
    `${diagnostic}\n${host}`,
    /console\.|localStorage|sessionStorage|indexedDB|document\.cookie|fetch\(|XMLHttpRequest|WebSocket|sendBeacon/,
  );
  assert.doesNotMatch(
    `${diagnostic}\n${diagnosticTypes}`,
    /rawBytes|rawFrame|rawPayload|usbVendorId|usbProductId|serialNumber|portInfo/i,
  );
  for (const layer of ["UI", "HOST", "RUST", "SERIAL", "MSP", "CLEANUP"]) {
    assert.ok(diagnostic.includes(`"${layer}"`), `missing diagnostic layer: ${layer}`);
  }
  for (const origin of [
    "PORT_SELECTION",
    "PORT_OPEN",
    "WRITER_ACQUISITION",
    "READER_ACQUISITION",
    "SERIAL_WRITE",
    "SERIAL_READ",
    "SERIAL_TIMEOUT",
    "MSP_FRAME",
    "IDENTITY_STAGE",
    "DIRECTIVE_REFUSAL",
    "PORT_CLOSE",
    "UI_BOUNDARY",
  ]) {
    assert.ok(diagnostic.includes(`"${origin}"`), `missing fixed diagnostic origin: ${origin}`);
  }
});
