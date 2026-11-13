#!/bin/sh
# Capture actual app screens with fictional demo content and isolated preferences.
set -eu
cd "$(dirname "$0")/.."
output_dir=$(realpath -m "${1:-docs/screenshots}")
mkdir -p "$output_dir"
preview_dir=$(mktemp -d /tmp/thermalprint-docs.XXXXXX)
trap 'rm -rf "$preview_dir"' EXIT HUP INT TERM
cargo build --locked
for style in light dark; do
    env THERMALPRINT_SCREENSHOTS="$output_dir" \
        THERMALPRINT_SCREENSHOTS_STYLE="$style" \
        XDG_CONFIG_HOME="$preview_dir/config" \
        XDG_DATA_HOME="$preview_dir/data" \
        THERMALPRINT_SCREENSHOTS_SIZE=1440x810 \
        target/debug/thermalprint
done
