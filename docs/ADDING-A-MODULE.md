# Adding an app to the Kvasir Hub

A module is one Rust crate plus one UI folder. The core never changes. Three places are touched:

| # | Where | What |
|---|-------|------|
| 1 | `crates/hub-mod-<app>/` | the Rust module (talks to the app, shapes its data) |
| 2 | `crates/hub-catalog/src/lib.rs` | one line in `modules()` and one field in `Endpoints` |
| 3 | `ui/src/modules/index.ts` | one entry in `MODULES`, pointing at `ui/src/modules/<app>/page.ts` |

If the app needs its own grant, its id must exist in `mod-identity` (`platform/crates/mod-identity`, was `kvasir-accounts`) (`App::as_str`); the hub-api
`/v1/me` answer lists the ids the account may open, and `Hub` only builds modules whose id is in that list.

## 1. The Rust crate

```toml
# crates/hub-mod-<app>/Cargo.toml
[dependencies]
hub-core = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

Depend on `hub-core` and nothing else of ours. Never import `ureq`, files or time: use the `HttpPort` you are given
(this is what makes the module testable with a fake and identical on desktop and Android).

```rust
pub fn definition(base_url: &str) -> ModuleDef {
    let base_url = base_url.to_owned();
    let factory: Factory = Box::new(move |http, credentials| Box::new(MyApp { /* built from http + credentials */ }));
    ModuleDef::new(Manifest { id: "myapp", title: "My app", summary: "One sentence for the home grid." }, factory)
}

impl Module for MyApp {
    fn call(&self, op: &str, args: &Value) -> Result<Value, HubError> { /* match op { ... } */ }
    // optional: fn upload(..)  (binary body)   fn media(..)  (audio, images, downloads; forward Range)
}
```

Rules that every module follows (see `hub-mod-music` and `hub-mod-files` as models):

- One file hides the protocol (`subsonic.rs`, `seafile.rs`): URL building, auth, envelope, error mapping.
  Another hides the answer shapes (`model.rs`): raw serde structs in, UI-facing camelCase structs out, defaults for
  optional fields so one missing field never fails a whole list.
- Validate every argument at the boundary (`Args::str`, ids, paths: no `..`, names: no `/`) BEFORE any request.
- Map failures to `HubError` (`Denied`, `NotFound`, `Upstream`, `Unavailable`); never return strings as errors.
- Tests with a fake `HttpPort` and literal JSON captured from the real server. Check the call URL, the headers and the
  reshaped result.

## 2. Catalog

```rust
pub struct Endpoints { ..., pub myapp: String }
pub fn modules(e: &Endpoints) -> Vec<ModuleDef> { vec![..., hub_mod_myapp::definition(&e.myapp)] }
```

`hub-catalog`'s test checks that module ids are unique and are known Kvasir app ids.

## 3. The UI page

```ts
// ui/src/modules/myapp/api.ts   typed wrappers + decoders over call('myapp', op, args)
// ui/src/modules/myapp/page.ts  export const myappPage: Page = { routes: [{ pattern: '/myapp', render }] }
// ui/src/modules/index.ts       { id: 'myapp', icon: 'apps', path: '/myapp', page: myappPage, widget?: ... }
```

- Decode every answer with `core/decode.ts` (never cast JSON). Test the decoder with the literal JSON the core returns.
- Use the shared pieces: `loadInto` (loading / error + Retry), `stateBlock` (empty and error states), `confirmDialog`,
  `promptDialog`, `menu`, `toast`, `meter`, `core/format.ts`.
- An optional `widget` shows a live summary on Home; a failing widget simply does not appear.
- Mobile first; the shell provides bottom navigation (first 4 destinations), the side rail and the player.
- Strict CSP: no inline `style="..."` and no inline scripts. Put styling in `ui/styles/*.css` using the tokens.

## Checklist

- [ ] `cargo test -p hub-mod-<app>` green with literal server JSON
- [ ] a real-server probe with a throwaway account (record it in the plan's result log)
- [ ] `./check.sh` green (fmt, clippy -D warnings, tests, `tsc --noEmit`)
- [ ] Playwright pass at 360 px and 1280 px, no console errors, no horizontal scroll
