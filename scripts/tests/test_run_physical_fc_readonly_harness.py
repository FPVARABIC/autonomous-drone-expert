from __future__ import annotations

import unittest
from http.server import ThreadingHTTPServer
from pathlib import Path
from tempfile import TemporaryDirectory
from threading import Thread
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from scripts import run_physical_fc_readonly_harness as HARNESS

ROOT = Path(__file__).resolve().parents[2]


class PhysicalFcReadonlyHarnessTests(unittest.TestCase):
    def test_build_commands_are_exact_locked_and_dependency_free(self) -> None:
        commands = HARNESS.build_commands()
        self.assertEqual(
            commands[0],
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
        )
        self.assertEqual(commands[1][:5], ("cargo", "run", "--locked", "--manifest-path", "tools/wasm-bindgen-cli-support/Cargo.toml"))
        self.assertEqual(commands[1][5], "--")
        self.assertEqual(Path(commands[1][6]), HARNESS.WASM_INPUT)
        self.assertEqual(Path(commands[1][7]), HARNESS.GLUE_DIR)
        self.assertNotIn("npm", " ".join(part for command in commands for part in command))

    def test_http_routes_are_a_complete_explicit_allowlist(self) -> None:
        routes = HARNESS.route_files()
        self.assertEqual(
            set(routes),
            {
                "/",
                "/physical-fc-readonly.mjs",
                "/webserial-readonly-host.mjs",
                "/wasm/ade_web_readonly_serial_wasm_bridge.js",
                "/wasm/ade_web_readonly_serial_wasm_bridge_bg.wasm",
            },
        )
        for route in routes.values():
            self.assertTrue(route.path.is_relative_to(ROOT))
        self.assertEqual(HARNESS.BIND_ADDRESS, "127.0.0.1")
        self.assertEqual(HARNESS.PORT, 8765)
        self.assertEqual(HARNESS.URL, "http://127.0.0.1:8765/")

    def test_static_source_routes_exist_before_generated_glue(self) -> None:
        routes = HARNESS.route_files()
        for key in ("/", "/physical-fc-readonly.mjs", "/webserial-readonly-host.mjs"):
            self.assertTrue(routes[key].path.is_file(), key)

    def test_handler_serves_only_allowlisted_local_files_with_security_headers(self) -> None:
        with TemporaryDirectory() as temporary:
            payload = Path(temporary) / "index.html"
            payload.write_text("manual harness", encoding="utf-8")
            routes = {"/": HARNESS.Route(payload, "text/plain; charset=utf-8")}
            server = ThreadingHTTPServer(("127.0.0.1", 0), HARNESS.handler_for(routes))
            server.RequestHandlerClass = HARNESS.handler_for(
                routes, f"127.0.0.1:{server.server_port}"
            )
            thread = Thread(target=server.serve_forever, daemon=True)
            thread.start()
            base = f"http://127.0.0.1:{server.server_port}"
            try:
                with urlopen(base + "/", timeout=5) as response:
                    self.assertEqual(response.read(), b"manual harness")
                    self.assertEqual(response.headers["Cache-Control"], "no-store")
                    self.assertIn("default-src 'none'", response.headers["Content-Security-Policy"])
                    self.assertIn("'wasm-unsafe-eval'", response.headers["Content-Security-Policy"])
                with urlopen(Request(base + "/", method="HEAD"), timeout=5) as response:
                    self.assertEqual(response.read(), b"")
                with self.assertRaises(HTTPError) as missing:
                    urlopen(base + "/not-allowlisted", timeout=5)
                self.assertEqual(missing.exception.code, 404)
                hostile_host = Request(base + "/", headers={"Host": "example.invalid"})
                with self.assertRaises(HTTPError) as rejected_host:
                    urlopen(hostile_host, timeout=5)
                self.assertEqual(rejected_host.exception.code, 421)
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)


if __name__ == "__main__":
    unittest.main()
