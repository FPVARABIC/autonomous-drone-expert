#!/usr/bin/env python3
"""M1C Android policy gate. Standard library only.

Enforces the Phase-4 safety and privacy limits by inspection:
  * AndroidManifest declares NO permissions at all (no INTERNET / Bluetooth / location /
    storage), requires the USB host feature, and exports only the launcher activity.
  * No raw-byte-content rendering in main sources (heuristic tripwire).

It complements guard_no_payload_writes.sh (which forbids write/DTR/RTS/USB-OUT calls).
Both are backstops; audited source and review remain the control.
"""
from __future__ import annotations

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ANDROID = "{http://schemas.android.com/apk/res/android}"
errors: list[str] = []


def check_manifest() -> None:
    path = ROOT / "app" / "src" / "main" / "AndroidManifest.xml"
    if not path.is_file():
        errors.append(f"missing {path}")
        return
    tree = ET.parse(path)
    root = tree.getroot()

    perms = [e.get(f"{ANDROID}name") for e in root.findall("uses-permission")]
    perms += [e.get(f"{ANDROID}name") for e in root.findall("uses-permission-sdk-23")]
    if perms:
        errors.append(f"AndroidManifest declares permissions, expected none: {perms}")

    feats = [e.get(f"{ANDROID}name") for e in root.findall("uses-feature")]
    if "android.hardware.usb.host" not in feats:
        errors.append("AndroidManifest does not require android.hardware.usb.host")

    app = root.find("application")
    if app is None:
        errors.append("AndroidManifest has no <application>")
        return
    if app.get(f"{ANDROID}allowBackup") not in ("false",):
        errors.append("application allowBackup must be false")

    exported = []
    for tag in ("activity", "service", "receiver", "provider"):
        for comp in app.findall(tag):
            name = comp.get(f"{ANDROID}name")
            is_exported = comp.get(f"{ANDROID}exported")
            if is_exported == "true":
                exported.append((tag, name))
            if tag in ("service", "receiver", "provider"):
                errors.append(f"unexpected component <{tag} {name}> (spike must have none)")
    if [c for c in exported] != [("activity", ".MainActivity")]:
        errors.append(f"exported components must be exactly the launcher activity; got {exported}")


RAW_BYTE_PATTERNS = [
    (r"toHexString\s*\(", "toHexString("),
    (r"%02[xX]", "%02x hex format"),
    (r"\.contentToString\s*\(", ".contentToString("),
    (r"encodeToString\s*\(", "encodeToString( (base64)"),
    (r"\bBase64\b", "Base64"),
]


def check_no_raw_byte_rendering() -> None:
    main = ROOT / "app" / "src" / "main"
    for kt in sorted(main.rglob("*.kt")):
        text = kt.read_text(encoding="utf-8")
        # strip // and /* */ comments and "..." strings so prose never trips it
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
        text = re.sub(r"//[^\n]*", "", text)
        text = re.sub(r'"(?:\\.|[^"\\])*"', '""', text)
        for pat, label in RAW_BYTE_PATTERNS:
            if re.search(pat, text):
                errors.append(f"{kt.relative_to(ROOT)}: raw-byte-content pattern '{label}'")


def main() -> int:
    check_manifest()
    check_no_raw_byte_rendering()
    if errors:
        print("ANDROID POLICY GATE FAILED\n")
        for e in errors:
            print(f"  - {e}")
        return 1
    print("android policy gate passed (no permissions, usb-host required, launcher-only "
          "exported, no raw-byte rendering)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
