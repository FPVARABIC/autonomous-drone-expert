# M2 physical FC read-only observation harness

This owner-controlled engineering page exercises the accepted production
`WebSerialReadonlyHost` and Rust WASM discovery path. It is not part of the Smart Configurator
React bundle and does not add product or write authority.

The available board is labelled `SPEEDYBEE AIO405`. That label is not a Betaflight target and
must not be used to infer one. Only the four typed identity responses may establish what the FC
reports. A scope mismatch is an honest, valid transport-and-identity result; it is not product
hardware support.

## Before connecting

1. **REMOVE ALL PROPELLERS.**
2. **DISCONNECT THE LIPO BATTERY.**
3. Use FC USB only.
4. Do not press BOOT.
5. Do not enter DFU mode.
6. Do not flash firmware.
7. Close Betaflight Configurator and SpeedyBee App.
8. Ensure no other program owns the serial port.
9. Do not connect an external USB-UART/FTDI adapter.
10. Do not connect battery power during this first observation.

The first physical run is USB-only. The page is explicitly:

- **READ-ONLY IDENTIFICATION TEST**
- **NO CONFIGURATION CHANGES**
- **NO SAVE**
- **NO REBOOT**
- **NO MOTOR COMMANDS**

## Launch

From the repository root, run exactly:

```powershell
python scripts/run_physical_fc_readonly_harness.py
```

The launcher builds the read-only serial bridge with Rust 1.85.0, generates Web glue through the
locked first-party `wasm-bindgen-cli-support` tool, and serves an explicit file allowlist using only
the Python standard library. It binds only to `127.0.0.1` and does not require an npm package,
global web server, CDN, analytics service, or external runtime network.

Open this exact URL in a Chromium browser with Web Serial support:

```text
http://127.0.0.1:8765/
```

## Owner-controlled observation

1. Complete the safety checklist above.
2. Connect the FC by USB only.
3. Open the localhost URL.
4. Click **Select FC and Read Identity** once.
5. Select only the intended FC in the browser-owned chooser.
6. Record only the displayed typed fields: `outcome`, `apiVersion`, `fcVariant`, `fcVersion`,
   `targetName`, and, when present, `scopeMismatchField` or `failure`.
7. Do not record or share serial number, COM path, USB identity, VID/PID, unique board signature,
   permission metadata, or raw serial frames.
8. Stop the launcher with Ctrl+C after the result is recorded.

There is no automatic port enumeration, retry, reconnect, baud scan, fallback hardware API, or
firmware repair. Cancellation and every malformed, mismatched, oversized, timed-out, disconnected,
or cleanup-failed path stop fail-closed through the existing production host.

If the four responses parse but the existing scope check rejects the reported identity, classify
the owner-provided result later as:

```text
PHYSICAL_TRANSPORT_AND_IDENTITY_OBSERVED
SCOPE_NOT_ACCEPTED
```

Do not change the proposed target or capability status in response to this run. Until the owner
returns the manual result and a later evidence commit is reviewed, status remains:

```text
PHYSICAL_FC_NOT_TESTED
HARDWARE_OBSERVED=NO
```
