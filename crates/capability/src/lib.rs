#![forbid(unsafe_code)]

//! # `ade-capability` — firmware capability knowledge without hardware authority
//!
//! M1 retains one deliberately narrow proposed target. M3 starts the descriptive capability-pack
//! layer from ADR-0007 without adding a driver, parser, transport or write path. The first M3
//! contract is review-only and read-only: it can describe an exact firmware/API/target tuple and
//! resolve observed identity facts against that description, but it cannot authorise a hardware
//! write.

/// How much confidence a legacy M1 target claim carries. There is intentionally no "Supported"
/// or "Validated" value below real hardware observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    /// Proposed on paper; not validated as supported hardware.
    ProposedNotHardwareValidated,
}

impl ValidationStatus {
    /// The exact status string used across reports and documentation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ValidationStatus::ProposedNotHardwareValidated => "PROPOSED — NOT HARDWARE VALIDATED",
        }
    }
}

/// The single proposed target of the M1 beeper slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposedTarget {
    /// Human board name.
    pub board: &'static str,
    /// Betaflight target identifier.
    pub betaflight_target: &'static str,
    /// Firmware family and version.
    pub firmware: &'static str,
    /// MSP API version.
    pub msp_api_version: &'static str,
    /// Validation status — always proposed in M1.
    pub status: ValidationStatus,
}

/// The proposed M1 target: SpeedyBee F405 V4 running Betaflight 4.5.5 (MSP API 1.46).
#[must_use]
pub const fn m1_proposed_target() -> ProposedTarget {
    ProposedTarget {
        board: "SpeedyBee F405 V4",
        betaflight_target: "SPEEDYBEEF405V4",
        firmware: "Betaflight 4.5.5",
        msp_api_version: "1.46",
        status: ValidationStatus::ProposedNotHardwareValidated,
    }
}

/// Internal firmware-family discriminator used only by the compatibility engine.
///
/// Firmware family is not product identity and need not be shown in the ordinary UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareFamily {
    /// Betaflight compatibility family.
    Betaflight,
    /// INAV compatibility family.
    Inav,
}

/// Three-byte firmware version identity used by the currently modelled read-only profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FirmwareVersion {
    /// Major or calendar-year byte, depending on the firmware profile.
    pub major: u8,
    /// Minor or calendar-month byte, depending on the firmware profile.
    pub minor: u8,
    /// Patch or calendar-maintenance byte, depending on the firmware profile.
    pub patch: u8,
}

impl FirmwareVersion {
    /// Construct an exact three-byte firmware version.
    #[must_use]
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

/// Inclusive MSP API range described by a capability pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiRange {
    /// MSP protocol version byte.
    pub protocol_version: u8,
    /// MSP API major byte.
    pub api_major: u8,
    /// Inclusive lower API minor bound.
    pub min_minor: u8,
    /// Inclusive upper API minor bound.
    pub max_minor: u8,
}

impl ApiRange {
    /// Whether an observed protocol/API tuple is inside this exact descriptive range.
    #[must_use]
    pub fn contains(self, protocol_version: u8, api_major: u8, api_minor: u8) -> bool {
        self.protocol_version == protocol_version
            && self.api_major == api_major
            && api_minor >= self.min_minor
            && api_minor <= self.max_minor
    }
}

/// Inclusive firmware-version range described by a capability pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareVersionRange {
    /// Inclusive lower version bound.
    pub min: FirmwareVersion,
    /// Inclusive upper version bound.
    pub max: FirmwareVersion,
}

impl FirmwareVersionRange {
    /// Whether an observed firmware version is inside this inclusive range.
    #[must_use]
    pub fn contains(self, version: FirmwareVersion) -> bool {
        version >= self.min && version <= self.max
    }
}

/// Firmware-version shape required by a capability descriptor.
///
/// Calendar releases keep both the raw three-byte tuple and the exact published version string.
/// They must not collapse into the legacy numeric range because doing so could match a different
/// extended string that happens to reuse the same three bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareVersionSelector {
    /// Inclusive legacy three-byte version range.
    LegacyRange(FirmwareVersionRange),
    /// Exact calendar tuple and exact length-prefixed version string.
    CalendarExtendedExact {
        /// Exact three bytes carried by the calendar-version reply.
        calendar_version: FirmwareVersion,
        /// Exact UTF-8 version string carried by the same reply.
        version_string: &'static str,
    },
}

/// Observed firmware-version shape passed into capability resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedFirmwareVersion<'a> {
    /// Legacy three-byte semantic version.
    Legacy(FirmwareVersion),
    /// Calendar tuple plus the exact version string from the same decoded reply.
    CalendarExtended {
        /// Observed three-byte calendar tuple.
        calendar_version: FirmwareVersion,
        /// Observed strict UTF-8 version string.
        version_string: &'a str,
    },
}

/// Target selector for the initial fail-closed M3 descriptor.
///
/// M3 intentionally has no wildcard selector: a review-only descriptor must name the exact target
/// it describes instead of broadening knowledge to hardware that was never reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSelector {
    /// Exact firmware target name.
    Exact(&'static str),
}

/// Trust state available before signed-pack distribution infrastructure exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityPackTrust {
    /// Repository-embedded descriptor reviewed with source, but not a signed distributable pack.
    ReviewOnlyEmbedded,
}

/// Hardware-write policy representable by the initial M3 capability-pack model.
///
/// There is deliberately no write-enabled variant in this milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityPackWritePolicy {
    /// The descriptor may inform read-only compatibility logic only.
    WritesBlocked,
}

/// Purely descriptive capability-pack metadata.
///
/// This type carries no command id, payload, transport handle, callback, executable expression or
/// write approval. Signing/checksum verification for distributable packs remains a later
/// knowledge-platform concern; this M3 schema is explicitly review-only and embedded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityPackDescriptor {
    /// Stable pack identifier.
    pub pack_id: &'static str,
    /// Data-schema version. Zero is invalid.
    pub schema_version: u16,
    /// Pack revision. Zero is invalid.
    pub pack_version: u32,
    /// Stable revocation identifier reserved for future distribution governance.
    pub revocation_id: &'static str,
    /// Firmware family described by this pack.
    pub family: FirmwareFamily,
    /// MSP API range described by this pack.
    pub api_range: ApiRange,
    /// Exact firmware-version shape described by this pack.
    pub version_selector: FirmwareVersionSelector,
    /// Exact target selector.
    pub target: TargetSelector,
    /// Current trust boundary.
    pub trust: CapabilityPackTrust,
    /// Current write boundary — M3 can only block writes.
    pub write_policy: CapabilityPackWritePolicy,
}

/// Structural descriptor validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorError {
    /// Pack id is empty.
    EmptyPackId,
    /// Schema version is zero.
    ZeroSchemaVersion,
    /// Pack revision is zero.
    ZeroPackVersion,
    /// Revocation id is empty.
    EmptyRevocationId,
    /// API minor range is inverted.
    InvertedApiRange,
    /// Firmware version range is inverted.
    InvertedVersionRange,
    /// Exact calendar version string is empty.
    EmptyCalendarVersionString,
    /// Exact target name is empty.
    EmptyExactTarget,
}

/// Validate one descriptor before it can participate in resolution.
///
/// # Errors
/// Returns a typed structural error for malformed or dangerously broad metadata.
pub fn validate_pack_descriptor(pack: &CapabilityPackDescriptor) -> Result<(), DescriptorError> {
    if pack.pack_id.is_empty() {
        return Err(DescriptorError::EmptyPackId);
    }
    if pack.schema_version == 0 {
        return Err(DescriptorError::ZeroSchemaVersion);
    }
    if pack.pack_version == 0 {
        return Err(DescriptorError::ZeroPackVersion);
    }
    if pack.revocation_id.is_empty() {
        return Err(DescriptorError::EmptyRevocationId);
    }
    if pack.api_range.min_minor > pack.api_range.max_minor {
        return Err(DescriptorError::InvertedApiRange);
    }
    match pack.version_selector {
        FirmwareVersionSelector::LegacyRange(range) if range.min > range.max => {
            return Err(DescriptorError::InvertedVersionRange);
        }
        FirmwareVersionSelector::CalendarExtendedExact {
            version_string: "", ..
        } => return Err(DescriptorError::EmptyCalendarVersionString),
        FirmwareVersionSelector::LegacyRange(_)
        | FirmwareVersionSelector::CalendarExtendedExact { .. } => {}
    }
    match pack.target {
        TargetSelector::Exact("") => return Err(DescriptorError::EmptyExactTarget),
        TargetSelector::Exact(_) => {}
    }
    Ok(())
}

/// Minimal observed identity view required for capability-pack resolution.
///
/// It intentionally contains no serial number, USB metadata, per-unit identifier or raw protocol
/// bytes. Construction from the authoritative facts layer is a later integration step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedFirmwareIdentity<'a> {
    /// Firmware family obtained from the read-only identity sequence.
    pub family: FirmwareFamily,
    /// MSP protocol version byte.
    pub protocol_version: u8,
    /// MSP API major byte.
    pub api_major: u8,
    /// MSP API minor byte.
    pub api_minor: u8,
    /// Firmware-version shape under the selected read-only profile.
    pub version: ObservedFirmwareVersion<'a>,
    /// Exact firmware target name.
    pub target_name: &'a str,
}

impl CapabilityPackDescriptor {
    fn matches(self, identity: &ObservedFirmwareIdentity<'_>) -> bool {
        if self.family != identity.family {
            return false;
        }
        if !self.api_range.contains(
            identity.protocol_version,
            identity.api_major,
            identity.api_minor,
        ) {
            return false;
        }
        let version_matches = match (self.version_selector, identity.version) {
            (
                FirmwareVersionSelector::LegacyRange(range),
                ObservedFirmwareVersion::Legacy(version),
            ) => range.contains(version),
            (
                FirmwareVersionSelector::CalendarExtendedExact {
                    calendar_version: expected_calendar,
                    version_string: expected_string,
                },
                ObservedFirmwareVersion::CalendarExtended {
                    calendar_version,
                    version_string,
                },
            ) => calendar_version == expected_calendar && version_string == expected_string,
            _ => false,
        };
        if !version_matches {
            return false;
        }
        match self.target {
            TargetSelector::Exact(target) => identity.target_name == target,
        }
    }
}

/// Fail-closed result of resolving review-only capability knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyPackResolution<'a> {
    /// No reviewed descriptor matches the observed identity.
    NoMatch,
    /// Exactly one validated descriptor matches.
    Match(&'a CapabilityPackDescriptor),
    /// More than one descriptor matches; the engine refuses to choose one implicitly.
    Ambiguous,
    /// A malformed descriptor was present. Resolution stops instead of ignoring bad knowledge.
    InvalidPack {
        /// Zero-based index in the supplied descriptor slice.
        index: usize,
        /// Structural failure.
        reason: DescriptorError,
    },
}

/// Resolve one observed identity against review-only descriptive packs.
///
/// The resolver validates every descriptor before considering matches. An invalid descriptor or
/// more than one matching descriptor is terminal and fail-closed. A match still carries
/// [`CapabilityPackWritePolicy::WritesBlocked`]; this function cannot authorise hardware writes.
#[must_use]
pub fn resolve_read_only_pack<'a>(
    identity: &ObservedFirmwareIdentity<'_>,
    packs: &'a [CapabilityPackDescriptor],
) -> ReadOnlyPackResolution<'a> {
    for (index, pack) in packs.iter().enumerate() {
        if let Err(reason) = validate_pack_descriptor(pack) {
            return ReadOnlyPackResolution::InvalidPack { index, reason };
        }
    }

    let mut matched = None;
    for pack in packs {
        if !pack.matches(identity) {
            continue;
        }
        if matched.is_some() {
            return ReadOnlyPackResolution::Ambiguous;
        }
        matched = Some(pack);
    }

    match matched {
        Some(pack) => ReadOnlyPackResolution::Match(pack),
        None => ReadOnlyPackResolution::NoMatch,
    }
}

/// First M3 review-only descriptor.
///
/// This is descriptive knowledge for the exact legacy M1 tuple only. It is not a hardware-support
/// claim and it cannot enable writes.
#[must_use]
pub const fn m3_review_only_betaflight_4_5_5_pack() -> CapabilityPackDescriptor {
    CapabilityPackDescriptor {
        pack_id: "bf-4.5.5-api1.46-speedybeef405v4-review",
        schema_version: 1,
        pack_version: 1,
        revocation_id: "bf-4.5.5-api1.46-speedybeef405v4-review-v1",
        family: FirmwareFamily::Betaflight,
        api_range: ApiRange {
            protocol_version: 0,
            api_major: 1,
            min_minor: 46,
            max_minor: 46,
        },
        version_selector: FirmwareVersionSelector::LegacyRange(FirmwareVersionRange {
            min: FirmwareVersion::new(4, 5, 5),
            max: FirmwareVersion::new(4, 5, 5),
        }),
        target: TargetSelector::Exact("SPEEDYBEEF405V4"),
        trust: CapabilityPackTrust::ReviewOnlyEmbedded,
        write_policy: CapabilityPackWritePolicy::WritesBlocked,
    }
}

/// Review-only descriptor for the pinned API 1.47 calendar-version identity.
///
/// The descriptor matches only the exact API tuple, exact calendar bytes, exact version string and
/// exact proposed target. It is descriptive repository knowledge, not hardware validation, and its
/// only representable write policy remains [`CapabilityPackWritePolicy::WritesBlocked`].
#[must_use]
pub const fn m3_review_only_betaflight_2025_12_1_pack() -> CapabilityPackDescriptor {
    CapabilityPackDescriptor {
        pack_id: "bf-2025.12.1-api1.47-speedybeef405v4-review",
        schema_version: 1,
        pack_version: 1,
        revocation_id: "bf-2025.12.1-api1.47-speedybeef405v4-review-v1",
        family: FirmwareFamily::Betaflight,
        api_range: ApiRange {
            protocol_version: 0,
            api_major: 1,
            min_minor: 47,
            max_minor: 47,
        },
        version_selector: FirmwareVersionSelector::CalendarExtendedExact {
            calendar_version: FirmwareVersion::new(25, 12, 1),
            version_string: "2025.12.1",
        },
        target: TargetSelector::Exact("SPEEDYBEEF405V4"),
        trust: CapabilityPackTrust::ReviewOnlyEmbedded,
        write_policy: CapabilityPackWritePolicy::WritesBlocked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_identity() -> ObservedFirmwareIdentity<'static> {
        ObservedFirmwareIdentity {
            family: FirmwareFamily::Betaflight,
            protocol_version: 0,
            api_major: 1,
            api_minor: 46,
            version: ObservedFirmwareVersion::Legacy(FirmwareVersion::new(4, 5, 5)),
            target_name: "SPEEDYBEEF405V4",
        }
    }

    fn exact_calendar_identity() -> ObservedFirmwareIdentity<'static> {
        ObservedFirmwareIdentity {
            family: FirmwareFamily::Betaflight,
            protocol_version: 0,
            api_major: 1,
            api_minor: 47,
            version: ObservedFirmwareVersion::CalendarExtended {
                calendar_version: FirmwareVersion::new(25, 12, 1),
                version_string: "2025.12.1",
            },
            target_name: "SPEEDYBEEF405V4",
        }
    }

    #[test]
    fn the_legacy_target_is_proposed_and_never_claims_support() {
        let target = m1_proposed_target();
        assert_eq!(
            target.status,
            ValidationStatus::ProposedNotHardwareValidated
        );
        assert_eq!(target.status.as_str(), "PROPOSED — NOT HARDWARE VALIDATED");
        assert!(!target.status.as_str().to_lowercase().contains("supported"));
        assert_eq!(target.betaflight_target, "SPEEDYBEEF405V4");
    }

    #[test]
    fn first_m3_descriptor_is_valid_review_only_and_write_blocked() {
        let pack = m3_review_only_betaflight_4_5_5_pack();
        assert_eq!(validate_pack_descriptor(&pack), Ok(()));
        assert_eq!(pack.trust, CapabilityPackTrust::ReviewOnlyEmbedded);
        assert_eq!(pack.write_policy, CapabilityPackWritePolicy::WritesBlocked);
    }

    #[test]
    fn exact_reviewed_identity_resolves_to_exactly_one_pack() {
        let pack = m3_review_only_betaflight_4_5_5_pack();
        let packs = [pack];
        assert_eq!(
            resolve_read_only_pack(&exact_identity(), &packs),
            ReadOnlyPackResolution::Match(&packs[0])
        );
    }

    #[test]
    fn api_version_target_version_and_family_mismatches_do_not_match() {
        let pack = m3_review_only_betaflight_4_5_5_pack();
        let packs = [pack];

        let mut api = exact_identity();
        api.api_minor = 47;
        assert_eq!(
            resolve_read_only_pack(&api, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut target = exact_identity();
        target.target_name = "OTHER";
        assert_eq!(
            resolve_read_only_pack(&target, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut version = exact_identity();
        version.version = ObservedFirmwareVersion::Legacy(FirmwareVersion::new(4, 5, 4));
        assert_eq!(
            resolve_read_only_pack(&version, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut family = exact_identity();
        family.family = FirmwareFamily::Inav;
        assert_eq!(
            resolve_read_only_pack(&family, &packs),
            ReadOnlyPackResolution::NoMatch
        );
    }

    #[test]
    fn exact_calendar_descriptor_is_review_only_write_blocked_and_matches_exactly() {
        let pack = m3_review_only_betaflight_2025_12_1_pack();
        let packs = [pack];
        assert_eq!(validate_pack_descriptor(&pack), Ok(()));
        assert_eq!(pack.trust, CapabilityPackTrust::ReviewOnlyEmbedded);
        assert_eq!(pack.write_policy, CapabilityPackWritePolicy::WritesBlocked);
        assert_eq!(
            resolve_read_only_pack(&exact_calendar_identity(), &packs),
            ReadOnlyPackResolution::Match(&packs[0])
        );
    }

    #[test]
    fn calendar_descriptor_rejects_api_target_calendar_string_and_shape_drift() {
        let pack = m3_review_only_betaflight_2025_12_1_pack();
        let packs = [pack];

        let mut api = exact_calendar_identity();
        api.api_minor = 46;
        assert_eq!(
            resolve_read_only_pack(&api, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut target = exact_calendar_identity();
        target.target_name = "SPEEDYBEEF405V5";
        assert_eq!(
            resolve_read_only_pack(&target, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut calendar = exact_calendar_identity();
        calendar.version = ObservedFirmwareVersion::CalendarExtended {
            calendar_version: FirmwareVersion::new(25, 12, 2),
            version_string: "2025.12.1",
        };
        assert_eq!(
            resolve_read_only_pack(&calendar, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut string = exact_calendar_identity();
        string.version = ObservedFirmwareVersion::CalendarExtended {
            calendar_version: FirmwareVersion::new(25, 12, 1),
            version_string: "2025.12.1-custom",
        };
        assert_eq!(
            resolve_read_only_pack(&string, &packs),
            ReadOnlyPackResolution::NoMatch
        );

        let mut legacy_shape = exact_calendar_identity();
        legacy_shape.version = ObservedFirmwareVersion::Legacy(FirmwareVersion::new(25, 12, 1));
        assert_eq!(
            resolve_read_only_pack(&legacy_shape, &packs),
            ReadOnlyPackResolution::NoMatch
        );
    }

    #[test]
    fn range_endpoints_are_inclusive() {
        let api = ApiRange {
            protocol_version: 0,
            api_major: 1,
            min_minor: 46,
            max_minor: 47,
        };
        assert!(api.contains(0, 1, 46));
        assert!(api.contains(0, 1, 47));
        assert!(!api.contains(0, 1, 45));
        assert!(!api.contains(1, 1, 46));

        let versions = FirmwareVersionRange {
            min: FirmwareVersion::new(4, 5, 5),
            max: FirmwareVersion::new(4, 5, 7),
        };
        assert!(versions.contains(FirmwareVersion::new(4, 5, 5)));
        assert!(versions.contains(FirmwareVersion::new(4, 5, 7)));
        assert!(!versions.contains(FirmwareVersion::new(4, 5, 8)));
    }

    #[test]
    fn malformed_descriptor_stops_resolution_instead_of_being_ignored() {
        let mut invalid = m3_review_only_betaflight_4_5_5_pack();
        invalid.api_range.min_minor = 47;
        invalid.api_range.max_minor = 46;
        let valid = m3_review_only_betaflight_4_5_5_pack();
        let packs = [invalid, valid];
        assert_eq!(
            resolve_read_only_pack(&exact_identity(), &packs),
            ReadOnlyPackResolution::InvalidPack {
                index: 0,
                reason: DescriptorError::InvertedApiRange,
            }
        );
    }

    #[test]
    fn duplicate_matches_are_ambiguous_and_never_selected_implicitly() {
        let pack = m3_review_only_betaflight_4_5_5_pack();
        let packs = [pack, pack];
        assert_eq!(
            resolve_read_only_pack(&exact_identity(), &packs),
            ReadOnlyPackResolution::Ambiguous
        );
    }

    #[test]
    fn descriptor_validation_rejects_empty_and_zero_governance_fields() {
        let mut pack = m3_review_only_betaflight_4_5_5_pack();
        pack.pack_id = "";
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::EmptyPackId)
        );

        let mut pack = m3_review_only_betaflight_4_5_5_pack();
        pack.schema_version = 0;
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::ZeroSchemaVersion)
        );

        let mut pack = m3_review_only_betaflight_4_5_5_pack();
        pack.pack_version = 0;
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::ZeroPackVersion)
        );

        let mut pack = m3_review_only_betaflight_4_5_5_pack();
        pack.revocation_id = "";
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::EmptyRevocationId)
        );

        let mut pack = m3_review_only_betaflight_4_5_5_pack();
        pack.version_selector = FirmwareVersionSelector::LegacyRange(FirmwareVersionRange {
            min: FirmwareVersion::new(4, 5, 6),
            max: FirmwareVersion::new(4, 5, 5),
        });
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::InvertedVersionRange)
        );

        let mut pack = m3_review_only_betaflight_2025_12_1_pack();
        pack.version_selector = FirmwareVersionSelector::CalendarExtendedExact {
            calendar_version: FirmwareVersion::new(25, 12, 1),
            version_string: "",
        };
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::EmptyCalendarVersionString)
        );

        let mut pack = m3_review_only_betaflight_4_5_5_pack();
        pack.target = TargetSelector::Exact("");
        assert_eq!(
            validate_pack_descriptor(&pack),
            Err(DescriptorError::EmptyExactTarget)
        );
    }
}
