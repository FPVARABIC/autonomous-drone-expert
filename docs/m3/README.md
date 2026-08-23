# M3 — Read-only capability-pack resolution

**Status:** slices 1–7 implemented; all hardware evidence remains unchanged

M3 begins the firmware capability-pack layer accepted by ADR-0007. This milestone does not add a
hardware write, a driver, a transport, an arbitrary command table or a signed-pack distribution
system. Its current contracts let reviewed firmware knowledge drive additional **read-only**
identity layouts without expanding hardware-write eligibility.

## Starting point

M2 is merged to `main`. The production Web/PWA path can perform the Rust-owned read-only identity
sequence, and the integrated M2 tree passed canonical CI and Android development-validation
packaging. Physical evidence remains bounded:

- `PHYSICAL_FC_TEST_ATTEMPTED=YES`
- `PHYSICAL_USB_SELECTION_OBSERVED=YES`
- `PHYSICAL_API_SCOPE_GATE_OBSERVED=YES`
- `UNSUPPORTED_API_OUTCOME_OBSERVED=YES`
- `READONLY_IDENTITY_COMPLETION=NO`
- `HARDWARE_SUPPORT_VALIDATED=NO`

M3 does not reinterpret or inflate those facts.

## Slice 1 — descriptive schema and fail-closed resolver

`ade-capability` models review-only firmware knowledge with exact protocol/API/version/target
selectors, explicit trust/revision data, `WritesBlocked`, validation before matching, and
fail-closed ambiguous/no-match results. The first embedded descriptor covers only Betaflight 4.5.5
/ MSP protocol 0 / API 1.46 / `SPEEDYBEEF405V4`; it is descriptive knowledge, not a hardware
support claim.

## Slice 2 — authoritative facts adapter

`ade-capability-resolution` accepts only an already-decoded `DeviceIdentity`, maps only exact
`BTFL` to the reviewed Betaflight family, drops identity fields not needed for capability
selection, and returns review-only/write-blocked knowledge. It owns no protocol parser, transport,
device handle or write approval and does not replace the M1 write-scope check.

## Slice 3 — pinned API 1.47 identity research

The clean-room provenance set for Betaflight `2025.12.1` at MSP protocol `0` / API `1.47` is
recorded against official upstream commit `85d201376a1fc33b223c27448808c2cc7b8f2743`:

- `MSP_API_VERSION`: three-byte protocol/API tuple;
- `MSP_FC_VARIANT`: exact four-byte `BTFL` identifier;
- `MSP_FC_VERSION`: three calendar-version bytes plus one-byte string length and version string;
- `MSP_BOARD_INFO`: the bounded complete field sequence through API 1.47, with the 32-byte
  per-unit signature excluded from stable identity/persistence.

These records remain source facts, not proof that the owner's physical FC is API 1.47 or
Betaflight 2025.12.1.

## Slice 4 — readable profile versus write eligibility

`ade-readonly-profile` separates readable identity layouts from M1 write eligibility. It knows two
exact candidates:

- protocol 0 / API 1.46: legacy three-byte `FC_VERSION`;
- protocol 0 / API 1.47: calendar triplet plus length-prefixed version string.

Both require exact `BTFL`. Unknown protocol/API/variant combinations fail closed. Every candidate
permanently carries `NeverAuthorizesWrites`. A regression test proves API 1.47 can be known to the
read registry while `ade-facts::check_m1_api_scope` still rejects it for writes.

## Slice 5 — strict profile-gated `FC_VERSION` decoder

The API 1.47 decoder runs only after the exact variant gate. It accepts the three published
calendar bytes, one `u8` string length, exactly that many strict UTF-8 bytes and no trailing
payload. Wrong command/direction, malformed length, overrun, trailing bytes and invalid UTF-8 all
fail closed. The calendar triplet and version string stay distinct; they are not reinterpreted as
the legacy semantic `FcVersion` type.

## Slice 6 — Rust-owned API 1.47 read-only identification

The production Rust identification state machine now selects the **read profile**, not the narrower
M1 write-scope predicate, after `MSP_API_VERSION`:

1. API 1.46 or reviewed API 1.47 selects a typed read candidate;
2. `MSP_FC_VARIANT` must equal `BTFL` before any profile-specific version decoder can run;
3. `MSP_FC_VERSION` uses the decoder selected by that profile;
4. `MSP_BOARD_INFO` completes the same bounded four-read sequence.

API 1.46 still returns the existing `DeviceIdentity`. API 1.47 instead returns a separate
`ReadonlyProfileIdentity` carrying `NeverAuthorizesWrites`; it cannot be supplied to the legacy M1
write flow by type. The older `Executor::identify()` remains a legacy/write-scope identity API and
refuses the read-only completion rather than converting calendar bytes into a legacy semantic
version.

The Web Serial Rust bridge recognizes this result as `read-only-complete`, exposes only the same
bounded API/variant/version/target fields, keeps `hardwareObserved=false`, and still emits at most
the same four empty-payload identity requests. A reviewed API with a non-`BTFL` variant stops after
the second read as `read-profile-unsupported`; an unknown API still stops after the first read.
The ordinary UI reports that the identity was read successfully **for reading only**, rather than
misclassifying reviewed API 1.47 as an unsupported API.

Because the Rust bridge changed, its checked-in WebAssembly product asset was regenerated on the
trusted Linux canonical path with the pinned Rust 1.85 / isolated Rust 1.97.1 generator and the
existing deterministic path remaps. The provenance policy records the resulting byte hashes; no
new runtime dependency or browser authority was introduced.

## Slice 7 — bounded read-profile and capability-selection evidence

The Rust Web Serial result now carries explicit, allowlisted evidence for the read profile and the
review-only capability decision that produced a completed identity result:

- API 1.46 reports the selected legacy read-profile id and its permanent
  `NeverAuthorizesWrites` boundary. An exact descriptor match additionally reports the stable pack
  id, `ReviewOnlyEmbedded` trust and `WritesBlocked` policy.
- A completed API 1.46 identity outside the exact reviewed descriptor reports
  `no-reviewed-match`; it does not inherit pack trust or a write policy.
- API 1.47 reports the selected calendar-extended read profile and the same permanent no-write
  boundary, but reports `not-reviewed` for capability selection. It cannot claim a capability pack
  until the exact target/version descriptor is reviewed in a separate slice.
- Unsupported APIs, rejected variants and protocol failures expose no profile or capability
  selection evidence.

The browser host and narrow connection facade preserve these fields only in the in-memory bounded
result. The ordinary React product UI neither renders nor persists them. The evidence contains no
raw frame, payload, per-unit signature, USB metadata or new transport authority.

## Safety properties

The current M3 slices cannot represent or perform:

- arbitrary MSP/CLI commands or arbitrary payload actions;
- a new transport or device handle;
- a write approval derived from a read profile;
- SET/SAVE/EEPROM/reboot/motor/arm/DFU/flashing authority;
- native Android FC USB authority;
- a wildcard target match;
- a signed/trusted distribution claim;
- `HARDWARE_SUPPORT_VALIDATED=YES`.

Unknown or ambiguous knowledge remains terminal and fail closed. Read-profile recognition never
changes `ade-facts::check_m1_api_scope`; API 1.47 therefore remains outside the exact M1 hardware
write scope even after read-only identity completion.

## Distribution boundary

ADR-0007 requires distributed capability packs to be signed, checksummed, versioned and
revocable. M3 intentionally stops before that infrastructure. `ReviewOnlyEmbedded` means
repository-reviewed descriptive data only and cannot authorize a write.

## Next M3 work

1. Add a review-only capability descriptor for the newly readable API 1.47 identity only after its
   exact target/version policy is separately reviewed; a read match must remain `WritesBlocked`.
2. Keep all real writes blocked until a later write milestone, compatible backup/recovery evidence,
   and separate owner approval.
3. No new physical operation is required for this software slice; physical evidence remains
   unchanged until a separately approved hardware observation.
