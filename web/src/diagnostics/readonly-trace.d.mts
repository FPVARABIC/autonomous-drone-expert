export type DiagnosticLayer = "UI" | "HOST" | "RUST" | "SERIAL" | "MSP" | "CLEANUP";
export type DiagnosticPhase =
  | "PORT_SELECTION"
  | "DISCOVERY"
  | "PORT_OPEN"
  | "API_VERSION"
  | "FC_VARIANT"
  | "FC_VERSION"
  | "BOARD_INFO"
  | "BEEPER_CONFIG"
  | "SERIAL_WRITE"
  | "SERIAL_READ"
  | "MSP_FRAME"
  | "IDENTITY_STAGE"
  | "SNAPSHOT_STAGE"
  | "PORT_CLOSE"
  | "CLEANUP"
  | "UI_BOUNDARY"
  | "FINAL_RESULT";
export type DiagnosticEvent =
  | "SELECT_START"
  | "SELECT_OK"
  | "SELECT_FAILED"
  | "DISCOVERY_START"
  | "PORT_OPEN_START"
  | "PORT_OPEN_OK"
  | "PORT_OPEN_FAILED"
  | "DIRECTIVE"
  | "TX_START"
  | "TX_OK"
  | "TX_FAILED"
  | "RX_CHUNK"
  | "RX_FAILED"
  | "FRAME_ACCEPTED"
  | "FRAME_REJECTED"
  | "IDENTITY_STAGE_OK"
  | "IDENTITY_STAGE_FAILED"
  | "SNAPSHOT_STAGE_OK"
  | "SNAPSHOT_STAGE_FAILED"
  | "PORT_CLOSE_START"
  | "PORT_CLOSE_OK"
  | "PORT_CLOSE_FAILED"
  | "CLEANUP_START"
  | "CLEANUP_OK"
  | "CLEANUP_FAILED"
  | "UI_BOUNDARY_FAILED"
  | "FINAL_OK"
  | "FINAL_FAILED";
export type DiagnosticStage =
  | "API_VERSION"
  | "FC_VARIANT"
  | "FC_VERSION"
  | "BOARD_INFO"
  | "BEEPER_CONFIG";
export type DiagnosticCommand =
  | "MSP_API_VERSION"
  | "MSP_FC_VARIANT"
  | "MSP_FC_VERSION"
  | "MSP_BOARD_INFO"
  | "MSP_BEEPER_CONFIG";
export type DiagnosticDirection = "REQUEST" | "REPLY" | "ERROR";
export type DiagnosticFailureClass =
  | "Unavailable"
  | "Cancelled"
  | "PermissionDenied"
  | "PortBusy"
  | "Disconnected"
  | "Timeout"
  | "MalformedResponse"
  | "ProtocolIdentityFailure"
  | "ProtocolSnapshotFailure"
  | "HardwareEvidenceBoundary"
  | "CloseFailure"
  | "Unknown";
export type DiagnosticFailureReason =
  | "PayloadTooLong"
  | "FrameTooLarge"
  | "Truncated"
  | "TrailingBytes"
  | "BadPreamble"
  | "BadDirection"
  | "BadChecksum"
  | "WrongLength"
  | "WrongCommand"
  | "WrongDirection"
  | "ErrorReply"
  | "ReplyMisclassified"
  | "FieldOverrun"
  | "TrailingPayload"
  | "InvalidUtf8"
  | "OtherProtocolIdentityFailure";
export type DiagnosticOrigin =
  | "PORT_SELECTION"
  | "DISCOVERY"
  | "PORT_OPEN"
  | "WRITER_ACQUISITION"
  | "READER_ACQUISITION"
  | "SERIAL_WRITE"
  | "SERIAL_READ"
  | "SERIAL_TIMEOUT"
  | "MSP_FRAME"
  | "IDENTITY_STAGE"
  | "SNAPSHOT_STAGE"
  | "DIRECTIVE_REFUSAL"
  | "PORT_CLOSE"
  | "READER_CANCEL"
  | "READER_RELEASE"
  | "WRITER_RELEASE"
  | "CLEANUP"
  | "UI_BOUNDARY"
  | "FINAL_RESULT";

export interface DiagnosticTraceEvent {
  readonly sequence: number;
  readonly layer: DiagnosticLayer;
  readonly phase: DiagnosticPhase;
  readonly event: DiagnosticEvent;
  readonly stage?: DiagnosticStage;
  readonly command?: DiagnosticCommand;
  readonly byteCount?: number;
  readonly direction?: DiagnosticDirection;
  readonly failureClass?: DiagnosticFailureClass;
  readonly failureReason?: DiagnosticFailureReason;
  readonly origin?: DiagnosticOrigin;
}

export declare const DIAGNOSTIC_TRACE_CAPACITY: 200;
export declare class DiagnosticTraceRecorder {
  constructor(capacity?: number);
  beginAttempt(): void;
  record(candidate: Omit<DiagnosticTraceEvent, "sequence">): DiagnosticTraceEvent;
  snapshot(): readonly DiagnosticTraceEvent[];
  clear(): void;
}
export declare function formatSafeDiagnosticTrace(events: readonly DiagnosticTraceEvent[]): string;
