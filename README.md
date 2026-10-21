# Kvasir Hub

One desktop + mobile app for every Kvasir app. Rust core, web UI, one module per app.
Design notes live outside this repo (private planning repo); the module recipe is in `docs/ADDING-A-MODULE.md`.

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

Server side: `crates/hub-api` (systemd `hub-api`, `https://<your-domain>/hub/v1/me`).

## Commands
```sh
./check.sh                         # fmt, clippy -D warnings, all tests, tsc --noEmit, UI tests
(cd ui && npm ci --include=dev && node build.mjs)          # build the UI into ui/dist (check.sh does this too)
cargo run -p hub-web -- --secret-file /tmp/hub-secret.json # dev host on 127.0.0.1:4560 (real servers)
cd shell-tauri && cargo tauri build                        # desktop bundle
cd shell-tauri && cargo tauri android build                # Android APK
```
Add an app: `docs/ADDING-A-MODULE.md`.

## Status
Early (0.1.0). Modules today: Files, Music, Anki. The Android build is a signed APK published as a GitHub
Release asset, never committed. The server side (`hub-api`) expects an account server that answers
`/hub/v1/me`; that is `app-portal` in [kvasir-platform](../kvasir-platform).

## Default servers
The Hub has no built-in server addresses. Set your domain (for example `example.org`) with `KVASIR_DOMAIN` at build time or `--domain` for `hub-web`; the app can also read it from `site.txt` in its data folder. Servers are `<app>.<domain>`; see `crates/hub-catalog`.

## Licence

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
