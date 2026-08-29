#!/usr/bin/env python3
"""Build and serve the owner-controlled physical FC read-only harness."""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Final, Mapping

ROOT: Final = Path(__file__).resolve().parent.parent
BIND_ADDRESS: Final = "127.0.0.1"
PORT: Final = 8765
URL: Final = f"http://{BIND_ADDRESS}:{PORT}/"
GLUE_DIR: Final = ROOT / "target" / "physical-fc-readonly-harness"
WASM_INPUT: Final = (
    ROOT
    / "target"
    / "wasm32-unknown-unknown"
    / "release"
    / "ade_web_readonly_serial_wasm_bridge.wasm"
)


@dataclass(frozen=True)
class Route:
    path: Path
    content_type: str


def build_commands() -> tuple[tuple[str, ...], ...]:
    """Return the exact locked build commands used by the owner launcher."""
    return (
        (
            "cargo",
            "+1.85.0",
            "build",
            "--locked",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "ade-web-readonly-serial-wasm-bridge",
        ),
        (
            "cargo",
            "run",
            "--locked",
            "--manifest-path",
            "tools/wasm-bindgen-cli-support/Cargo.toml",
            "--",
            str(WASM_INPUT),
            str(GLUE_DIR),
        ),
    )


def route_files() -> Mapping[str, Route]:
    """Return the complete, explicit HTTP allowlist."""
    manual = ROOT / "web" / "tests" / "manual"
    return {
        "/": Route(manual / "physical-fc-readonly.html", "text/html; charset=utf-8"),
        "/physical-fc-readonly.mjs": Route(
            manual / "physical-fc-readonly.mjs", "text/javascript; charset=utf-8"
        ),
        "/webserial-readonly-host.mjs": Route(
            ROOT / "web" / "src" / "transport" / "webserial-readonly-host.mjs",
            "text/javascript; charset=utf-8",
        ),
        "/wasm/ade_web_readonly_serial_wasm_bridge.js": Route(
            GLUE_DIR / "ade_web_readonly_serial_wasm_bridge.js",
            "text/javascript; charset=utf-8",
        ),
        "/wasm/ade_web_readonly_serial_wasm_bridge_bg.wasm": Route(
            GLUE_DIR / "ade_web_readonly_serial_wasm_bridge_bg.wasm",
            "application/wasm",
        ),
    }


def build() -> None:
    """Build the MSRV bridge and generate glue with the audited isolated tool."""
    GLUE_DIR.mkdir(parents=True, exist_ok=True)
    for command in build_commands():
        subprocess.run(command, cwd=ROOT, check=True)


def require_routes(routes: Mapping[str, Route]) -> None:
    missing = [str(route.path) for route in routes.values() if not route.path.is_file()]
    if missing:
        raise FileNotFoundError("manual harness route is missing: " + ", ".join(missing))


def handler_for(
    routes: Mapping[str, Route], allowed_authority: str | None = None
) -> type[BaseHTTPRequestHandler]:
    """Create a no-listing, allowlist-only localhost handler."""

    class HarnessHandler(BaseHTTPRequestHandler):
        server_version = "AdeReadonlyHarness/1"

        def _serve(self, include_body: bool) -> None:
            if self.client_address[0] != BIND_ADDRESS:
                self.send_error(403)
                return
            if allowed_authority is not None and self.headers.get("Host") != allowed_authority:
                self.send_error(421)
                return
            route = routes.get(self.path)
            if route is None:
                self.send_error(404)
                return
            payload = route.path.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", route.content_type)
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Referrer-Policy", "no-referrer")
            self.send_header(
                "Content-Security-Policy",
                "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; "
                "style-src 'unsafe-inline'; base-uri 'none'; frame-ancestors 'none'",
            )
            self.end_headers()
            if include_body:
                self.wfile.write(payload)

        def do_GET(self) -> None:  # noqa: N802 - stdlib handler API
            self._serve(include_body=True)

        def do_HEAD(self) -> None:  # noqa: N802 - stdlib handler API
            self._serve(include_body=False)

        def log_message(self, format: str, *args: object) -> None:
            return

    return HarnessHandler


def main() -> int:
    print("Building the exact read-only Web Serial Rust bridge...")
    build()
    routes = route_files()
    require_routes(routes)
    server = ThreadingHTTPServer(
        (BIND_ADDRESS, PORT), handler_for(routes, f"{BIND_ADDRESS}:{PORT}")
    )
    print(f"Manual harness ready at {URL}")
    print("Open this URL in a Chromium browser with Web Serial support.")
    print("Press Ctrl+C after the owner-controlled observation.")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nManual harness stopped.")
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
