#!/usr/bin/env bash
# Builds a runnable bundle in dist/desktop-mygo: the app, the BoxBox server,
# and the media thumbnail worker with its Rust library.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$repo/dist/desktop-mygo"
mkdir -p "$out"

if [[ "${SKIP_SERVER:-}" != 1 ]]; then
	(cd "$repo" && bun run build:single)
fi
install -m 755 "$repo/dist/boxbox" "$out/boxbox-server"

cargo build --release --locked --manifest-path "$repo/desktop/native/Cargo.toml"
install -m 644 "$repo/desktop/native/target/release/libboxbox_thumbnail.so" "$out/"

cd "$repo/desktop-mygo"
go build -trimpath -ldflags="-s -w" -o "$out/boxbox-mygo" .
go build -trimpath -ldflags="-s -w" -o "$out/boxbox-thumbnail" ./cmd/boxbox-thumbnail
echo "Built $out"
