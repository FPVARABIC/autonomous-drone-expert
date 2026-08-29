# M1C — Android USB Serial Hardware Harness (SPIKE)

**Experiment. Not production. Never merge to `main`.**
**SPIKE — REQUIRES HARDWARE TEST — DO NOT USE FOR FLIGHT CONFIGURATION.**

An isolated Android app to run **read-only** USB-serial hardware checks (enumeration,
permission, open/close, read-timeout accuracy, unplug detection) so the phone + board
behaviour can be observed. It is the Android counterpart of the Windows M1B harness and,
like it, decides no production backend.

## Guarantees

- **NO PAYLOAD-BYTE WRITES BY CONSTRUCTION** — the transport interface has no write
  method; the only USB transfer is a bulk read on the IN endpoint. No MSP, no CLI, no
  DTR/RTS, no control transfer, no USB-OUT.
- **No permissions** in the manifest (USB host uses runtime permission): no INTERNET,
  Bluetooth, location, or storage. No background service. No telemetry.
- Opening a port is **not** assumed side-effect-free; the UI says so and asks the operator
  to record any reset/re-enumeration.
- No raw payload content is printed, logged, or stored — counts, timestamps,
  classifications and stage names only.

## Layout

| Path | Contents |
| --- | --- |
| `app/src/main/.../domain` | Android-independent core (transport contract, runner, report, safety gate) — JVM unit-tested |
| `app/src/main/.../platform` | Android USB (discovery, permission, read-only transport) |
| `app/src/main/.../ui` | Compose screens (Arabic-first RTL) + ViewModel |
| `app/src/test` | JVM unit tests + a clearly-marked FakeTransport |
| `scripts/` | payload-write guard (+ selftest), Android policy gate |
| `docs/` | version matrix, licenses, phone+board test protocol |

## Build / verify

CI (`.github/workflows/spike-android-usb-harness.yml`, Android branch only) runs policy
gates, unit tests, lint, `assembleDebug`, and packages a **private** debug APK artifact
(`m1c-android-usb-harness-<sha>`, 7-day retention) with `SHA256SUMS.txt` and
`BUILD-INFO.txt`. See `docs/HARDWARE-TEST-PROTOCOL.txt` to run it on a phone + board.

Pinned versions and license review: `docs/VERSIONS.md`.
