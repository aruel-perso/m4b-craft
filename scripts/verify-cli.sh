#!/usr/bin/env bash
set -euo pipefail
# Run at repository root. No external assets, installs, or original files modified.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
ffmpeg -v error -nostdin -f lavfi -i sine=frequency=440:duration=1 "$work/track2.wav"
ffmpeg -v error -nostdin -f lavfi -i sine=frequency=880:duration=1 "$work/track10.wav"
cargo run -p m4b-cli -- new --project "$work/book.json" --title 'CLI verification' --author 'm4b-Craft' "$work/track10.wav" "$work/track2.wav"
cargo run -p m4b-cli -- check "$work/book.json"
cargo run -p m4b-cli -- export "$work/book.json" "$work/book.m4b"
ffprobe -v error -show_format -show_chapters -of json "$work/book.m4b"
if cargo run -p m4b-cli -- export "$work/book.json" "$work/book.m4b"; then
  echo 'ERROR: overwrite was accepted' >&2
  exit 1
fi
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"verify","version":"1"}}}' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cargo run -p m4b-cli -- mcp
