import type {
  DiagnosticOrigin,
  DiagnosticTraceEvent,
} from "../diagnostics/readonly-trace.mjs";

export type WebSerialFailure =
  | "Unavailable"
  | "Cancelled"
  | "PermissionDenied"
  | "PortBusy"
  | "Disconnected"
  | "Timeout"
  | "Unknown";

export interface PortSelectionResult {
  ok: boolean;
  failure?: WebSerialFailure;
  failureOrigin?: DiagnosticOrigin;
}

export type IdentityFailureStage =
  | "API_VERSION"
  | "FC_VARIANT"
  | "FC_VERSION"
  | "BOARD_INFO";

export type IdentityFailureReason =
  | "PayloadTooLong"
  | "FrameTooLarge"
  | "Truncated"
  | "TrailingBytes"
  | "BadPreamble"
  | "BadDirection"
  | "BadChecksum"
  | "WrongCommand"
  | "WrongDirection"
  | "ErrorReply"
  | "ReplyMisclassified"
  | "WrongLength"
  | "FieldOverrun"
  | "TrailingPayload"
  | "InvalidUtf8"
  | "OtherProtocolIdentityFailure";

export type ReadProfileId =
  | "api-1.46-legacy"
  | "api-1.47-calendar-extended";

export type ReadProfileWriteAuthority = "never-authorizes-writes";

export type CapabilitySelectionStatus =
  | "review-only-match"
  | "no-reviewed-match"
  | "unknown-firmware-family"
  | "ambiguous"
  | "invalid-pack"
  | "not-reviewed";

export interface ReadonlyDiscoveryResult {
  outcome:
    | "in-scope"
    | "scope-mismatch"
    | "read-only-complete"
    | "read-profile-unsupported"
    | "api-unsupported"
    | "failed"
    | "pending";
  failure?: string;
  failureOrigin?: DiagnosticOrigin;
  failureStage?: IdentityFailureStage;
  failureReason?: IdentityFailureReason;
  scopeMismatchField?: string;
  apiVersion?: string;
  fcVariant?: string;
  fcVersion?: string;
  targetName?: string;
  readProfileId?: ReadProfileId;
  readProfileWriteAuthority?: ReadProfileWriteAuthority;
  capabilityStatus?: CapabilitySelectionStatus;
  capabilityPackId?: string;
  capabilityTrust?: "review-only-embedded";
  capabilityWritePolicy?: "writes-blocked";
  hardwareObserved: false;
}

export declare class WebSerialReadonlyHost {
  constructor(options?: { serial?: object; timeoutMs?: number });
  selectPortFromUserGesture(): Promise<PortSelectionResult>;
  discover(): Promise<ReadonlyDiscoveryResult>;
  recordUiBoundaryFailure(): void;
  recordHardwareEvidenceBoundary(): void;
  diagnosticTrace(): readonly DiagnosticTraceEvent[];
  safeDiagnosticTraceText(): string;
  clearDiagnosticTrace(): void;
}

export declare const WEB_SERIAL_READONLY_INITIAL_BAUD_RATE: 115200;
