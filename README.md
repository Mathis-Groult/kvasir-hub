# Kvasir Hub

One desktop + mobile app for every Kvasir app. Rust core, web UI, one module per app.
Plan: `~/plans/kvasir/2026-10-07-kvasir-hub-app-core-and-modules.md`. Decisions: `docs/decisions/0013`, `0014` (in `~/Kvasir/docs`).

```
ui/ (TypeScript, strict)        shell-tauri/ (desktop + Android)     crates/hub-web (dev + tests)
        \                                  |                                  /
         `------------- hub-rpc: commands, media URLs, uploads -------------'
                                           |
                                      hub-catalog  <- the only place that lists modules and servers
                         ___________________|___________________
                  hub-mod-files   hub-mod-music   (one crate per app)
                         \\____________  |  ____________/
                                      hub-core  (Module trait, Hub facade, ports, errors; no IO)
                  hub-http (ureq)   hub-store-file (0600)   hub-directory (sign-in via hub-api)
```

Server side: `crates/hub-api` (systemd `hub-api`, `https://example.org/hub/v1/me`).

## Commands
```sh
./check.sh                         # fmt, clippy -D warnings, all tests, tsc --noEmit, UI tests
(cd ui && npm install --include=dev && node build.mjs)     # build the UI into ui/dist
cargo run -p hub-web -- --secret-file /tmp/hub-secret.json # dev host on 127.0.0.1:4560 (real servers)
cd shell-tauri && cargo tauri build                        # desktop bundle
cd shell-tauri && cargo tauri android build                # Android APK
```
Add an app: `docs/ADDING-A-MODULE.md`.
