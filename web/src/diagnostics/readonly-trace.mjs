export const DIAGNOSTIC_TRACE_CAPACITY = 200;

export const DIAGNOSTIC_LAYERS = Object.freeze([
  "UI",
  "HOST",
  "RUST",
  "SERIAL",
  "MSP",
  "CLEANUP",
]);

export const DIAGNOSTIC_PHASES = Object.freeze([
  "PORT_SELECTION",
  "DISCOVERY",
  "PORT_OPEN",
  "API_VERSION",
  "FC_VARIANT",
  "FC_VERSION",
  "BOARD_INFO",
  "BEEPER_CONFIG",
  "SERIAL_WRITE",
  "SERIAL_READ",
  "MSP_FRAME",
  "IDENTITY_STAGE",
  "SNAPSHOT_STAGE",
  "PORT_CLOSE",
  "CLEANUP",
  "UI_BOUNDARY",
  "FINAL_RESULT",
]);

export const DIAGNOSTIC_EVENTS = Object.freeze([
  "SELECT_START",
  "SELECT_OK",
  "SELECT_FAILED",
  "DISCOVERY_START",
  "PORT_OPEN_START",
  "PORT_OPEN_OK",
  "PORT_OPEN_FAILED",
  "DIRECTIVE",
  "TX_START",
  "TX_OK",
  "TX_FAILED",
  "RX_CHUNK",
  "RX_FAILED",
  "FRAME_ACCEPTED",
  "FRAME_REJECTED",
  "IDENTITY_STAGE_OK",
  "IDENTITY_STAGE_FAILED",
  "SNAPSHOT_STAGE_OK",
  "SNAPSHOT_STAGE_FAILED",
  "PORT_CLOSE_START",
  "PORT_CLOSE_OK",
  "PORT_CLOSE_FAILED",
  "CLEANUP_START",
  "CLEANUP_OK",
  "CLEANUP_FAILED",
  "UI_BOUNDARY_FAILED",
  "FINAL_OK",
  "FINAL_FAILED",
]);

export const DIAGNOSTIC_STAGES = Object.freeze([
  "API_VERSION",
  "FC_VARIANT",
  "FC_VERSION",
  "BOARD_INFO",
  "BEEPER_CONFIG",
]);

export const DIAGNOSTIC_COMMANDS = Object.freeze([
  "MSP_API_VERSION",
  "MSP_FC_VARIANT",
  "MSP_FC_VERSION",
  "MSP_BOARD_INFO",
  "MSP_BEEPER_CONFIG",
]);

export const DIAGNOSTIC_DIRECTIONS = Object.freeze([
  "REQUEST",
  "REPLY",
  "ERROR",
]);

export const DIAGNOSTIC_FAILURE_CLASSES = Object.freeze([
  "Unavailable",
  "Cancelled",
  "PermissionDenied",
  "PortBusy",
  "Disconnected",
  "Timeout",
  "MalformedResponse",
  "ProtocolIdentityFailure",
  "ProtocolSnapshotFailure",
  "HardwareEvidenceBoundary",
  "CloseFailure",
  "Unknown",
]);

export const DIAGNOSTIC_FAILURE_REASONS = Object.freeze([
  "PayloadTooLong",
  "FrameTooLarge",
  "Truncated",
  "TrailingBytes",
  "BadPreamble",
  "BadDirection",
  "BadChecksum",
  "WrongLength",
  "WrongCommand",
  "WrongDirection",
  "ErrorReply",
  "ReplyMisclassified",
  "FieldOverrun",
  "TrailingPayload",
  "InvalidUtf8",
  "OtherProtocolIdentityFailure",
]);

export const DIAGNOSTIC_ORIGINS = Object.freeze([
  "PORT_SELECTION",
  "DISCOVERY",
  "PORT_OPEN",
  "WRITER_ACQUISITION",
  "READER_ACQUISITION",
  "SERIAL_WRITE",
  "SERIAL_READ",
  "SERIAL_TIMEOUT",
  "MSP_FRAME",
  "IDENTITY_STAGE",
  "SNAPSHOT_STAGE",
  "DIRECTIVE_REFUSAL",
  "PORT_CLOSE",
  "READER_CANCEL",
  "READER_RELEASE",
  "WRITER_RELEASE",
  "CLEANUP",
  "UI_BOUNDARY",
  "FINAL_RESULT",
]);

const ALLOWED_KEYS = new Set([
  "layer",
  "phase",
  "event",
  "stage",
  "command",
  "byteCount",
  "direction",
  "failureClass",
  "failureReason",
  "origin",
]);

const sets = Object.freeze({
  layer: new Set(DIAGNOSTIC_LAYERS),
  phase: new Set(DIAGNOSTIC_PHASES),
  event: new Set(DIAGNOSTIC_EVENTS),
  stage: new Set(DIAGNOSTIC_STAGES),
  command: new Set(DIAGNOSTIC_COMMANDS),
  direction: new Set(DIAGNOSTIC_DIRECTIONS),
  failureClass: new Set(DIAGNOSTIC_FAILURE_CLASSES),
  failureReason: new Set(DIAGNOSTIC_FAILURE_REASONS),
  origin: new Set(DIAGNOSTIC_ORIGINS),
});

function requireToken(field, value) {
  if (!sets[field].has(value)) {
    throw new TypeError(`unsupported diagnostic ${field}`);
  }
  return value;
}

function normalizeEvent(candidate) {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
    throw new TypeError("diagnostic event must be an object");
  }

  for (const key of Object.keys(candidate)) {
    if (!ALLOWED_KEYS.has(key)) {
      throw new TypeError("unsupported diagnostic field");
    }
  }

  const normalized = {
    layer: requireToken("layer", candidate.layer),
    phase: requireToken("phase", candidate.phase),
    event: requireToken("event", candidate.event),
  };

  for (const field of [
    "stage",
    "command",
    "direction",
    "failureClass",
    "failureReason",
    "origin",
  ]) {
    if (candidate[field] !== undefined) {
      normalized[field] = requireToken(field, candidate[field]);
    }
  }

  if (candidate.byteCount !== undefined) {
    if (
      !Number.isSafeInteger(candidate.byteCount) ||
      candidate.byteCount < 0 ||
      candidate.byteCount > 65_535
    ) {
      throw new TypeError("unsupported diagnostic byteCount");
    }
    normalized.byteCount = candidate.byteCount;
  }

  if (
    (normalized.event.endsWith("_FAILED") || normalized.failureClass) &&
    !normalized.origin
  ) {
    throw new TypeError("diagnostic failure events require a fixed origin");
  }

  return normalized;
}

export class DiagnosticTraceRecorder {
  #capacity;
  #events = [];
  #nextSequence = 1;

  constructor(capacity = DIAGNOSTIC_TRACE_CAPACITY) {
    if (!Number.isSafeInteger(capacity) || capacity < 100 || capacity > 250) {
      throw new TypeError("diagnostic trace capacity must be between 100 and 250");
    }
    this.#capacity = capacity;
  }

  beginAttempt() {
    this.clear();
  }

  record(candidate) {
    const normalized = normalizeEvent(candidate);
    const entry = Object.freeze({
      sequence: this.#nextSequence,
      ...normalized,
    });

    this.#nextSequence += 1;
    this.#events.push(entry);
    if (this.#events.length > this.#capacity) {
      this.#events.splice(0, this.#events.length - this.#capacity);
    }
    return entry;
  }

  snapshot() {
    return Object.freeze(this.#events.slice());
  }

  clear() {
    this.#events.length = 0;
    this.#nextSequence = 1;
  }
}

export function formatSafeDiagnosticTrace(events) {
  if (!Array.isArray(events)) {
    throw new TypeError("diagnostic trace snapshot must be an array");
  }

  const lines = ["ADE_READONLY_DIAGNOSTIC_TRACE_V1"];
  for (const candidate of events) {
    if (!Number.isSafeInteger(candidate?.sequence) || candidate.sequence < 1) {
      throw new TypeError("diagnostic trace sequence is invalid");
    }
    const { sequence: _sequence, ...candidateEvent } = candidate;
    const event = normalizeEvent(candidateEvent);
    const fields = [
      `sequence=${candidate.sequence}`,
      `layer=${event.layer}`,
      `phase=${event.phase}`,
      `event=${event.event}`,
    ];
    for (const field of [
      "stage",
      "command",
      "byteCount",
      "direction",
      "failureClass",
      "failureReason",
      "origin",
    ]) {
      if (event[field] !== undefined) {
        fields.push(`${field}=${event[field]}`);
      }
    }
    lines.push(fields.join(" "));
  }
  return `${lines.join("\n")}\n`;
}
