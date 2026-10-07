#!/bin/sh
# Gate: formatting, lints as errors, tests, UI type-check. Run before every commit.
set -eu
. "$HOME/.cargo/env"
cd "$(dirname "$0")"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -j2 -- -D warnings
cargo test --workspace -j2
(cd ui && node_modules/.bin/tsc --noEmit -p tsconfig.json)
(cd ui && node --test src/core/*.test.ts src/modules/*/*.test.ts src/ui/*.test.ts)
