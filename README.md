# Autonomous Drone Expert

> **Internal working name.** The final product name is deferred (see `docs/adr/ADR-0004`).

An independent, offline-first expert for **configuring, diagnosing and programming drones**
from their components and intended flight style. Firmware compatibility engines remain
internal implementation details rather than the product identity.

## Status

**M1 accepted on simulation; G1 merged; M2 production read-only Web/PWA connection merged;
M3 read-only profiles and bounded capability-selection evidence implemented. This is not a
production release and hardware support is not validated.**

M0 foundations are complete. M1 implements one deliberately narrow vertical slice for the
Betaflight 4.5.5 `SYSTEM_INIT` beeper bit over project-owned Mock and Replay transports:
Identify → Read → Plan → Backup → Write → Save → Reboot → Reconnect → Verify →
Recovery/Report. Its write evidence remains simulation evidence only.

M2 integrates the repository-owned React/PWA shell with the deterministic Rust/WASM core,
IndexedDB journal storage, canonical same-origin WASM assets and a Rust-owned Web Serial
read-only identity path. A bounded physical USB-only attempt reached the early API-scope gate
and stopped on an unsupported observed API outcome; it did **not** complete identity and does
not establish hardware support. The Android artifact is a development-validation thin wrapper
with no native flight-controller USB authority.

M3 starts the ADR-0007 capability-pack layer as descriptive review-only data. Its first slice
validates exact firmware/API/version/target descriptors and resolves them fail-closed for
read-only knowledge. It has no write-enabled capability-pack state and adds no hardware
authority. See `docs/m3/README.md`.

There is still no production hardware write transport, firmware flashing, motor control,
Android native FC USB support or hardware-support claim. Real writes remain separately gated by
backup, explicit approval, verification and recovery requirements.

## What this project is

A platform built on three intelligence pillars over a deterministic execution core:

1. **Hardware Knowledge Database** — what a component can do, its limits and known issues.
2. **Intelligent Diagnostic Expert** — investigates faults like an engineer, not an error message.
3. **Knowledge Engine** — accumulates verified experience under strict governance.

The execution core follows one lifecycle:

```
Discover -> Decide -> Plan -> Apply -> Reboot -> Verify -> Diagnose -> Recovery
```

## What this project is not

- Not a fork, clone or reskin of Betaflight Configurator or INAV Configurator.
- Not derived from any previous project of the author.
- Not a read-only inspector: the end product programs the aircraft and verifies the result.

## Independence

This is a standalone repository with its own codebase, architecture, tests and release
lifecycle. It does not depend on, import from, or share code with any other repository.

## Authoritative documents

| Document | Path |
| --- | --- |
| Foundational document v1.1 (single source of truth) | `docs/foundational/v1.1.md` |
| M0 Foundations Gate | `docs/m0/foundations-gate.md` |
| M0 Amendment | `docs/m0/amendment.md` |
| Final execution review | `docs/m0/final-execution-review.md` |
| Architecture decisions | `docs/adr/` |
| Source provenance policy | `provenance/README.md` |
| Hardware support matrix | `docs/hardware-support-matrix/README.md` |
| M1 simulation acceptance candidate | `docs/m1/README.md` |
| M2 Product Core / Web read-only boundary | `docs/m2/README.md` |
| M3 read-only capability-pack resolution | `docs/m3/README.md` |
| Web dependency audit and policy | `docs/m2/WEB-DEPENDENCY-AUDIT.md` |
| Approved Site v3 provenance boundary | `docs/m2/SMART-CONFIGURATOR-SITE-V3-PROVENANCE.md` |
| Binding product contract | `docs/product/PRODUCT-CONTRACT.md` |

## Licensing

See `NOTICE.md`. There is intentionally **no `LICENSE` file**: the final license is deferred
and this repository is not licensed for distribution or use.
