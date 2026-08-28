# `web/` — canonical Web/PWA product package

This is the only approved root for the Web/PWA product package. Its exact npm 11.13.0
manifest and lock are machine-governed by `policy/web-dependencies.json`; dependency or
lock changes require a fresh audit and explicit digest update.

The approved UI source-integration gate provides this layout:

```text
web/
├── public/
├── src/
├── tests/
├── package.json
├── package-lock.json
├── tsconfig.json
└── vite.config.ts
```

`package.json` and `package-lock.json` arrived together after the complete closure, source,
integrity, license, install-script, native-binary and vulnerability review. The governing
package manager is `npm@11.13.0`; pnpm, Yarn, Bun, Deno, nested workspaces and packages
outside `web/` are rejected by `scripts/check_web_dependencies.py`.

## Product boundary

- TypeScript/React is the Web shell; Rust remains the deterministic product core.
- UI code consumes product contracts. It does not build protocol frames or own write
  authority.
- IndexedDB remains the only browser persistence authority. The storage-only WASM host driver
  executes Rust-emitted load/CAS directives but cannot create journal bytes or accept state.
- Web Serial is limited to the Rust-owned read-only identity sequence and, for one exact reviewed
  API 1.46 identity, one typed beeper snapshot read. It exposes no generic command or write path.
- Android is a development-validation wrapper only; it has no native flight-controller USB
  authority.
- The visible USB chooser requires an explicit user gesture and retains the selected port only in
  memory. It never enumerates grants, reads USB metadata or claims hardware validation.

The approved Site v3 is a design/provenance reference, not a package template. Its hosting,
database, server-component and platform scaffolding remain prohibited.
