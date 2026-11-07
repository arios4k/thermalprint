#!/bin/sh
# Build from a clean source snapshot; do not copy existing Cargo build outputs.
set -eu
if [ "$#" -gt 1 ]; then
    echo 'Usage: build-snap.sh [output-directory]' >&2
    exit 1
fi
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_dir=${1:-"$project_root/target/snap"}
mkdir -p "$output_dir" "$project_root/target"
output_dir=$(CDPATH= cd -- "$output_dir" && pwd)
build_source=$(mktemp -d "$project_root/target/snap-source.XXXXXX")
python3 - "$project_root" "$build_source" <<'PY'
from pathlib import Path
import shutil
import sys
root, destination = map(Path, sys.argv[1:])
for name in ('Cargo.toml', 'Cargo.lock', 'LICENSE', 'README.md', 'src', 'tests',
             'assets', 'data', 'docs', 'snap'):
    source = root / name
    if source.is_dir():
        shutil.copytree(source, destination / name)
    else:
        shutil.copy2(source, destination / name)
PY
printf 'Snap source snapshot: %s\n' "$build_source"
# Keep the build tree for inspecting logs and resuming interrupted builds.
cd "$build_source"
snapcraft pack --use-lxd
mv ./*.snap "$output_dir/"
