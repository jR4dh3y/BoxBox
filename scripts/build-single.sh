#!/usr/bin/env bash

set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend_dir="$repo_dir/frontend"
embedded_dir="$repo_dir/backend/internal/static/dist"
output_dir="$repo_dir/dist"
binary_name="boxbox"

if [[ "$(go env GOOS)" == "windows" ]]; then
	binary_name+=".exe"
fi

echo "Building frontend with Bun..."
bun run --cwd "$frontend_dir" build

echo "Staging frontend for go:embed..."
find "$embedded_dir" -mindepth 1 ! -name .gitkeep ! -name .gitignore -delete
cp -R "$frontend_dir/build/." "$embedded_dir/"

echo "Building self-contained Go binary..."
mkdir -p "$output_dir"
(
	cd "$repo_dir/backend"
	CGO_ENABLED=0 go build \
		-trimpath \
		-ldflags="-s -w" \
		-o "$output_dir/$binary_name" \
		./cmd/server
)

binary_size="$(du -h "$output_dir/$binary_name" | cut -f1)"
echo "Built $output_dir/$binary_name ($binary_size)"
