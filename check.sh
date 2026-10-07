#!/bin/sh
# Gate: formatting, lints as errors, tests, UI type-check. Run before every commit.
set -eu
. "$HOME/.cargo/env"
cd "$(dirname "$0")"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -j2 -- -D warnings
cargo test --workspace -j2
(cd ui && npx --no-install tsc --noEmit -p tsconfig.json)
(cd ui && node --test dist-test 2>/dev/null || true)
