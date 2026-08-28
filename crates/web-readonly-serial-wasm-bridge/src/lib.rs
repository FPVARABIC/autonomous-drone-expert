#![forbid(unsafe_code)]

//! Dedicated WebAssembly facade for bounded read-only Web Serial discovery and snapshot.
//!
//! JavaScript receives only four directive labels: open the explicitly selected port, exchange an
//! identity read, exchange the one reviewed snapshot read, and close. The API 1.46 path may issue the
//! exact four-read identity prefix plus one beeper snapshot read. JavaScript cannot select a
//! command, construct an MSP frame, provide a `WriteApproval`, or obtain a generic transport
//! effect. Raw response chunks return to the bounded Rust MSP accumulator.

use core::fmt;
use std::collections::VecDeque;

use ade_capability::{
    CapabilityPackTrust, CapabilityPackWritePolicy, m3_review_only_betaflight_4_5_5_pack,
};
use ade_capability_resolution::{
    ReviewOnlyCapabilityStatus, resolve_review_only_capability,
    resolve_review_only_read_profile_capability,
};
use ade_core_api::{ScopeStatus, check_scope};
use ade_execution::{
    ExecError, IdentificationProgress, IdentificationRequest, IdentificationStage,
    ReadonlyBeeperSnapshotRead, ReadonlyIdentification, ReadonlyProfileIdentity,
};
use ade_facts::DeviceIdentity;
use ade_protocol_msp::{
    ApiVersion, BeeperConfigSnapshot, CommandId, Direction, MspError, MspV1ResponseAccumulator,
    ResponseProgress, decode_frame,
};
use ade_readonly_profile::{ReadProfileWriteAuthority, ReadonlyIdentityProfileId};
use ade_runtime_ports::{
    BoundaryError, IoCoordinator, IoEffect, IoResponse, OutboundPacket, RequestId, TransportEffect,
    TransportFailure, TransportResult,
};
use ade_safety::WriteCommandClass;
use ade_session::SessionState;
use wasm_bindgen::prelude::*;

#[derive(Debug)]
enum BridgeError {
    Boundary,
    Execution,
    Protocol,
    InvalidDecimal,
    InvalidFailure,
    InvalidState,
    NonTransportEffect,
    DirectiveRefused,
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let marker = match self {
            Self::Boundary => "BOUNDARY",
            Self::Execution => "IDENTITY",
            Self::Protocol => "PROTOCOL",
            Self::InvalidDecimal => "INVALID_REQUEST_ID_DECIMAL",
            Self::InvalidFailure => "INVALID_TRANSPORT_FAILURE",
            Self::InvalidState => "INVALID_STATE",
            Self::NonTransportEffect => "NON_TRANSPORT_EFFECT",
            Self::DirectiveRefused => "DIRECTIVE_REFUSED",
        };
        write!(formatter, "RUST_WEB_SERIAL_REFUSAL:{marker}")
    }
}

impl From<BoundaryError> for BridgeError {
    fn from(_error: BoundaryError) -> Self {
        Self::Boundary
    }
}

impl From<ExecError> for BridgeError {
    fn from(_error: ExecError) -> Self {
        Self::Execution
    }
}

impl From<MspError> for BridgeError {
    fn from(_error: MspError) -> Self {
        Self::Protocol
    }
}

fn js_error(error: BridgeError) -> JsError {
    JsError::new(&error.to_string())
}

fn parse_request_id(value: &str) -> Result<RequestId, BridgeError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(BridgeError::InvalidDecimal);
    }
    value
        .parse::<u64>()
        .map(RequestId::new)
        .map_err(|_| BridgeError::InvalidDecimal)
}

fn parse_failure(value: &str) -> Result<TransportFailure, BridgeError> {
    match value {
        "PortBusy" => Ok(TransportFailure::PortBusy),
        "PermissionDenied" => Ok(TransportFailure::PermissionDenied),
        "Unavailable" | "MissingDriver" => Ok(TransportFailure::MissingDriver),
        "Disconnected" => Ok(TransportFailure::Disconnected),
        "Timeout" => Ok(TransportFailure::Timeout),
        "Cancelled" => Ok(TransportFailure::Cancelled),
        "Unknown" => Ok(TransportFailure::Unknown),
        _ => Err(BridgeError::InvalidFailure),
    }
}

const fn failure_label(failure: TransportFailure) -> &'static str {
    match failure {
        TransportFailure::PortBusy => "PortBusy",
        TransportFailure::PermissionDenied => "PermissionDenied",
        TransportFailure::MissingDriver => "Unavailable",
        TransportFailure::Disconnected => "Disconnected",
        TransportFailure::Timeout => "Timeout",
        TransportFailure::Cancelled => "Cancelled",
        TransportFailure::Unknown => "Unknown",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Opening,
    Exchanging,
    Closing,
    Complete,
}

#[derive(Debug)]
enum FinalOutcome {
    InScope {
        identity: DeviceIdentity,
        selection: ReadSelectionEvidence,
        snapshot: BeeperConfigSnapshot,
    },
    ScopeMismatch {
        identity: DeviceIdentity,
        field: &'static str,
        selection: ReadSelectionEvidence,
    },
    ApiScopeMismatch {
        api: ApiVersion,
        field: &'static str,
    },
    ReadOnlyComplete {
        identity: ReadonlyProfileIdentity,
        selection: ReadSelectionEvidence,
    },
    ReadProfileMismatch {
        api: ApiVersion,
        field: &'static str,
    },
    Failed {
        class: &'static str,
        diagnostic: Option<ReadonlyFailureDiagnostic>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadonlyStage {
    ApiVersion,
    FcVariant,
    FcVersion,
    BoardInfo,
    BeeperConfig,
}

impl ReadonlyStage {
    fn from_identification(stage: IdentificationStage) -> Result<Self, BridgeError> {
        match stage {
            IdentificationStage::ApiVersion => Ok(Self::ApiVersion),
            IdentificationStage::FcVariant => Ok(Self::FcVariant),
            IdentificationStage::FcVersion => Ok(Self::FcVersion),
            IdentificationStage::BoardInfo => Ok(Self::BoardInfo),
            IdentificationStage::Complete => Err(BridgeError::InvalidState),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::ApiVersion => "API_VERSION",
            Self::FcVariant => "FC_VARIANT",
            Self::FcVersion => "FC_VERSION",
            Self::BoardInfo => "BOARD_INFO",
            Self::BeeperConfig => "BEEPER_CONFIG",
        }
    }

    const fn command(self) -> CommandId {
        match self {
            Self::ApiVersion => CommandId::ApiVersion,
            Self::FcVariant => CommandId::FcVariant,
            Self::FcVersion => CommandId::FcVersion,
            Self::BoardInfo => CommandId::BoardInfo,
            Self::BeeperConfig => CommandId::BeeperConfig,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReadSelectionEvidence {
    profile_id: ReadonlyIdentityProfileId,
    write_authority: ReadProfileWriteAuthority,
    capability: CapabilitySelectionEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilitySelectionEvidence {
    Match {
        pack_id: &'static str,
        trust: CapabilityPackTrust,
        write_policy: CapabilityPackWritePolicy,
    },
    NoMatch,
    UnknownFirmwareFamily,
    Ambiguous,
    InvalidPack,
}

fn capability_status_evidence(status: ReviewOnlyCapabilityStatus) -> CapabilitySelectionEvidence {
    match status {
        ReviewOnlyCapabilityStatus::Match {
            pack_id,
            trust,
            write_policy,
        } => CapabilitySelectionEvidence::Match {
            pack_id,
            trust,
            write_policy,
        },
        ReviewOnlyCapabilityStatus::NoMatch => CapabilitySelectionEvidence::NoMatch,
        ReviewOnlyCapabilityStatus::UnknownFirmwareFamily => {
            CapabilitySelectionEvidence::UnknownFirmwareFamily
        }
        ReviewOnlyCapabilityStatus::Ambiguous => CapabilitySelectionEvidence::Ambiguous,
        ReviewOnlyCapabilityStatus::InvalidPack { .. } => CapabilitySelectionEvidence::InvalidPack,
    }
}

fn capability_evidence(identity: &DeviceIdentity) -> CapabilitySelectionEvidence {
    capability_status_evidence(resolve_review_only_capability(identity))
}

fn legacy_selection_evidence(
    identity: &DeviceIdentity,
) -> Result<ReadSelectionEvidence, BridgeError> {
    if identity.api.protocol_version != 0
        || identity.api.api_major != 1
        || identity.api.api_minor != 46
        || identity.variant.identifier != *b"BTFL"
    {
        return Err(BridgeError::InvalidState);
    }
    Ok(ReadSelectionEvidence {
        profile_id: ReadonlyIdentityProfileId::BetaflightApi146Legacy,
        write_authority: ReadProfileWriteAuthority::NeverAuthorizesWrites,
        capability: capability_evidence(identity),
    })
}

fn readonly_selection_evidence(
    identity: &ReadonlyProfileIdentity,
) -> Result<ReadSelectionEvidence, BridgeError> {
    if identity.write_authority != ReadProfileWriteAuthority::NeverAuthorizesWrites {
        return Err(BridgeError::InvalidState);
    }
    let capability = match identity.profile_id {
        ReadonlyIdentityProfileId::BetaflightApi147CalendarExtended => {
            capability_status_evidence(resolve_review_only_read_profile_capability(
                &identity.api,
                &identity.variant,
                &identity.version,
                &identity.target_name,
            ))
        }
        ReadonlyIdentityProfileId::BetaflightApi146Legacy => {
            return Err(BridgeError::InvalidState);
        }
    };
    Ok(ReadSelectionEvidence {
        profile_id: identity.profile_id,
        write_authority: identity.write_authority,
        capability,
    })
}

const fn read_profile_label(profile_id: ReadonlyIdentityProfileId) -> &'static str {
    match profile_id {
        ReadonlyIdentityProfileId::BetaflightApi146Legacy => "api-1.46-legacy",
        ReadonlyIdentityProfileId::BetaflightApi147CalendarExtended => "api-1.47-calendar-extended",
    }
}

const fn read_profile_write_authority_label(
    write_authority: ReadProfileWriteAuthority,
) -> &'static str {
    match write_authority {
        ReadProfileWriteAuthority::NeverAuthorizesWrites => "never-authorizes-writes",
    }
}

const fn capability_status_label(capability: CapabilitySelectionEvidence) -> &'static str {
    match capability {
        CapabilitySelectionEvidence::Match { .. } => "review-only-match",
        CapabilitySelectionEvidence::NoMatch => "no-reviewed-match",
        CapabilitySelectionEvidence::UnknownFirmwareFamily => "unknown-firmware-family",
        CapabilitySelectionEvidence::Ambiguous => "ambiguous",
        CapabilitySelectionEvidence::InvalidPack => "invalid-pack",
    }
}

const fn capability_trust_label(trust: CapabilityPackTrust) -> &'static str {
    match trust {
        CapabilityPackTrust::ReviewOnlyEmbedded => "review-only-embedded",
    }
}

const fn capability_write_policy_label(policy: CapabilityPackWritePolicy) -> &'static str {
    match policy {
        CapabilityPackWritePolicy::WritesBlocked => "writes-blocked",
    }
}

fn api146_snapshot_is_reviewed(selection: ReadSelectionEvidence) -> bool {
    if selection.profile_id != ReadonlyIdentityProfileId::BetaflightApi146Legacy
        || selection.write_authority != ReadProfileWriteAuthority::NeverAuthorizesWrites
    {
        return false;
    }
    match selection.capability {
        CapabilitySelectionEvidence::Match {
            pack_id,
            trust: CapabilityPackTrust::ReviewOnlyEmbedded,
            write_policy: CapabilityPackWritePolicy::WritesBlocked,
        } => pack_id == m3_review_only_betaflight_4_5_5_pack().pack_id,
        CapabilitySelectionEvidence::NoMatch
        | CapabilitySelectionEvidence::UnknownFirmwareFamily
        | CapabilitySelectionEvidence::Ambiguous
        | CapabilitySelectionEvidence::InvalidPack => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentityFailureReason {
    PayloadTooLong,
    FrameTooLarge,
    Truncated,
    TrailingBytes,
    BadPreamble,
    BadDirection,
    BadChecksum,
    WrongCommand,
    WrongDirection,
    ErrorReply,
    ReplyMisclassified,
    WrongLength,
    FieldOverrun,
    TrailingPayload,
    InvalidUtf8,
    OtherProtocolIdentityFailure,
}

impl IdentityFailureReason {
    const fn from_msp(error: &MspError) -> Self {
        match error {
            MspError::PayloadTooLong(_) => Self::PayloadTooLong,
            MspError::FrameTooLarge { .. } => Self::FrameTooLarge,
            MspError::Truncated { .. } => Self::Truncated,
            MspError::TrailingBytes { .. } => Self::TrailingBytes,
            MspError::BadPreamble => Self::BadPreamble,
            MspError::BadDirection(_) => Self::BadDirection,
            MspError::BadChecksum { .. } => Self::BadChecksum,
            MspError::WrongCommand { .. } => Self::WrongCommand,
            MspError::WrongDirection => Self::WrongDirection,
            MspError::WrongLength { .. } => Self::WrongLength,
            MspError::FieldOverrun { .. } => Self::FieldOverrun,
            MspError::TrailingPayload { .. } => Self::TrailingPayload,
            MspError::InvalidUtf8 { .. } => Self::InvalidUtf8,
        }
    }

    const fn from_exec(error: &ExecError) -> Self {
        match error {
            ExecError::Payload(error) => Self::from_msp(error),
            ExecError::ReplyCommandMismatch { .. } => Self::WrongCommand,
            ExecError::ReplyDirectionInvalid => Self::WrongDirection,
            ExecError::ErrorReply { .. } => Self::ErrorReply,
            ExecError::ReplyMisclassified(_) => Self::ReplyMisclassified,
            _ => Self::OtherProtocolIdentityFailure,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::PayloadTooLong => "PayloadTooLong",
            Self::FrameTooLarge => "FrameTooLarge",
            Self::Truncated => "Truncated",
            Self::TrailingBytes => "TrailingBytes",
            Self::BadPreamble => "BadPreamble",
            Self::BadDirection => "BadDirection",
            Self::BadChecksum => "BadChecksum",
            Self::WrongCommand => "WrongCommand",
            Self::WrongDirection => "WrongDirection",
            Self::ErrorReply => "ErrorReply",
            Self::ReplyMisclassified => "ReplyMisclassified",
            Self::WrongLength => "WrongLength",
            Self::FieldOverrun => "FieldOverrun",
            Self::TrailingPayload => "TrailingPayload",
            Self::InvalidUtf8 => "InvalidUtf8",
            Self::OtherProtocolIdentityFailure => "OtherProtocolIdentityFailure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReadonlyFailureDiagnostic {
    stage: ReadonlyStage,
    reason: IdentityFailureReason,
}

impl ReadonlyFailureDiagnostic {
    const fn from_msp(stage: ReadonlyStage, error: &MspError) -> Self {
        Self {
            stage,
            reason: IdentityFailureReason::from_msp(error),
        }
    }

    const fn from_exec(stage: ReadonlyStage, error: &ExecError) -> Self {
        Self {
            stage,
            reason: IdentityFailureReason::from_exec(error),
        }
    }

    const fn stage_label(self) -> &'static str {
        self.stage.label()
    }
}

const TRACE_EVENT_LIMIT: usize = 32;

const fn command_label(command: CommandId) -> Option<&'static str> {
    match command {
        CommandId::ApiVersion => Some("MSP_API_VERSION"),
        CommandId::FcVariant => Some("MSP_FC_VARIANT"),
        CommandId::FcVersion => Some("MSP_FC_VERSION"),
        CommandId::BoardInfo => Some("MSP_BOARD_INFO"),
        CommandId::BeeperConfig => Some("MSP_BEEPER_CONFIG"),
        _ => None,
    }
}

const fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::Request => "REQUEST",
        Direction::Reply => "REPLY",
        Direction::Error => "ERROR",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RustTraceEvent {
    layer: &'static str,
    phase: &'static str,
    event: &'static str,
    stage: ReadonlyStage,
    command: CommandId,
    byte_count: Option<u32>,
    direction: Option<Direction>,
    failure_class: Option<&'static str>,
    failure_reason: Option<IdentityFailureReason>,
    origin: Option<&'static str>,
}

/// One privacy-bounded protocol event emitted by the authoritative Rust state machine.
///
/// Every string getter returns a member of a fixed vocabulary. No raw frame, payload,
/// identity value, browser error or user-controlled string is retained here.
#[wasm_bindgen]
pub struct WasmReadonlyTraceEvent {
    event: RustTraceEvent,
}

#[wasm_bindgen]
impl WasmReadonlyTraceEvent {
    #[wasm_bindgen(getter)]
    pub fn layer(&self) -> String {
        self.event.layer.to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn phase(&self) -> String {
        self.event.phase.to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn event(&self) -> String {
        self.event.event.to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn stage(&self) -> Option<String> {
        Some(self.event.stage.label().to_owned())
    }

    #[wasm_bindgen(getter)]
    pub fn command(&self) -> Option<String> {
        command_label(self.event.command).map(str::to_owned)
    }

    #[wasm_bindgen(getter, js_name = byteCount)]
    pub fn byte_count(&self) -> Option<u32> {
        self.event.byte_count
    }

    #[wasm_bindgen(getter)]
    pub fn direction(&self) -> Option<String> {
        self.event.direction.map(direction_label).map(str::to_owned)
    }

    #[wasm_bindgen(getter, js_name = failureClass)]
    pub fn failure_class(&self) -> Option<String> {
        self.event.failure_class.map(str::to_owned)
    }

    #[wasm_bindgen(getter, js_name = failureReason)]
    pub fn failure_reason(&self) -> Option<String> {
        self.event
            .failure_reason
            .map(IdentityFailureReason::label)
            .map(str::to_owned)
    }

    #[wasm_bindgen(getter)]
    pub fn origin(&self) -> Option<String> {
        self.event.origin.map(str::to_owned)
    }
}

/// One opaque host operation created by the Rust discovery state machine.
#[wasm_bindgen]
pub struct WasmReadonlySerialDirective {
    request_id: String,
    kind: &'static str,
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl WasmReadonlySerialDirective {
    #[wasm_bindgen(getter, js_name = requestId)]
    pub fn request_id(&self) -> String {
        self.request_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.to_owned()
    }

    /// Bytes exist only on a fixed identification or snapshot read directive and were already
    /// authorised and framed by Rust.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

const fn readonly_directive_kind(command: CommandId) -> Option<&'static str> {
    match command {
        CommandId::ApiVersion
        | CommandId::FcVariant
        | CommandId::FcVersion
        | CommandId::BoardInfo => Some("exchange-identification-read"),
        CommandId::BeeperConfig => Some("exchange-snapshot-read"),
        CommandId::SetBeeperConfig | CommandId::EepromWrite | CommandId::Reboot => None,
    }
}

fn directive_from(
    effect: IoEffect,
    expected_command: Option<CommandId>,
) -> Result<WasmReadonlySerialDirective, BridgeError> {
    match effect {
        IoEffect::Transport {
            request_id,
            effect: TransportEffect::OpenSelectedReadOnlyPort,
        } if expected_command.is_none() => Ok(WasmReadonlySerialDirective {
            request_id: request_id.get().to_string(),
            kind: "open-selected-read-only-port",
            bytes: Vec::new(),
        }),
        IoEffect::Transport {
            request_id,
            effect: TransportEffect::Close,
        } if expected_command.is_none() => Ok(WasmReadonlySerialDirective {
            request_id: request_id.get().to_string(),
            kind: "close",
            bytes: Vec::new(),
        }),
        IoEffect::Transport {
            request_id,
            effect: TransportEffect::Exchange(packet),
        } => {
            let expected = expected_command.ok_or(BridgeError::DirectiveRefused)?;
            if packet.class() != WriteCommandClass::NoWrite
                || packet.approval().is_some()
                || packet.approved_target().is_some()
                || packet.approved_recovery().is_some()
            {
                return Err(BridgeError::DirectiveRefused);
            }
            let frame = decode_frame(packet.bytes())?;
            let command = frame.known_command().ok_or(BridgeError::DirectiveRefused)?;
            let kind = readonly_directive_kind(command).ok_or(BridgeError::DirectiveRefused)?;
            if frame.direction != Direction::Request
                || command != expected
                || frame.payload_len() != 0
            {
                return Err(BridgeError::DirectiveRefused);
            }
            Ok(WasmReadonlySerialDirective {
                request_id: request_id.get().to_string(),
                kind,
                bytes: packet.bytes().to_vec(),
            })
        }
        IoEffect::Transport { .. } => Err(BridgeError::DirectiveRefused),
        IoEffect::Storage { .. } => Err(BridgeError::NonTransportEffect),
    }
}

/// Rust-owned read-only discovery state exposed through a narrow WebAssembly ABI.
#[wasm_bindgen]
pub struct WasmReadonlySerialDiscovery {
    coordinator: IoCoordinator,
    identification: ReadonlyIdentification,
    snapshot_read: Option<ReadonlyBeeperSnapshotRead>,
    pending_snapshot_identity: Option<DeviceIdentity>,
    pending_snapshot_selection: Option<ReadSelectionEvidence>,
    phase: Phase,
    pending_id: Option<RequestId>,
    accumulator: Option<MspV1ResponseAccumulator>,
    outcome: Option<FinalOutcome>,
    trace_events: VecDeque<RustTraceEvent>,
}

impl WasmReadonlySerialDiscovery {
    fn create() -> Result<Self, BridgeError> {
        Self::create_in_state(SessionState::Identifying)
    }

    fn create_in_state(state: SessionState) -> Result<Self, BridgeError> {
        Ok(Self {
            coordinator: IoCoordinator::new(),
            identification: ReadonlyIdentification::new(state)?,
            snapshot_read: None,
            pending_snapshot_identity: None,
            pending_snapshot_selection: None,
            phase: Phase::Ready,
            pending_id: None,
            accumulator: None,
            outcome: None,
            trace_events: VecDeque::with_capacity(TRACE_EVENT_LIMIT),
        })
    }

    fn push_trace(&mut self, event: RustTraceEvent) {
        if self.trace_events.len() == TRACE_EVENT_LIMIT {
            self.trace_events.pop_front();
        }
        self.trace_events.push_back(event);
    }

    fn push_directive_trace(
        &mut self,
        stage: ReadonlyStage,
        command: CommandId,
        byte_count: usize,
    ) -> Result<(), BridgeError> {
        let phase = stage.label();
        let byte_count = u32::try_from(byte_count).map_err(|_| BridgeError::Boundary)?;
        self.push_trace(RustTraceEvent {
            layer: "RUST",
            phase,
            event: "DIRECTIVE",
            stage,
            command,
            byte_count: Some(byte_count),
            direction: Some(Direction::Request),
            failure_class: None,
            failure_reason: None,
            origin: None,
        });
        Ok(())
    }

    fn push_frame_trace(
        &mut self,
        event: &'static str,
        stage: ReadonlyStage,
        command: CommandId,
        direction: Option<Direction>,
        failure_class: Option<&'static str>,
        failure_reason: Option<IdentityFailureReason>,
    ) {
        self.push_trace(RustTraceEvent {
            layer: "MSP",
            phase: "MSP_FRAME",
            event,
            stage,
            command,
            byte_count: None,
            direction,
            failure_class,
            failure_reason,
            origin: failure_class.map(|_| "MSP_FRAME"),
        });
    }

    fn push_identity_trace(
        &mut self,
        event: &'static str,
        stage: ReadonlyStage,
        command: CommandId,
        failure_reason: Option<IdentityFailureReason>,
    ) {
        self.push_trace(RustTraceEvent {
            layer: "RUST",
            phase: "IDENTITY_STAGE",
            event,
            stage,
            command,
            byte_count: None,
            direction: None,
            failure_class: failure_reason.map(|_| "ProtocolIdentityFailure"),
            failure_reason,
            origin: failure_reason.map(|_| "IDENTITY_STAGE"),
        });
    }

    fn push_snapshot_trace(
        &mut self,
        event: &'static str,
        failure_reason: Option<IdentityFailureReason>,
    ) {
        self.push_trace(RustTraceEvent {
            layer: "RUST",
            phase: "SNAPSHOT_STAGE",
            event,
            stage: ReadonlyStage::BeeperConfig,
            command: CommandId::BeeperConfig,
            byte_count: None,
            direction: None,
            failure_class: failure_reason.map(|_| "ProtocolSnapshotFailure"),
            failure_reason,
            origin: failure_reason.map(|_| "SNAPSHOT_STAGE"),
        });
    }

    fn begin_effect(
        &mut self,
        effect: TransportEffect,
        expected_command: Option<CommandId>,
        next_phase: Phase,
    ) -> Result<WasmReadonlySerialDirective, BridgeError> {
        let effect = self.coordinator.begin_transport(effect)?;
        self.pending_id = Some(effect.request_id());
        self.phase = next_phase;
        directive_from(effect, expected_command)
    }

    fn begin_open(&mut self) -> Result<WasmReadonlySerialDirective, BridgeError> {
        if self.phase != Phase::Ready {
            return Err(BridgeError::InvalidState);
        }
        self.begin_effect(
            TransportEffect::OpenSelectedReadOnlyPort,
            None,
            Phase::Opening,
        )
    }

    fn verify_pending(&self, request_id: &str, phase: Phase) -> Result<RequestId, BridgeError> {
        if self.phase != phase {
            return Err(BridgeError::InvalidState);
        }
        let request_id = parse_request_id(request_id)?;
        if self.pending_id != Some(request_id) {
            return Err(BridgeError::Boundary);
        }
        Ok(request_id)
    }

    fn next_exchange(&mut self) -> Result<WasmReadonlySerialDirective, BridgeError> {
        let stage = ReadonlyStage::from_identification(self.identification.stage())?;
        let request: IdentificationRequest = self.identification.next_request()?;
        let command = request.command();
        let packet = OutboundPacket::read_only(request.bytes().to_vec())?;
        self.accumulator = Some(MspV1ResponseAccumulator::new(command));
        let directive = self.begin_effect(
            TransportEffect::Exchange(packet),
            Some(command),
            Phase::Exchanging,
        )?;
        self.push_directive_trace(stage, command, directive.bytes.len())?;
        Ok(directive)
    }

    fn begin_snapshot_exchange(
        &mut self,
        identity: DeviceIdentity,
        selection: ReadSelectionEvidence,
    ) -> Result<WasmReadonlySerialDirective, BridgeError> {
        if self.snapshot_read.is_some()
            || self.pending_snapshot_identity.is_some()
            || self.pending_snapshot_selection.is_some()
        {
            return Err(BridgeError::InvalidState);
        }
        let mut snapshot_read =
            ReadonlyBeeperSnapshotRead::new(SessionState::SnapshotRead)?;
        let request = snapshot_read.next_request()?;
        let command = request.command();
        if command != CommandId::BeeperConfig {
            return Err(BridgeError::InvalidState);
        }
        let packet = OutboundPacket::read_only(request.bytes().to_vec())?;
        self.accumulator = Some(MspV1ResponseAccumulator::new(command));
        self.snapshot_read = Some(snapshot_read);
        self.pending_snapshot_identity = Some(identity);
        self.pending_snapshot_selection = Some(selection);
        let directive = self.begin_effect(
            TransportEffect::Exchange(packet),
            Some(command),
            Phase::Exchanging,
        )?;
        self.push_directive_trace(
            ReadonlyStage::BeeperConfig,
            command,
            directive.bytes.len(),
        )?;
        Ok(directive)
    }

    fn start_close(&mut self) -> Result<WasmReadonlySerialDirective, BridgeError> {
        self.accumulator = None;
        self.begin_effect(TransportEffect::Close, None, Phase::Closing)
    }

    fn accept_open_ok(
        &mut self,
        request_id: &str,
    ) -> Result<WasmReadonlySerialDirective, BridgeError> {
        let request_id = self.verify_pending(request_id, Phase::Opening)?;
        self.coordinator.accept(IoResponse::Transport {
            request_id,
            result: TransportResult::Open(Ok(())),
        })?;
        self.pending_id = None;
        self.next_exchange()
    }

    fn accept_open_err(&mut self, request_id: &str, failure: &str) -> Result<(), BridgeError> {
        let request_id = self.verify_pending(request_id, Phase::Opening)?;
        let failure = parse_failure(failure)?;
        self.coordinator.accept(IoResponse::Transport {
            request_id,
            result: TransportResult::Open(Err(failure)),
        })?;
        self.pending_id = None;
        self.outcome = Some(FinalOutcome::Failed {
            class: failure_label(failure),
            diagnostic: None,
        });
        self.phase = Phase::Complete;
        Ok(())
    }

    fn fail_exchange(
        &mut self,
        request_id: RequestId,
        label: &'static str,
        diagnostic: Option<ReadonlyFailureDiagnostic>,
    ) -> Result<WasmReadonlySerialDirective, BridgeError> {
        self.coordinator.cancel_transport(request_id)?;
        self.pending_id = None;
        self.snapshot_read = None;
        self.pending_snapshot_identity = None;
        self.pending_snapshot_selection = None;
        self.outcome = Some(FinalOutcome::Failed {
            class: label,
            diagnostic,
        });
        self.start_close()
    }

    fn current_stage(&self) -> Result<ReadonlyStage, BridgeError> {
        if self.snapshot_read.is_some() {
            Ok(ReadonlyStage::BeeperConfig)
        } else {
            ReadonlyStage::from_identification(self.identification.stage())
        }
    }

    fn accept_snapshot_frame(
        &mut self,
        frame: &ade_protocol_msp::Frame,
    ) -> Result<Option<WasmReadonlySerialDirective>, BridgeError> {
        let result = self
            .snapshot_read
            .as_mut()
            .ok_or(BridgeError::InvalidState)?
            .accept_response(frame);
        match result {
            Ok(snapshot) => {
                self.push_snapshot_trace("SNAPSHOT_STAGE_OK", None);
                if !self
                    .snapshot_read
                    .as_ref()
                    .is_some_and(ReadonlyBeeperSnapshotRead::is_complete)
                {
                    return Err(BridgeError::InvalidState);
                }
                self.snapshot_read = None;
                let identity = self
                    .pending_snapshot_identity
                    .take()
                    .ok_or(BridgeError::InvalidState)?;
                let selection = self
                    .pending_snapshot_selection
                    .take()
                    .ok_or(BridgeError::InvalidState)?;
                self.outcome = Some(FinalOutcome::InScope {
                    identity,
                    selection,
                    snapshot,
                });
                self.start_close().map(Some)
            }
            Err(error) => {
                let diagnostic = ReadonlyFailureDiagnostic::from_exec(
                    ReadonlyStage::BeeperConfig,
                    &error,
                );
                self.push_snapshot_trace("SNAPSHOT_STAGE_FAILED", Some(diagnostic.reason));
                self.snapshot_read = None;
                self.pending_snapshot_identity = None;
                self.pending_snapshot_selection = None;
                self.outcome = Some(FinalOutcome::Failed {
                    class: "ProtocolSnapshotFailure",
                    diagnostic: Some(diagnostic),
                });
                self.start_close().map(Some)
            }
        }
    }

    fn accept_identification_frame(
        &mut self,
        stage: ReadonlyStage,
        command: CommandId,
        frame: &ade_protocol_msp::Frame,
    ) -> Result<Option<WasmReadonlySerialDirective>, BridgeError> {
        match self.identification.accept_response(frame) {
            Ok(IdentificationProgress::Complete(identity)) => {
                self.push_identity_trace("IDENTITY_STAGE_OK", stage, command, None);
                let selection = legacy_selection_evidence(&identity)?;
                match check_scope(&identity) {
                    ScopeStatus::InScope => {
                        if !api146_snapshot_is_reviewed(selection) {
                            return Err(BridgeError::InvalidState);
                        }
                        self.begin_snapshot_exchange(identity, selection).map(Some)
                    }
                    ScopeStatus::Mismatch { field } => {
                        self.outcome = Some(FinalOutcome::ScopeMismatch {
                            identity,
                            field,
                            selection,
                        });
                        self.start_close().map(Some)
                    }
                    ScopeStatus::NotChecked => Err(BridgeError::InvalidState),
                }
            }
            Ok(IdentificationProgress::ReadOnlyComplete(identity)) => {
                self.push_identity_trace("IDENTITY_STAGE_OK", stage, command, None);
                let selection = readonly_selection_evidence(&identity)?;
                self.outcome = Some(FinalOutcome::ReadOnlyComplete {
                    identity,
                    selection,
                });
                self.start_close().map(Some)
            }
            Ok(IdentificationProgress::Pending) => {
                self.push_identity_trace("IDENTITY_STAGE_OK", stage, command, None);
                self.next_exchange().map(Some)
            }
            Ok(IdentificationProgress::ApiScopeMismatch { api, field }) => {
                self.push_identity_trace("IDENTITY_STAGE_OK", stage, command, None);
                self.outcome = Some(FinalOutcome::ApiScopeMismatch { api, field });
                self.start_close().map(Some)
            }
            Ok(IdentificationProgress::ReadProfileMismatch { api, field }) => {
                self.push_identity_trace("IDENTITY_STAGE_OK", stage, command, None);
                self.outcome = Some(FinalOutcome::ReadProfileMismatch { api, field });
                self.start_close().map(Some)
            }
            Err(error) => {
                let diagnostic = ReadonlyFailureDiagnostic::from_exec(stage, &error);
                self.push_identity_trace(
                    "IDENTITY_STAGE_FAILED",
                    stage,
                    command,
                    Some(diagnostic.reason),
                );
                self.outcome = Some(FinalOutcome::Failed {
                    class: "ProtocolIdentityFailure",
                    diagnostic: Some(diagnostic),
                });
                self.start_close().map(Some)
            }
        }
    }

    fn accept_chunk(
        &mut self,
        request_id: &str,
        chunk: &[u8],
    ) -> Result<Option<WasmReadonlySerialDirective>, BridgeError> {
        let request_id = self.verify_pending(request_id, Phase::Exchanging)?;
        let stage = self.current_stage()?;
        let command = stage.command();
        let progress = match self
            .accumulator
            .as_mut()
            .ok_or(BridgeError::InvalidState)?
            .push(chunk)
        {
            Ok(progress) => progress,
            Err(error) => {
                let diagnostic = ReadonlyFailureDiagnostic::from_msp(stage, &error);
                self.push_frame_trace(
                    "FRAME_REJECTED",
                    stage,
                    command,
                    None,
                    Some("MalformedResponse"),
                    Some(diagnostic.reason),
                );
                return self
                    .fail_exchange(request_id, "MalformedResponse", Some(diagnostic))
                    .map(Some);
            }
        };
        let ResponseProgress::Complete(frame) = progress else {
            return Ok(None);
        };

        self.push_frame_trace(
            "FRAME_ACCEPTED",
            stage,
            command,
            Some(frame.direction),
            None,
            None,
        );

        self.coordinator.accept(IoResponse::Transport {
            request_id,
            result: TransportResult::Exchange(Ok(Vec::new())),
        })?;
        self.pending_id = None;
        self.accumulator = None;
        if stage == ReadonlyStage::BeeperConfig {
            self.accept_snapshot_frame(&frame)
        } else {
            self.accept_identification_frame(stage, command, &frame)
        }
    }

    fn accept_exchange_err(
        &mut self,
        request_id: &str,
        failure: &str,
    ) -> Result<WasmReadonlySerialDirective, BridgeError> {
        let request_id = self.verify_pending(request_id, Phase::Exchanging)?;
        let failure = parse_failure(failure)?;
        self.coordinator.accept(IoResponse::Transport {
            request_id,
            result: TransportResult::Exchange(Err(failure)),
        })?;
        self.pending_id = None;
        self.snapshot_read = None;
        self.pending_snapshot_identity = None;
        self.pending_snapshot_selection = None;
        self.outcome = Some(FinalOutcome::Failed {
            class: failure_label(failure),
            diagnostic: None,
        });
        self.start_close()
    }

    fn accept_close(&mut self, request_id: &str, failure: Option<&str>) -> Result<(), BridgeError> {
        let request_id = self.verify_pending(request_id, Phase::Closing)?;
        let result = match failure {
            Some(value) => TransportResult::Close(Err(parse_failure(value)?)),
            None => TransportResult::Close(Ok(())),
        };
        self.coordinator
            .accept(IoResponse::Transport { request_id, result })?;
        self.pending_id = None;
        if failure.is_some() {
            self.outcome = Some(FinalOutcome::Failed {
                class: "CloseFailure",
                diagnostic: None,
            });
        }
        self.phase = Phase::Complete;
        Ok(())
    }

    fn identity(&self) -> Option<&DeviceIdentity> {
        match self.outcome.as_ref()? {
            FinalOutcome::InScope { identity, .. }
            | FinalOutcome::ScopeMismatch { identity, .. } => Some(identity),
            FinalOutcome::ApiScopeMismatch { .. }
            | FinalOutcome::ReadOnlyComplete { .. }
            | FinalOutcome::ReadProfileMismatch { .. }
            | FinalOutcome::Failed { .. } => None,
        }
    }

    fn read_only_identity(&self) -> Option<&ReadonlyProfileIdentity> {
        match self.outcome.as_ref()? {
            FinalOutcome::ReadOnlyComplete { identity, .. } => Some(identity),
            _ => None,
        }
    }

    fn selection_evidence(&self) -> Option<ReadSelectionEvidence> {
        match self.outcome.as_ref()? {
            FinalOutcome::InScope { selection, .. }
            | FinalOutcome::ScopeMismatch { selection, .. }
            | FinalOutcome::ReadOnlyComplete { selection, .. } => Some(*selection),
            _ => None,
        }
    }

    fn snapshot(&self) -> Option<&BeeperConfigSnapshot> {
        match self.outcome.as_ref()? {
            FinalOutcome::InScope { snapshot, .. } => Some(snapshot),
            _ => None,
        }
    }
}

#[wasm_bindgen]
impl WasmReadonlySerialDiscovery {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmReadonlySerialDiscovery, JsError> {
        Self::create().map_err(js_error)
    }

    #[wasm_bindgen(js_name = begin)]
    pub fn begin(&mut self) -> Result<WasmReadonlySerialDirective, JsError> {
        self.begin_open().map_err(js_error)
    }

    #[wasm_bindgen(js_name = acceptOpenSuccess)]
    pub fn accept_open_success(
        &mut self,
        request_id: &str,
    ) -> Result<WasmReadonlySerialDirective, JsError> {
        self.accept_open_ok(request_id).map_err(js_error)
    }

    #[wasm_bindgen(js_name = acceptOpenFailure)]
    pub fn accept_open_failure(&mut self, request_id: &str, failure: &str) -> Result<(), JsError> {
        self.accept_open_err(request_id, failure).map_err(js_error)
    }

    #[wasm_bindgen(js_name = acceptReadChunk)]
    pub fn accept_read_chunk(
        &mut self,
        request_id: &str,
        chunk: Vec<u8>,
    ) -> Result<Option<WasmReadonlySerialDirective>, JsError> {
        self.accept_chunk(request_id, &chunk).map_err(js_error)
    }

    #[wasm_bindgen(js_name = acceptExchangeFailure)]
    pub fn accept_exchange_failure(
        &mut self,
        request_id: &str,
        failure: &str,
    ) -> Result<WasmReadonlySerialDirective, JsError> {
        self.accept_exchange_err(request_id, failure)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = acceptCloseSuccess)]
    pub fn accept_close_success(&mut self, request_id: &str) -> Result<(), JsError> {
        self.accept_close(request_id, None).map_err(js_error)
    }

    #[wasm_bindgen(js_name = acceptCloseFailure)]
    pub fn accept_close_failure(&mut self, request_id: &str, failure: &str) -> Result<(), JsError> {
        self.accept_close(request_id, Some(failure))
            .map_err(js_error)
    }

    /// Remove the oldest privacy-bounded protocol event, if one is waiting.
    ///
    /// The queue is fixed at 32 entries and carries only stable enum-like labels and a
    /// request byte count. The browser host drains it into its separate bounded RAM trace.
    #[wasm_bindgen(js_name = takeTraceEvent)]
    pub fn take_trace_event(&mut self) -> Option<WasmReadonlyTraceEvent> {
        self.trace_events
            .pop_front()
            .map(|event| WasmReadonlyTraceEvent { event })
    }

    #[wasm_bindgen(getter, js_name = outcomeKind)]
    pub fn outcome_kind(&self) -> String {
        match &self.outcome {
            Some(FinalOutcome::InScope { .. }) if self.phase == Phase::Complete => "in-scope",
            Some(FinalOutcome::ScopeMismatch { .. }) if self.phase == Phase::Complete => {
                "scope-mismatch"
            }
            Some(FinalOutcome::ApiScopeMismatch { .. }) if self.phase == Phase::Complete => {
                "api-unsupported"
            }
            Some(FinalOutcome::ReadOnlyComplete { .. }) if self.phase == Phase::Complete => {
                "read-only-complete"
            }
            Some(FinalOutcome::ReadProfileMismatch { .. }) if self.phase == Phase::Complete => {
                "read-profile-unsupported"
            }
            Some(FinalOutcome::Failed { .. }) if self.phase == Phase::Complete => "failed",
            _ => "pending",
        }
        .to_owned()
    }

    #[wasm_bindgen(getter, js_name = failureClass)]
    pub fn failure_class(&self) -> Option<String> {
        match &self.outcome {
            Some(FinalOutcome::Failed { class, .. }) => Some((*class).to_owned()),
            _ => None,
        }
    }

    /// The fixed identity stage at which a protocol failure occurred.
    ///
    /// This getter exposes only a fixed read-stage label and never command bytes or payload.
    #[wasm_bindgen(getter, js_name = failureStage)]
    pub fn failure_stage(&self) -> Option<String> {
        match &self.outcome {
            Some(FinalOutcome::Failed {
                diagnostic: Some(diagnostic),
                ..
            }) => Some(diagnostic.stage_label().to_owned()),
            _ => None,
        }
    }

    /// The allowlisted structural reason for a protocol failure.
    ///
    /// Numeric counts and all raw response or identity data are deliberately discarded.
    #[wasm_bindgen(getter, js_name = failureReason)]
    pub fn failure_reason(&self) -> Option<String> {
        match &self.outcome {
            Some(FinalOutcome::Failed {
                diagnostic: Some(diagnostic),
                ..
            }) => Some(diagnostic.reason.label().to_owned()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter, js_name = scopeMismatchField)]
    pub fn scope_mismatch_field(&self) -> Option<String> {
        match &self.outcome {
            Some(
                FinalOutcome::ScopeMismatch { field, .. }
                | FinalOutcome::ApiScopeMismatch { field, .. }
                | FinalOutcome::ReadProfileMismatch { field, .. },
            ) => Some((*field).to_owned()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter, js_name = apiVersion)]
    pub fn api_version(&self) -> Option<String> {
        match &self.outcome {
            Some(
                FinalOutcome::ApiScopeMismatch { api, .. }
                | FinalOutcome::ReadProfileMismatch { api, .. },
            ) => Some(format!("{}.{}", api.api_major, api.api_minor)),
            Some(FinalOutcome::ReadOnlyComplete { identity, .. }) => Some(format!(
                "{}.{}",
                identity.api.api_major, identity.api.api_minor
            )),
            _ => self
                .identity()
                .map(|identity| format!("{}.{}", identity.api.api_major, identity.api.api_minor)),
        }
    }

    #[wasm_bindgen(getter, js_name = fcVariant)]
    pub fn fc_variant(&self) -> Option<String> {
        self.read_only_identity()
            .map(|identity| identity.variant.identifier_string())
            .or_else(|| {
                self.identity()
                    .map(|identity| identity.variant.identifier_string())
            })
    }

    #[wasm_bindgen(getter, js_name = fcVersion)]
    pub fn fc_version(&self) -> Option<String> {
        self.read_only_identity()
            .map(ReadonlyProfileIdentity::fc_version_string)
            .or_else(|| {
                self.identity().map(|identity| {
                    format!(
                        "{}.{}.{}",
                        identity.version.major, identity.version.minor, identity.version.patch
                    )
                })
            })
    }

    #[wasm_bindgen(getter, js_name = targetName)]
    pub fn target_name(&self) -> Option<String> {
        self.read_only_identity()
            .map(|identity| identity.target_name.clone())
            .or_else(|| self.identity().map(|identity| identity.target_name.clone()))
    }

    /// Stable identifier of the reviewed identity layout selected by Rust.
    #[wasm_bindgen(getter, js_name = readProfileId)]
    pub fn read_profile_id(&self) -> Option<String> {
        self.selection_evidence()
            .map(|selection| read_profile_label(selection.profile_id).to_owned())
    }

    /// Permanent write boundary carried by the selected read profile.
    #[wasm_bindgen(getter, js_name = readProfileWriteAuthority)]
    pub fn read_profile_write_authority(&self) -> Option<String> {
        self.selection_evidence().map(|selection| {
            read_profile_write_authority_label(selection.write_authority).to_owned()
        })
    }

    /// Bounded outcome from the repository-reviewed capability resolver.
    #[wasm_bindgen(getter, js_name = capabilityStatus)]
    pub fn capability_status(&self) -> Option<String> {
        self.selection_evidence()
            .map(|selection| capability_status_label(selection.capability).to_owned())
    }

    /// Stable pack id only when exactly one validated review-only descriptor matched.
    #[wasm_bindgen(getter, js_name = capabilityPackId)]
    pub fn capability_pack_id(&self) -> Option<String> {
        match self.selection_evidence()?.capability {
            CapabilitySelectionEvidence::Match { pack_id, .. } => Some(pack_id.to_owned()),
            _ => None,
        }
    }

    /// Trust boundary only for an exact review-only capability match.
    #[wasm_bindgen(getter, js_name = capabilityTrust)]
    pub fn capability_trust(&self) -> Option<String> {
        match self.selection_evidence()?.capability {
            CapabilitySelectionEvidence::Match { trust, .. } => {
                Some(capability_trust_label(trust).to_owned())
            }
            _ => None,
        }
    }

    /// Write policy only for an exact review-only capability match.
    #[wasm_bindgen(getter, js_name = capabilityWritePolicy)]
    pub fn capability_write_policy(&self) -> Option<String> {
        match self.selection_evidence()?.capability {
            CapabilitySelectionEvidence::Match { write_policy, .. } => {
                Some(capability_write_policy_label(write_policy).to_owned())
            }
            _ => None,
        }
    }

    /// Stable marker proving the exact typed nine-byte snapshot completed in Rust.
    #[wasm_bindgen(getter, js_name = snapshotStatus)]
    pub fn snapshot_status(&self) -> Option<String> {
        self.snapshot().map(|_| "beeper-config-complete".to_owned())
    }

    /// Typed beeper disable flags from the reviewed snapshot layout.
    #[wasm_bindgen(getter, js_name = beeperOffFlags)]
    pub fn beeper_off_flags(&self) -> Option<u32> {
        self.snapshot().map(|snapshot| snapshot.beeper_off_flags)
    }

    /// Typed DShot beacon tone from the reviewed snapshot layout.
    #[wasm_bindgen(getter, js_name = dshotBeaconTone)]
    pub fn dshot_beacon_tone(&self) -> Option<u8> {
        self.snapshot().map(|snapshot| snapshot.dshot_beacon_tone)
    }

    /// Typed DShot beacon disable flags from the reviewed snapshot layout.
    #[wasm_bindgen(getter, js_name = dshotBeaconOffFlags)]
    pub fn dshot_beacon_off_flags(&self) -> Option<u32> {
        self.snapshot()
            .map(|snapshot| snapshot.dshot_beacon_off_flags)
    }

    /// Derived state of the one bit targeted by the simulation-only M1 slice.
    #[wasm_bindgen(getter, js_name = systemInitDisabled)]
    pub fn system_init_disabled(&self) -> Option<bool> {
        self.snapshot()
            .map(BeeperConfigSnapshot::system_init_disabled)
    }

    #[wasm_bindgen(getter, js_name = hardwareObserved)]
    pub fn hardware_observed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_protocol_msp::{Direction, encode_frame};
    use ade_safety::{ExecutionTarget, RecoveryClass, authorize_write};

    fn transport_effect(effect: TransportEffect) -> IoEffect {
        IoEffect::Transport {
            request_id: RequestId::new(9),
            effect,
        }
    }

    fn board_payload(target: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"F405");
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&[0, 0]);
        for value in [target, "SpeedyBee F405 V4", "SPB"] {
            payload.push(u8::try_from(value.len()).unwrap());
            payload.extend_from_slice(value.as_bytes());
        }
        payload.extend_from_slice(&[0; ade_protocol_msp::SIGNATURE_LENGTH]);
        payload.extend_from_slice(&[0, 0]);
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&[0, 0]);
        payload
    }

    fn valid_board_payload() -> Vec<u8> {
        board_payload("SPEEDYBEEF405V4")
    }

    fn valid_snapshot_payload() -> Vec<u8> {
        BeeperConfigSnapshot {
            beeper_off_flags: ade_protocol_msp::SYSTEM_INIT_OFF_MASK,
            dshot_beacon_tone: 3,
            dshot_beacon_off_flags: 0x0102_0304,
        }
        .to_reply_payload()
    }

    fn feed_reply(
        bridge: &mut WasmReadonlySerialDiscovery,
        directive: &WasmReadonlySerialDirective,
        direction: Direction,
        command: CommandId,
        payload: &[u8],
    ) -> WasmReadonlySerialDirective {
        let frame = encode_frame(direction, command, payload).unwrap();
        bridge
            .accept_chunk(&directive.request_id, &frame)
            .unwrap()
            .expect("a complete reply must produce the next directive")
    }

    fn bridge_at(
        stage: IdentificationStage,
    ) -> (WasmReadonlySerialDiscovery, WasmReadonlySerialDirective) {
        let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
        let open = bridge.begin_open().unwrap();
        let mut exchange = bridge.accept_open_ok(&open.request_id).unwrap();
        let prefix: &[(CommandId, &[u8])] = &[
            (CommandId::ApiVersion, &[0, 1, 46]),
            (CommandId::FcVariant, b"BTFL"),
            (CommandId::FcVersion, &[4, 5, 5]),
        ];
        let count = match stage {
            IdentificationStage::ApiVersion => 0,
            IdentificationStage::FcVariant => 1,
            IdentificationStage::FcVersion => 2,
            IdentificationStage::BoardInfo => 3,
            IdentificationStage::Complete => panic!("complete has no pending read"),
        };
        for &(command, payload) in &prefix[..count] {
            exchange = feed_reply(&mut bridge, &exchange, Direction::Reply, command, payload);
        }
        assert_eq!(bridge.identification.stage(), stage);
        (bridge, exchange)
    }

    fn stage_case(stage: IdentificationStage) -> (CommandId, Vec<u8>) {
        match stage {
            IdentificationStage::ApiVersion => (CommandId::ApiVersion, vec![0, 1, 46]),
            IdentificationStage::FcVariant => (CommandId::FcVariant, b"BTFL".to_vec()),
            IdentificationStage::FcVersion => (CommandId::FcVersion, vec![4, 5, 5]),
            IdentificationStage::BoardInfo => (CommandId::BoardInfo, valid_board_payload()),
            IdentificationStage::Complete => panic!("complete has no reply case"),
        }
    }

    fn assert_failure(
        mut bridge: WasmReadonlySerialDiscovery,
        exchange: WasmReadonlySerialDirective,
        direction: Direction,
        command: CommandId,
        payload: &[u8],
        expected: (&str, &str, &str),
    ) {
        let (class, stage, reason) = expected;
        let close = feed_reply(&mut bridge, &exchange, direction, command, payload);
        assert_eq!(close.kind, "close");
        assert_eq!(bridge.failure_class().as_deref(), Some(class));
        assert_eq!(bridge.failure_stage().as_deref(), Some(stage));
        assert_eq!(bridge.failure_reason().as_deref(), Some(reason));
        assert!(bridge.identity().is_none());
        bridge.accept_close(&close.request_id, None).unwrap();
        assert_eq!(bridge.outcome_kind(), "failed");
        assert!(!bridge.hardware_observed());
    }

    #[test]
    fn only_the_four_empty_identification_requests_cross_the_facade() {
        for command in [
            CommandId::ApiVersion,
            CommandId::FcVariant,
            CommandId::FcVersion,
            CommandId::BoardInfo,
        ] {
            let bytes = ade_protocol_msp::encode_frame(Direction::Request, command, &[]).unwrap();
            let packet = OutboundPacket::read_only(bytes.clone()).unwrap();
            let directive = directive_from(
                transport_effect(TransportEffect::Exchange(packet)),
                Some(command),
            )
            .unwrap();
            assert_eq!(directive.kind, "exchange-identification-read");
            assert_eq!(directive.bytes, bytes);
        }
    }

    #[test]
    fn the_only_additional_read_directive_is_the_empty_beeper_snapshot_request() {
        let bytes =
            ade_protocol_msp::encode_frame(Direction::Request, CommandId::BeeperConfig, &[])
                .unwrap();
        let packet = OutboundPacket::read_only(bytes.clone()).unwrap();
        let directive = directive_from(
            transport_effect(TransportEffect::Exchange(packet)),
            Some(CommandId::BeeperConfig),
        )
        .unwrap();
        assert_eq!(directive.kind, "exchange-snapshot-read");
        assert_eq!(directive.bytes, bytes);

        let with_payload = ade_protocol_msp::encode_frame(
            Direction::Request,
            CommandId::BeeperConfig,
            &[0],
        )
        .unwrap();
        let packet = OutboundPacket::read_only(with_payload).unwrap();
        assert!(
            directive_from(
                transport_effect(TransportEffect::Exchange(packet)),
                Some(CommandId::BeeperConfig),
            )
            .is_err()
        );
    }

    #[test]
    fn unsupported_api_versions_close_after_only_the_api_read() {
        for (payload, expected_api, expected_field) in [
            ([0, 1, 45], "1.45", "msp_api_version"),
            ([0, 1, 48], "1.48", "msp_api_version"),
            ([0, 2, 46], "2.46", "msp_api_version"),
            ([1, 1, 46], "1.46", "protocol_version"),
        ] {
            let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
            let open = bridge.begin_open().unwrap();
            let exchange = bridge.accept_open_ok(&open.request_id).unwrap();
            let request = decode_frame(&exchange.bytes).unwrap();
            assert_eq!(request.known_command(), Some(CommandId::ApiVersion));
            assert_eq!(request.payload_len(), 0);

            let close = feed_reply(
                &mut bridge,
                &exchange,
                Direction::Reply,
                CommandId::ApiVersion,
                &payload,
            );
            assert_eq!(close.kind, "close");
            assert!(close.bytes.is_empty());
            assert!(bridge.identification.is_complete());
            assert!(bridge.identity().is_none());
            assert!(bridge.failure_class().is_none());
            assert!(bridge.failure_stage().is_none());
            assert!(bridge.failure_reason().is_none());
            assert_eq!(bridge.api_version().as_deref(), Some(expected_api));
            assert_eq!(
                bridge.scope_mismatch_field().as_deref(),
                Some(expected_field),
            );

            bridge.accept_close(&close.request_id, None).unwrap();
            assert_eq!(bridge.outcome_kind(), "api-unsupported");
            assert!(!bridge.hardware_observed());
            let events: Vec<_> = bridge.trace_events.drain(..).collect();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.event == "DIRECTIVE")
                    .count(),
                1,
            );
            assert!(events.iter().all(|event| {
                event.stage == ReadonlyStage::ApiVersion
                    && event.command == CommandId::ApiVersion
            }));
        }
    }

    #[test]
    fn reviewed_api_147_betaflight_runs_all_four_reads_and_returns_read_only_identity() {
        let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
        let open = bridge.begin_open().unwrap();
        let api = bridge.accept_open_ok(&open.request_id).unwrap();
        let variant = feed_reply(
            &mut bridge,
            &api,
            Direction::Reply,
            CommandId::ApiVersion,
            &[0, 1, 47],
        );
        let version = feed_reply(
            &mut bridge,
            &variant,
            Direction::Reply,
            CommandId::FcVariant,
            b"BTFL",
        );
        let mut version_payload = vec![25, 12, 1, 9];
        version_payload.extend_from_slice(b"2025.12.1");
        let board = feed_reply(
            &mut bridge,
            &version,
            Direction::Reply,
            CommandId::FcVersion,
            &version_payload,
        );
        let close = feed_reply(
            &mut bridge,
            &board,
            Direction::Reply,
            CommandId::BoardInfo,
            &valid_board_payload(),
        );
        assert_eq!(close.kind, "close");
        assert_eq!(bridge.api_version().as_deref(), Some("1.47"));
        assert_eq!(bridge.fc_variant().as_deref(), Some("BTFL"));
        assert_eq!(bridge.fc_version().as_deref(), Some("2025.12.1"));
        assert_eq!(bridge.target_name().as_deref(), Some("SPEEDYBEEF405V4"));
        assert!(bridge.scope_mismatch_field().is_none());
        assert_eq!(
            bridge.read_profile_id().as_deref(),
            Some("api-1.47-calendar-extended")
        );
        assert_eq!(
            bridge.read_profile_write_authority().as_deref(),
            Some("never-authorizes-writes")
        );
        assert_eq!(
            bridge.capability_status().as_deref(),
            Some("review-only-match")
        );
        assert_eq!(
            bridge.capability_pack_id().as_deref(),
            Some("bf-2025.12.1-api1.47-speedybeef405v4-review")
        );
        assert_eq!(
            bridge.capability_trust().as_deref(),
            Some("review-only-embedded")
        );
        assert_eq!(
            bridge.capability_write_policy().as_deref(),
            Some("writes-blocked")
        );
        assert!(bridge.snapshot_status().is_none());
        assert!(bridge.beeper_off_flags().is_none());
        assert!(bridge.system_init_disabled().is_none());
        bridge.accept_close(&close.request_id, None).unwrap();
        assert_eq!(bridge.outcome_kind(), "read-only-complete");
        assert!(!bridge.hardware_observed());
        assert_eq!(
            bridge
                .trace_events
                .iter()
                .filter(|event| event.event == "DIRECTIVE")
                .count(),
            4,
        );
    }

    #[test]
    fn api_147_version_string_drift_completes_read_only_without_a_pack_match() {
        let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
        let open = bridge.begin_open().unwrap();
        let api = bridge.accept_open_ok(&open.request_id).unwrap();
        let variant = feed_reply(
            &mut bridge,
            &api,
            Direction::Reply,
            CommandId::ApiVersion,
            &[0, 1, 47],
        );
        let version = feed_reply(
            &mut bridge,
            &variant,
            Direction::Reply,
            CommandId::FcVariant,
            b"BTFL",
        );
        let mut version_payload = vec![25, 12, 1, 16];
        version_payload.extend_from_slice(b"2025.12.1-custom");
        let board = feed_reply(
            &mut bridge,
            &version,
            Direction::Reply,
            CommandId::FcVersion,
            &version_payload,
        );
        let close = feed_reply(
            &mut bridge,
            &board,
            Direction::Reply,
            CommandId::BoardInfo,
            &valid_board_payload(),
        );
        assert_eq!(
            bridge.capability_status().as_deref(),
            Some("no-reviewed-match")
        );
        assert!(bridge.capability_pack_id().is_none());
        assert!(bridge.capability_trust().is_none());
        assert!(bridge.capability_write_policy().is_none());
        bridge.accept_close(&close.request_id, None).unwrap();
        assert_eq!(bridge.outcome_kind(), "read-only-complete");
        assert!(!bridge.hardware_observed());
    }

    #[test]
    fn reviewed_api_147_wrong_variant_stops_before_fc_version() {
        let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
        let open = bridge.begin_open().unwrap();
        let api = bridge.accept_open_ok(&open.request_id).unwrap();
        let variant = feed_reply(
            &mut bridge,
            &api,
            Direction::Reply,
            CommandId::ApiVersion,
            &[0, 1, 47],
        );
        let close = feed_reply(
            &mut bridge,
            &variant,
            Direction::Reply,
            CommandId::FcVariant,
            b"INAV",
        );
        assert_eq!(close.kind, "close");
        assert_eq!(bridge.api_version().as_deref(), Some("1.47"));
        assert_eq!(bridge.scope_mismatch_field().as_deref(), Some("fc_variant"));
        assert!(bridge.fc_variant().is_none());
        assert!(bridge.read_profile_id().is_none());
        assert!(bridge.capability_status().is_none());
        bridge.accept_close(&close.request_id, None).unwrap();
        assert_eq!(bridge.outcome_kind(), "read-profile-unsupported");
        assert!(!bridge.hardware_observed());
        assert_eq!(
            bridge
                .trace_events
                .iter()
                .filter(|event| event.event == "DIRECTIVE")
                .count(),
            2,
        );
    }

    #[test]
    fn refused_identification_contexts_cannot_create_a_web_serial_directive() {
        let all_states = [
            SessionState::Disconnected,
            SessionState::Connecting,
            SessionState::Identifying,
            SessionState::SnapshotRead,
            SessionState::Planning,
            SessionState::AwaitingApproval,
            SessionState::BackingUp,
            SessionState::ApplyingTransient,
            SessionState::TransientWritePendingReconcileOnResume,
            SessionState::Saving,
            SessionState::Rebooting,
            SessionState::Reconnecting,
            SessionState::Verifying,
            SessionState::Recovering,
            SessionState::CompletedVerified,
            SessionState::CompletedRestored,
            SessionState::StateUnknownRecoveryRequired,
        ];

        for state in all_states {
            match WasmReadonlySerialDiscovery::create_in_state(state) {
                Ok(mut discovery) => {
                    assert!(state.permits_identification(), "unexpected state {state:?}");
                    assert_eq!(
                        discovery.begin_open().unwrap().kind,
                        "open-selected-read-only-port",
                    );
                }
                Err(BridgeError::Execution) => {
                    assert!(!state.permits_identification(), "refused state {state:?}");
                }
                Err(error) => panic!("unexpected error for {state:?}: {error:?}"),
            }
        }
    }

    #[test]
    fn prohibited_read_write_unknown_malformed_approval_and_wrong_state_never_form_a_directive() {
        for command in [
            CommandId::BeeperConfig,
            CommandId::SetBeeperConfig,
            CommandId::EepromWrite,
            CommandId::Reboot,
        ] {
            let bytes = encode_frame(Direction::Request, command, &[]).unwrap();
            let effect = match OutboundPacket::read_only(bytes.clone()) {
                Ok(packet) => TransportEffect::Exchange(packet),
                Err(_) => {
                    let approval = authorize_write(
                        ExecutionTarget::Mock,
                        match command {
                            CommandId::SetBeeperConfig => WriteCommandClass::TransientConfig,
                            CommandId::EepromWrite => WriteCommandClass::PersistentConfig,
                            CommandId::Reboot => WriteCommandClass::Reboot,
                            _ => unreachable!(),
                        },
                        match command {
                            CommandId::SetBeeperConfig => {
                                RecoveryClass::TransientWritePendingReconcileOnResume
                            }
                            CommandId::EepromWrite => RecoveryClass::AutomaticRollbackSupported,
                            CommandId::Reboot => RecoveryClass::ManualRecoveryRequired,
                            _ => unreachable!(),
                        },
                    )
                    .unwrap();
                    TransportEffect::Exchange(OutboundPacket::approved(bytes, approval).unwrap())
                }
            };
            assert!(directive_from(transport_effect(effect), Some(CommandId::ApiVersion)).is_err());
        }

        let unknown = vec![b'$', b'M', b'<', 0, 99, 99];
        assert!(OutboundPacket::read_only(unknown).is_err());
        assert!(OutboundPacket::read_only(vec![1, 2, 3]).is_err());

        let allowed = OutboundPacket::read_only(
            encode_frame(Direction::Request, CommandId::ApiVersion, &[]).unwrap(),
        )
        .unwrap();
        assert!(
            directive_from(transport_effect(TransportEffect::Exchange(allowed)), None).is_err()
        );
    }

    #[test]
    fn stale_wrong_kind_and_duplicate_responses_leave_pending_state_honest() {
        let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
        let open = bridge.begin_open().unwrap();
        assert!(bridge.accept_open_ok("2").is_err());
        assert!(bridge.coordinator.has_pending_transport());
        assert!(
            bridge
                .coordinator
                .accept(IoResponse::Transport {
                    request_id: RequestId::new(1),
                    result: TransportResult::Close(Ok(())),
                })
                .is_err()
        );
        assert!(bridge.coordinator.has_pending_transport());
        let exchange = bridge.accept_open_ok(&open.request_id).unwrap();
        assert!(bridge.accept_open_ok(&open.request_id).is_err());
        assert!(bridge.coordinator.has_pending_transport());
        assert_ne!(exchange.request_id, open.request_id);
    }

    #[test]
    fn fixed_length_decode_failures_report_each_exact_identity_stage() {
        let (bridge, exchange) = bridge_at(IdentificationStage::ApiVersion);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::ApiVersion,
            &[0, 1],
            ("ProtocolIdentityFailure", "API_VERSION", "WrongLength"),
        );

        let (bridge, exchange) = bridge_at(IdentificationStage::FcVariant);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::FcVariant,
            b"BTF",
            ("ProtocolIdentityFailure", "FC_VARIANT", "WrongLength"),
        );

        let (bridge, exchange) = bridge_at(IdentificationStage::FcVersion);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::FcVersion,
            &[4, 5],
            ("ProtocolIdentityFailure", "FC_VERSION", "WrongLength"),
        );

        let (bridge, exchange) = bridge_at(IdentificationStage::FcVersion);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::FcVersion,
            &[
                25, 12, 5, 9, b'2', b'0', b'2', b'5', b'.', b'1', b'2', b'.', b'5',
            ],
            ("ProtocolIdentityFailure", "FC_VERSION", "WrongLength"),
        );
    }

    #[test]
    fn board_info_structural_failures_expose_categories_without_payload_data() {
        let (bridge, exchange) = bridge_at(IdentificationStage::BoardInfo);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::BoardInfo,
            &[],
            ("ProtocolIdentityFailure", "BOARD_INFO", "FieldOverrun"),
        );

        let mut trailing = valid_board_payload();
        trailing.push(0);
        let (bridge, exchange) = bridge_at(IdentificationStage::BoardInfo);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::BoardInfo,
            &trailing,
            ("ProtocolIdentityFailure", "BOARD_INFO", "TrailingPayload"),
        );

        let mut invalid_utf8 = valid_board_payload();
        invalid_utf8[9] = 0xff;
        let (bridge, exchange) = bridge_at(IdentificationStage::BoardInfo);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::BoardInfo,
            &invalid_utf8,
            ("ProtocolIdentityFailure", "BOARD_INFO", "InvalidUtf8"),
        );
    }

    #[test]
    fn correlation_and_error_replies_remain_typed_and_fail_closed() {
        let (bridge, exchange) = bridge_at(IdentificationStage::ApiVersion);
        assert_failure(
            bridge,
            exchange,
            Direction::Reply,
            CommandId::FcVariant,
            b"BTFL",
            ("MalformedResponse", "API_VERSION", "WrongCommand"),
        );

        let (bridge, exchange) = bridge_at(IdentificationStage::ApiVersion);
        assert_failure(
            bridge,
            exchange,
            Direction::Error,
            CommandId::ApiVersion,
            &[],
            ("ProtocolIdentityFailure", "API_VERSION", "ErrorReply"),
        );
    }

    #[test]
    fn every_identity_stage_rejects_wrong_command_direction_and_error_reply() {
        for stage in [
            IdentificationStage::ApiVersion,
            IdentificationStage::FcVariant,
            IdentificationStage::FcVersion,
            IdentificationStage::BoardInfo,
        ] {
            let (command, payload) = stage_case(stage);
            let wrong_command = if command == CommandId::ApiVersion {
                CommandId::FcVariant
            } else {
                CommandId::ApiVersion
            };

            let (bridge, exchange) = bridge_at(stage);
            assert_failure(
                bridge,
                exchange,
                Direction::Reply,
                wrong_command,
                &payload,
                (
                    "MalformedResponse",
                    ReadonlyStage::from_identification(stage).unwrap().label(),
                    "WrongCommand",
                ),
            );

            let (bridge, exchange) = bridge_at(stage);
            assert_failure(
                bridge,
                exchange,
                Direction::Request,
                command,
                &payload,
                (
                    "MalformedResponse",
                    ReadonlyStage::from_identification(stage).unwrap().label(),
                    "WrongDirection",
                ),
            );

            let (bridge, exchange) = bridge_at(stage);
            assert_failure(
                bridge,
                exchange,
                Direction::Error,
                command,
                &payload,
                (
                    "ProtocolIdentityFailure",
                    ReadonlyStage::from_identification(stage).unwrap().label(),
                    "ErrorReply",
                ),
            );
        }
    }

    #[test]
    fn trace_events_are_rust_authoritative_fixed_vocabulary_only() {
        let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
        let open = bridge.begin_open().unwrap();
        assert!(bridge.take_trace_event().is_none());

        let exchange = bridge.accept_open_ok(&open.request_id).unwrap();
        let directive = bridge.take_trace_event().unwrap().event;
        assert_eq!(directive.layer, "RUST");
        assert_eq!(directive.phase, "API_VERSION");
        assert_eq!(directive.event, "DIRECTIVE");
        assert_eq!(directive.stage, ReadonlyStage::ApiVersion);
        assert_eq!(directive.command, CommandId::ApiVersion);
        assert_eq!(directive.byte_count, Some(6));
        assert_eq!(directive.direction, Some(Direction::Request));
        assert_eq!(directive.failure_class, None);
        assert_eq!(directive.failure_reason, None);
        assert_eq!(directive.origin, None);
        assert!(bridge.take_trace_event().is_none());

        let frame = encode_frame(Direction::Reply, CommandId::ApiVersion, &[0, 1, 46]).unwrap();
        assert!(
            bridge
                .accept_chunk(&exchange.request_id, &frame[..2])
                .unwrap()
                .is_none()
        );
        assert!(bridge.take_trace_event().is_none());
        assert!(
            bridge
                .accept_chunk(&exchange.request_id, &frame[2..])
                .unwrap()
                .is_some()
        );

        let frame_event = bridge.take_trace_event().unwrap().event;
        assert_eq!(frame_event.layer, "MSP");
        assert_eq!(frame_event.phase, "MSP_FRAME");
        assert_eq!(frame_event.event, "FRAME_ACCEPTED");
        assert_eq!(frame_event.command, CommandId::ApiVersion);
        assert_eq!(frame_event.direction, Some(Direction::Reply));
        assert_eq!(frame_event.failure_class, None);

        let identity_event = bridge.take_trace_event().unwrap().event;
        assert_eq!(identity_event.layer, "RUST");
        assert_eq!(identity_event.phase, "IDENTITY_STAGE");
        assert_eq!(identity_event.event, "IDENTITY_STAGE_OK");
        assert_eq!(identity_event.stage, ReadonlyStage::ApiVersion);
        assert_eq!(identity_event.failure_reason, None);

        let next_directive = bridge.take_trace_event().unwrap().event;
        assert_eq!(next_directive.stage, ReadonlyStage::FcVariant);
        assert_eq!(next_directive.command, CommandId::FcVariant);
        assert!(bridge.take_trace_event().is_none());
    }

    #[test]
    fn malformed_frames_emit_only_a_structural_rejection_category() {
        let (mut bridge, exchange) = bridge_at(IdentificationStage::ApiVersion);
        while bridge.take_trace_event().is_some() {}

        let mut frame = encode_frame(Direction::Reply, CommandId::ApiVersion, &[0, 1, 46]).unwrap();
        *frame.last_mut().unwrap() ^= 0xff;
        let close = bridge
            .accept_chunk(&exchange.request_id, &frame)
            .unwrap()
            .unwrap();
        assert_eq!(close.kind, "close");

        let rejected = bridge.take_trace_event().unwrap().event;
        assert_eq!(rejected.event, "FRAME_REJECTED");
        assert_eq!(rejected.stage, ReadonlyStage::ApiVersion);
        assert_eq!(rejected.command, CommandId::ApiVersion);
        assert_eq!(rejected.byte_count, None);
        assert_eq!(rejected.direction, None);
        assert_eq!(rejected.failure_class, Some("MalformedResponse"));
        assert_eq!(
            rejected.failure_reason,
            Some(IdentityFailureReason::BadChecksum)
        );
        assert_eq!(rejected.origin, Some("MSP_FRAME"));
        assert!(bridge.take_trace_event().is_none());
    }

    #[test]
    fn randomized_chunk_segmentation_preserves_identity_and_snapshot_trace() {
        for initial_seed in 1_u32..=64 {
            let mut seed = initial_seed;
            let mut bridge = WasmReadonlySerialDiscovery::create().unwrap();
            let open = bridge.begin_open().unwrap();
            let mut directive = bridge.accept_open_ok(&open.request_id).unwrap();
            let replies = [
                (CommandId::ApiVersion, vec![0, 1, 46]),
                (CommandId::FcVariant, b"BTFL".to_vec()),
                (CommandId::FcVersion, vec![4, 5, 5]),
                (CommandId::BoardInfo, valid_board_payload()),
                (CommandId::BeeperConfig, valid_snapshot_payload()),
            ];

            for (command, payload) in replies {
                let frame = encode_frame(Direction::Reply, command, &payload).unwrap();
                let mut offset = 0;
                let mut next = None;
                while offset < frame.len() {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let length = usize::try_from(seed % 7 + 1).unwrap();
                    let end = (offset + length).min(frame.len());
                    let result = bridge
                        .accept_chunk(&directive.request_id, &frame[offset..end])
                        .unwrap();
                    if result.is_some() {
                        assert_eq!(end, frame.len());
                        next = result;
                    }
                    offset = end;
                }
                directive = next.expect("the final segment must advance the Rust state machine");
            }
            assert_eq!(directive.kind, "close");

            let events: Vec<_> = bridge.trace_events.drain(..).collect();
            assert_eq!(events.len(), 15);
            for (index, stage) in [
                ReadonlyStage::ApiVersion,
                ReadonlyStage::FcVariant,
                ReadonlyStage::FcVersion,
                ReadonlyStage::BoardInfo,
            ]
            .into_iter()
            .enumerate()
            {
                let triplet = &events[index * 3..index * 3 + 3];
                assert_eq!(triplet[0].event, "DIRECTIVE");
                assert_eq!(triplet[1].event, "FRAME_ACCEPTED");
                assert_eq!(triplet[2].event, "IDENTITY_STAGE_OK");
                assert!(triplet.iter().all(|event| event.stage == stage));
            }
            let snapshot = &events[12..15];
            assert_eq!(snapshot[0].event, "DIRECTIVE");
            assert_eq!(snapshot[1].event, "FRAME_ACCEPTED");
            assert_eq!(snapshot[2].event, "SNAPSHOT_STAGE_OK");
            assert!(
                snapshot
                    .iter()
                    .all(|event| event.stage == ReadonlyStage::BeeperConfig)
            );
        }
    }

    #[test]
    fn successful_identity_path_has_no_failure_diagnostic() {
        let (mut bridge, exchange) = bridge_at(IdentificationStage::BoardInfo);
        let snapshot = feed_reply(
            &mut bridge,
            &exchange,
            Direction::Reply,
            CommandId::BoardInfo,
            &valid_board_payload(),
        );
        assert_eq!(snapshot.kind, "exchange-snapshot-read");
        let request = decode_frame(&snapshot.bytes).unwrap();
        assert_eq!(request.known_command(), Some(CommandId::BeeperConfig));
        assert_eq!(request.payload_len(), 0);
        let close = feed_reply(
            &mut bridge,
            &snapshot,
            Direction::Reply,
            CommandId::BeeperConfig,
            &valid_snapshot_payload(),
        );
        assert_eq!(close.kind, "close");
        assert!(bridge.failure_class().is_none());
        assert!(bridge.failure_stage().is_none());
        assert!(bridge.failure_reason().is_none());
        bridge.accept_close(&close.request_id, None).unwrap();
        assert_eq!(bridge.outcome_kind(), "in-scope");
        assert_eq!(bridge.read_profile_id().as_deref(), Some("api-1.46-legacy"));
        assert_eq!(
            bridge.read_profile_write_authority().as_deref(),
            Some("never-authorizes-writes")
        );
        assert_eq!(
            bridge.capability_status().as_deref(),
            Some("review-only-match")
        );
        assert_eq!(
            bridge.capability_pack_id().as_deref(),
            Some("bf-4.5.5-api1.46-speedybeef405v4-review")
        );
        assert_eq!(
            bridge.capability_trust().as_deref(),
            Some("review-only-embedded")
        );
        assert_eq!(
            bridge.capability_write_policy().as_deref(),
            Some("writes-blocked")
        );
        assert_eq!(
            bridge.snapshot_status().as_deref(),
            Some("beeper-config-complete")
        );
        assert_eq!(
            bridge.beeper_off_flags(),
            Some(ade_protocol_msp::SYSTEM_INIT_OFF_MASK)
        );
        assert_eq!(bridge.dshot_beacon_tone(), Some(3));
        assert_eq!(bridge.dshot_beacon_off_flags(), Some(0x0102_0304));
        assert_eq!(bridge.system_init_disabled(), Some(true));
        assert!(!bridge.hardware_observed());
    }

    #[test]
    fn snapshot_payload_failures_are_typed_and_fail_closed() {
        let (mut bridge, board) = bridge_at(IdentificationStage::BoardInfo);
        let snapshot = feed_reply(
            &mut bridge,
            &board,
            Direction::Reply,
            CommandId::BoardInfo,
            &valid_board_payload(),
        );
        assert_failure(
            bridge,
            snapshot,
            Direction::Reply,
            CommandId::BeeperConfig,
            &[0; 8],
            ("ProtocolSnapshotFailure", "BEEPER_CONFIG", "WrongLength"),
        );

        let (mut bridge, board) = bridge_at(IdentificationStage::BoardInfo);
        let snapshot = feed_reply(
            &mut bridge,
            &board,
            Direction::Reply,
            CommandId::BoardInfo,
            &valid_board_payload(),
        );
        assert_failure(
            bridge,
            snapshot,
            Direction::Error,
            CommandId::BeeperConfig,
            &[],
            ("ProtocolSnapshotFailure", "BEEPER_CONFIG", "ErrorReply"),
        );

        let (mut bridge, board) = bridge_at(IdentificationStage::BoardInfo);
        let snapshot = feed_reply(
            &mut bridge,
            &board,
            Direction::Reply,
            CommandId::BoardInfo,
            &valid_board_payload(),
        );
        assert_failure(
            bridge,
            snapshot,
            Direction::Reply,
            CommandId::ApiVersion,
            &[0, 1, 46],
            ("MalformedResponse", "BEEPER_CONFIG", "WrongCommand"),
        );
    }

    #[test]
    fn legacy_scope_mismatch_keeps_profile_evidence_but_selects_no_capability() {
        let (mut bridge, exchange) = bridge_at(IdentificationStage::BoardInfo);
        let close = feed_reply(
            &mut bridge,
            &exchange,
            Direction::Reply,
            CommandId::BoardInfo,
            &board_payload("UNREVIEWED_TARGET"),
        );
        bridge.accept_close(&close.request_id, None).unwrap();
        assert_eq!(bridge.outcome_kind(), "scope-mismatch");
        assert_eq!(bridge.read_profile_id().as_deref(), Some("api-1.46-legacy"));
        assert_eq!(
            bridge.read_profile_write_authority().as_deref(),
            Some("never-authorizes-writes")
        );
        assert_eq!(
            bridge.capability_status().as_deref(),
            Some("no-reviewed-match")
        );
        assert!(bridge.capability_pack_id().is_none());
        assert!(bridge.capability_trust().is_none());
        assert!(bridge.capability_write_policy().is_none());
        assert!(!bridge.hardware_observed());
    }
}
