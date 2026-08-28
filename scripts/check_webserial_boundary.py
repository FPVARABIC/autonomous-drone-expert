#!/usr/bin/env python3
"""Fail-closed Web Serial read-only authority and privacy gate (ADR-0013)."""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path, PurePosixPath

import verify_webserial_product_assets

ROOT = Path(__file__).resolve().parent.parent
ADAPTER = PurePosixPath("web/src/transport/webserial-readonly-host.mjs")
DECLARATION = PurePosixPath("web/src/transport/webserial-readonly-host.d.mts")
FACADE = PurePosixPath("web/src/connection/readonly-fc-connection.mjs")
FACADE_DECLARATION = PurePosixPath("web/src/connection/readonly-fc-connection.d.mts")
DIAGNOSTIC_TRACE = PurePosixPath("web/src/diagnostics/readonly-trace.mjs")
DIAGNOSTIC_TRACE_DECLARATION = PurePosixPath("web/src/diagnostics/readonly-trace.d.mts")
APP = PurePosixPath("web/src/App.tsx")
DERIVED_ASSETS = {
    PurePosixPath("web/public/wasm/ade_web_readonly_serial_wasm_bridge.js"),
    PurePosixPath("web/public/wasm/ade_web_readonly_serial_wasm_bridge_bg.wasm"),
}
SERIAL_BRIDGE = PurePosixPath("crates/web-readonly-serial-wasm-bridge/src/lib.rs")
STORAGE_BRIDGE = PurePosixPath("crates/web-storage-wasm-bridge/src/lib.rs")
EXPECTED_PACKAGE_LOCK_SHA256 = (
    "c3015e9454da094d307975921b8aa2c195a15b9dffe0498a9c758b57d922c05d"
)


def product_sources(root: Path = ROOT) -> dict[PurePosixPath, str]:
    files: dict[PurePosixPath, str] = {}
    for directory in (root / "web" / "src", root / "web" / "public"):
        if not directory.is_dir():
            continue
        for path in sorted(candidate for candidate in directory.rglob("*") if candidate.is_file()):
            relative = PurePosixPath(path.relative_to(root).as_posix())
            if relative in DERIVED_ASSETS:
                continue
            files[relative] = path.read_text(encoding="utf-8")
    return files


def source_authority_errors(files: dict[PurePosixPath, str]) -> list[str]:
    errors: list[str] = []
    adapter = files.get(ADAPTER)
    if adapter is None:
        return [f"missing designated Web Serial adapter: {ADAPTER}"]
    declaration = files.get(DECLARATION)
    if declaration is None:
        errors.append(f"missing first-party Web Serial declaration: {DECLARATION}")
    facade = files.get(FACADE)
    facade_declaration = files.get(FACADE_DECLARATION)
    diagnostic_trace = files.get(DIAGNOSTIC_TRACE)
    diagnostic_declaration = files.get(DIAGNOSTIC_TRACE_DECLARATION)
    app = files.get(APP)
    if facade is None:
        errors.append(f"missing product read-only connection facade: {FACADE}")
    if facade_declaration is None:
        errors.append(f"missing product read-only connection declaration: {FACADE_DECLARATION}")
    if diagnostic_trace is None:
        errors.append(f"missing privacy-bounded diagnostic trace: {DIAGNOSTIC_TRACE}")
    if diagnostic_declaration is None:
        errors.append(
            f"missing privacy-bounded diagnostic trace declaration: {DIAGNOSTIC_TRACE_DECLARATION}"
        )
    if app is None:
        errors.append(f"missing approved React product surface: {APP}")

    serial_patterns = {
        "navigator.serial": re.compile(r"(?:globalThis\.)?navigator\?*\.serial"),
        "requestPort": re.compile(r"\brequestPort\b"),
        "writer.write": re.compile(r"#writer\.write\s*\("),
    }
    for label, pattern in serial_patterns.items():
        owners = [path for path, source in files.items() if pattern.search(source)]
        if owners != [ADAPTER]:
            errors.append(f"{label} authority must exist only in {ADAPTER}; found {owners}")

    if len(re.findall(r"\brequestPort\s*\(", adapter)) != 1:
        errors.append("requestPort must have exactly one call in the explicit selection boundary")
    if len(re.findall(r"#writer\.write\s*\(", adapter)) != 1:
        errors.append("writer.write must have exactly one call in the designated adapter")
    required = (
        "selectPortFromUserGesture",
        "open({ baudRate: INITIAL_MSP_BAUD_RATE })",
        "exchange-identification-read",
        "exchange-snapshot-read",
        "acceptReadChunk",
        "acceptExchangeFailure",
        "failureStage: discovery.failureStage ?? undefined",
        "failureReason: discovery.failureReason ?? undefined",
        "failureOrigin:",
        "takeTraceEvent",
        "releaseLock",
        "close",
    )
    for marker in required:
        if marker not in adapter:
            errors.append(f"designated adapter is missing required bounded behavior: {marker}")

    binding_module = "virtual:ade-web-readonly-serial-wasm"
    import_match = re.search(
        rf'import\s*\{{(?P<names>[^}}]+)\}}\s*from\s*["\']{re.escape(binding_module)}["\']',
        adapter,
        re.DOTALL,
    )
    imported_names = set()
    if import_match is not None:
        imported_names = {
            name.strip()
            for name in import_match.group("names").split(",")
            if name.strip()
        }
    required_bindings = {"WasmReadonlySerialDirective", "WasmReadonlySerialDiscovery"}
    if imported_names != required_bindings:
        errors.append("adapter must statically import only the exact generated Rust trust types")
    if len(re.findall(r"new\s+WasmReadonlySerialDiscovery\s*\(\s*\)", adapter)) != 1:
        errors.append("discover must internally create exactly one genuine Rust discovery")
    if len(re.findall(r"instanceof\s+WasmReadonlySerialDirective\b", adapter)) != 1:
        errors.append("directives must be checked against the exact generated Rust class")
    if len(re.findall(r"async\s+discover\s*\(\s*\)", adapter)) != 1:
        errors.append("public discover must accept no caller-supplied authority")

    substitutable_trust = (
        r"\brustDirectiveType\b",
        r"\b(?:setRustBindings|bindings|rustBindings|bindingFactory|bindingsFactory|discoveryFactory|directiveFactory)\b",
        r"\b(?:validator|trustCallback|directiveType|discoveryType)\b",
        r"\b(?:directiveConstructor|discoveryConstructor)\b",
        r"\bdiscover\s*\(\s*[A-Za-z_$]",
    )
    for pattern in substitutable_trust:
        if re.search(pattern, adapter):
            errors.append("adapter exposes caller-substitutable Rust trust authority")

    if declaration is not None:
        if not re.search(
            r"constructor\s*\(\s*options\?\s*:\s*\{\s*serial\?\s*:\s*object\s*;\s*timeoutMs\?\s*:\s*number\s*\}\s*\)",
            declaration,
        ):
            errors.append("public constructor may expose only serial and bounded timing options")
        if not re.search(r"\bdiscover\s*\(\s*\)\s*:", declaration):
            errors.append("public declaration must expose zero-argument discover()")
        for pattern in substitutable_trust:
            if re.search(pattern, declaration):
                errors.append("declaration exposes caller-substitutable Rust trust authority")

    if facade is not None:
        required_facade = (
            'import initReadonlySerialWasm from "virtual:ade-web-readonly-serial-wasm"',
            'import { WebSerialReadonlyHost } from "../transport/webserial-readonly-host.mjs"',
            "await initReadonlySerialWasm();",
            "const selection = await this.#host.selectPortFromUserGesture()",
            "await this.#host.discover()",
            "result.hardwareObserved !== false",
            "failureStage: result.failureStage",
            "failureReason: result.failureReason",
        )
        for marker in required_facade:
            if marker not in facade:
                errors.append(f"product connection facade missing trusted behavior: {marker}")
        if not re.search(r"async\s+discover\s*\(\s*\)", facade):
            errors.append("product facade discover must accept zero arguments")
        if re.search(r"export\s+class\s+PreparedReadonlyFcConnection", facade):
            errors.append("prepared host implementation must not be caller-constructible")
        if re.search(
            r"(?:globalThis\.)?navigator\?*\.serial|\brequestPort\b|\b(?:getPorts|getInfo)\s*\(",
            facade,
        ):
            errors.append("browser permission mechanics escaped from the accepted host")
        if re.search(
            r"\b(?:MSP_[A-Z0-9_]+|CommandId|WriteApproval|TransportEffect|OutboundPacket|"
            r"sendRaw|writeRaw|commandId|payload)\b",
            facade,
            re.IGNORECASE,
        ):
            errors.append("product connection facade gained command/payload/write authority")
        if re.search(r"\b(?:localStorage|sessionStorage|indexedDB)\b|document\.cookie", facade):
            errors.append("product connection facade must not persist device identity")
        if "/wasm/ade_web_readonly_serial_wasm_bridge" in facade:
            errors.append("product connection facade contains a root-absolute WASM assumption")

    if facade_declaration is not None:
        if not re.search(r"\bdiscover\s*\(\s*\)\s*:", facade_declaration):
            errors.append("product facade declaration must expose zero-argument discover()")
        if re.search(r"\b(?:constructor|serial\?|host\?|bindings|factory|command|payload)\b", facade_declaration, re.IGNORECASE):
            errors.append("product facade declaration exposes replaceable or selectable authority")

    if diagnostic_trace is not None:
        for marker in (
            "DIAGNOSTIC_TRACE_CAPACITY = 200",
            "capacity < 100 || capacity > 250",
            "ALLOWED_KEYS",
            "beginAttempt()",
            "snapshot()",
            "clear()",
            "formatSafeDiagnosticTrace",
        ):
            if marker not in diagnostic_trace:
                errors.append(f"diagnostic trace missing bounded behavior: {marker}")
        forbidden_diagnostic = (
            r"\b(?:console\.|localStorage|sessionStorage|indexedDB|document\.cookie)\b",
            r"\b(?:fetch|XMLHttpRequest|WebSocket|sendBeacon)\s*\(?",
            r"raw.?bytes|raw.?frame|raw.?payload|usbVendorId|usbProductId|serial.?number|port.?info",
        )
        for pattern in forbidden_diagnostic:
            if re.search(pattern, diagnostic_trace, re.IGNORECASE):
                errors.append("diagnostic trace gained a forbidden data field or outbound sink")

    if app is not None:
        if (
            'from "./connection/readonly-fc-connection.mjs"' not in app
            or "await prepared.selectPortFromUserGesture()" not in app
            or "await prepared.discover()" not in app
        ):
            errors.append("React must call only the prepared narrow read-only facade")
        forbidden_app = (
            r"webserial-readonly-host|WasmReadonlySerial|(?:globalThis\.)?navigator\?*\.serial",
            r"\b(?:requestPort|getPorts|getInfo)\s*\(",
            r"\b(?:MSP_[A-Z0-9_]+|CommandId|WriteApproval|TransportEffect|SerialPort|"
            r"sendRaw|writeRaw|commandId|payload)\b",
            r"\b(?:localStorage|sessionStorage|indexedDB)\b|document\.cookie",
            r"serial.?number|usbVendorId|usbProductId|deviceId|raw.?frame|raw.?payload|"
            r"board.?signature|\bUID\b|hardwareObserved",
        )
        for pattern in forbidden_app:
            if re.search(pattern, app, re.IGNORECASE):
                errors.append("React gained serial authority or privacy-sensitive device data")

    forbidden_adapter = (
        (r"\bgetPorts\s*\(", "automatic/granted port enumeration"),
        (r"\b(sendRaw|writeRaw|sendMsp|writeCommand|executeArbitraryBytes)\b", "raw API"),
        (r"\b(CommandId|WriteApproval|TransportEffect|OutboundPacket)\b", "Rust authority type"),
        (r"\bMSP_[A-Z0-9_]*|\b(?:buildMsp|decodeMsp|encodeMsp)\b", "JavaScript MSP semantics"),
        (r"\b(console\.|localStorage|sessionStorage|indexedDB)\b", "logging/persistence"),
    )
    for pattern, label in forbidden_adapter:
        if re.search(pattern, adapter, re.IGNORECASE):
            errors.append(f"designated adapter contains forbidden {label}")

    all_source = "\n".join(files.values())
    if re.search(r"navigator\?*\.(?:usb|hid)\b|\b(?:WebUSB|WebHID|USBDevice|HIDDevice)\b", all_source):
        errors.append("WebUSB/WebHID authority is forbidden")

    outside = "\n".join(source for path, source in files.items() if path != ADAPTER)
    if re.search(
        r"(?:globalThis\.)?navigator\?*\.serial|\brequestPort\b|#writer\.write\s*\(|"
        r"\b(?:SerialPort|WriteApproval|TransportEffect|CommandId)\b",
        outside,
    ):
        errors.append("serial/MSP/write authority escaped the designated adapter")

    for path, source in files.items():
        if path == ADAPTER:
            continue
        if path.suffix in {".tsx", ".jsx"} or path.name == "sw.js":
            if re.search(r"\bserial\b|requestPort|writer\.write", source, re.IGNORECASE):
                errors.append(f"UI/service-worker serial authority forbidden at {path}")
    return errors


def repository_errors(root: Path = ROOT) -> list[str]:
    errors = source_authority_errors(product_sources(root))
    adapter_path = root / ADAPTER
    declaration_path = root / DECLARATION
    if adapter_path.is_file() and hashlib.sha256(adapter_path.read_bytes()).hexdigest() != (
        "54b73f51a4f5c30e632e9cf8c76a7d1828d240c6af56a784372b079db0719903"
    ):
        errors.append("accepted production Web Serial host source drifted")
    if declaration_path.is_file() and hashlib.sha256(declaration_path.read_bytes()).hexdigest() != (
        "6006c87ab766a56b34d99a262373a4c799583f7e70332139ff96e2cb5387cabc"
    ):
        errors.append("accepted production Web Serial host declaration drifted")
    errors.extend(verify_webserial_product_assets.verify(root=root))
    for relative in (SERIAL_BRIDGE, STORAGE_BRIDGE):
        if not (root / relative).is_file():
            errors.append(f"missing bridge source: {relative}")
    if (root / SERIAL_BRIDGE).is_file():
        serial_bridge = (root / SERIAL_BRIDGE).read_text(encoding="utf-8")
        if "#![forbid(unsafe_code)]" not in serial_bridge:
            errors.append("read-only serial bridge must forbid first-party unsafe")
        for marker in (
            "WasmReadonlySerialDiscovery",
            "MspV1ResponseAccumulator",
            "ReadonlyBeeperSnapshotRead",
            "SessionState::SnapshotRead",
            "BeeperConfigSnapshot",
            "WriteCommandClass::NoWrite",
            "packet.approval().is_some()",
            "TRACE_EVENT_LIMIT",
            "take_trace_event",
        ):
            if marker not in serial_bridge:
                errors.append(f"Rust serial authority proof missing: {marker}")
        directive_impl = re.search(
            r"impl WasmReadonlySerialDirective\s*\{(?P<body>.*?)^\}",
            serial_bridge,
            re.DOTALL | re.MULTILINE,
        )
        if directive_impl is None or "constructor" in directive_impl.group("body"):
            errors.append("JavaScript-visible directive constructor is forbidden")
    if (root / STORAGE_BRIDGE).is_file():
        storage_bridge = (root / STORAGE_BRIDGE).read_text(encoding="utf-8")
        if re.search(r"\b(TransportEffect|OutboundPacket|WebSerial|requestPort)\b", storage_bridge):
            errors.append("storage-only WASM bridge gained transport authority")

    lock = root / "web" / "package-lock.json"
    if not lock.is_file():
        errors.append("web/package-lock.json is missing")
    elif hashlib.sha256(lock.read_bytes()).hexdigest() != EXPECTED_PACKAGE_LOCK_SHA256:
        errors.append("web/package-lock.json drifted from the owner-approved SHA-256")
    return errors


def main() -> int:
    errors = repository_errors()
    if errors:
        print("WEB SERIAL AUTHORITY GATE FAILED\n")
        for error in errors:
            print(f"  - {error}")
        return 1
    print("web serial read-only authority gate passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
