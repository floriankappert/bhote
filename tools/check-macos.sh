#!/usr/bin/env bash
# Type-checks the daemon for macOS on any machine (no Mac, no SDK needed): the only crate that needs a C compiler for the target (ureq, through ring) is
# replaced by a stub in a throw-away copy. Catches what breaks the macOS build (libc constants, missing items), not what breaks at run time.
# Needs: rustup target add aarch64-apple-darwin. Usage: bash tools/check-macos.sh
set -e
cd "$(dirname "$0")/../daemon"
rustup target list --installed 2>/dev/null | grep -q aarch64-apple-darwin || rustup target add aarch64-apple-darwin
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
mkdir -p "$T/stub/src"; cp -r src Cargo.toml "$T/"
printf '[package]\nname = "ureq"\nversion = "2.0.0"\nedition = "2021"\n' > "$T/stub/Cargo.toml"
cat > "$T/stub/src/lib.rs" <<'RS'
use std::time::Duration;
pub struct Req; pub struct Resp;
pub fn get(_: &str) -> Req { Req }
impl Req { pub fn set(self, _: &str, _: &str) -> Self { self } pub fn timeout(self, _: Duration) -> Self { self } pub fn call(self) -> Result<Resp, String> { Err("stub".into()) } }
impl Resp { pub fn into_string(self) -> Result<String, String> { Err("stub".into()) } pub fn into_reader(self) -> std::io::Empty { std::io::empty() } }
RS
sed -i 's|^ureq = .*|ureq = { path = "stub" }|' "$T/Cargo.toml"
(cd "$T" && cargo check --target aarch64-apple-darwin 2>&1 | grep -E '^error' -A12 | head -60; test "${PIPESTATUS[0]}" = 0) && echo "ok: the daemon type-checks for macOS"
