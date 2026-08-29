#!/usr/bin/env bash
# Pre-build guard for the M1C Android harness: refuse to build if MAIN source contains an
# actual payload-write / control-line / USB-OUT CALL.
#
# Matching rule: call syntax only, evaluated after stripping // and /* */ comments AND
# double-quoted string content, so prose and banner text (e.g. "no write") never trips
# it. This is the same lesson as the Windows guard fix: match calls, not words.
#
# Honest limitation: still a regex tripwire, not a Kotlin parser. It is a build backstop;
# the audited source, the read-only transport interface, and human review remain the
# authoritative control. Spike tooling only -- never promoted to main.
set -euo pipefail

# Directory holding MAIN (non-test) Kotlin sources.
MAIN_DIR="${1:-app/src/main}"

# Strip // line comments, /* */ block comments, and "..." string contents.
strip() {
  python3 - "$1" <<'PY'
import re, sys
src = open(sys.argv[1], encoding="utf-8").read()
src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)     # block comments
src = re.sub(r'//[^\n]*', '', src)                  # line comments
src = re.sub(r'"(?:\\.|[^"\\])*"', '""', src)       # double-quoted strings
sys.stdout.write(src)
PY
}

# Forbidden CALL patterns (method-call syntax) and forbidden direction/stream tokens.
PATTERN='\.(write|writeAsync|setDTR|setRTS|setDtr|setRts|purge|flush|discard[A-Za-z]*)[[:space:]]*\(|\.controlTransfer[[:space:]]*\(|outputStream|USB_DIR_OUT'

scan() {
  local dir="$1" hit=0
  while IFS= read -r -d '' f; do
    if strip "$f" | grep -nE "$PATTERN" >/dev/null; then
      echo "  offending calls in $f:"
      strip "$f" | grep -nE "$PATTERN" | sed 's/^/    /'
      hit=1
    fi
  done < <(find "$dir" -name '*.kt' -print0 2>/dev/null)
  return $hit
}

cmd="${1:-}"
case "$cmd" in
  check)
    dir="${2:-app/src/main}"
    if scan "$dir"; then
      echo "guard: clean -- no payload-write / control-line / USB-OUT calls in $dir"
    else
      echo "guard: forbidden call found in $dir -- refusing to build"
      exit 1
    fi
    ;;
  selftest)
    dir="${2:-app/src/main}"
    root="$(mktemp -d)"; trap 'rm -rf "$root"' EXIT
    mk() { mkdir -p "$root/$1"; printf '%s\n' "$2" > "$root/$1/f.kt"; }

    # 1. real main source passes
    if scan "$dir" >/dev/null; then echo "selftest 1: real main source passes ......... OK"; else
      echo "selftest 1 FAILED: real main source rejected"; exit 1; fi

    # 2. port.write(data) fails
    mk c2 'fun f(p: P, d: ByteArray) { p.write(d) }'
    if scan "$root/c2" >/dev/null; then echo "selftest 2 FAILED: p.write(d) accepted"; exit 1; else
      echo "selftest 2: p.write(data) fails ............. OK"; fi

    # 3. banner text containing "no write" passes (string content is stripped)
    mk c3 'const val B = "This tool performs no write/flush and no USB_DIR_OUT transfer"'
    if scan "$root/c3" >/dev/null; then echo "selftest 3: banner-style string passes ...... OK"; else
      echo "selftest 3 FAILED: banner string rejected"; exit 1; fi

    # 4. port.setDTR(true) fails
    mk c4 'fun f(p: P) { p.setDTR(true) }'
    if scan "$root/c4" >/dev/null; then echo "selftest 4 FAILED: p.setDTR(true) accepted"; exit 1; else
      echo "selftest 4: p.setDTR(true) fails ............ OK"; fi

    # 5. synthetic USB OUT payload transfer fails
    mk c5 'val dir = UsbConstants.USB_DIR_OUT'
    if scan "$root/c5" >/dev/null; then echo "selftest 5 FAILED: USB OUT transfer accepted"; exit 1; else
      echo "selftest 5: USB OUT payload transfer fails .. OK"; fi

    echo "guard selftest: 5/5 OK"
    ;;
  *)
    echo "usage: $0 {check|selftest} [main-dir]"; exit 2 ;;
esac
