#!/bin/sh
# Optional host-side setup. Run this from an ordinary terminal, not snap run.
set -eu

if [ -n "${SNAP:-}" ]; then
    echo 'Run this helper from your normal desktop terminal, outside snap run.' >&2
    exit 1
fi
if [ "$#" -gt 1 ]; then
    echo 'Usage: register-mime.sh [/snap/thermalprint/current]' >&2
    exit 1
fi
snap_root=${1:-/snap/thermalprint/current}
snap_instance=$(basename "$(dirname "$snap_root")")
case "$snap_instance" in
    thermalprint|thermalprint_*) ;;
    *) echo 'Expected a thermalprint Snap installation directory.' >&2; exit 1 ;;
esac
case "$snap_instance" in
    *[!a-z0-9_-]*) echo 'Invalid Snap instance name.' >&2; exit 1 ;;
esac
mime_source="$snap_root/usr/share/mime/packages/org.thermalprint.ThermalPrint.xml"
if [ ! -r "$mime_source" ]; then
    echo "Cannot find the ThermalPrint MIME definition: $mime_source" >&2
    exit 1
fi
for utility in update-mime-database xdg-mime; do
    if ! command -v "$utility" >/dev/null 2>&1; then
        echo "Install shared-mime-info and xdg-utils first (missing $utility)." >&2
        exit 1
    fi
done
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
mkdir -p "$data_home/mime/packages"
cp "$mime_source" "$data_home/mime/packages/org.thermalprint.ThermalPrint.xml"
update-mime-database "$data_home/mime"
xdg-mime default "${snap_instance}_thermalprint.desktop" application/x-thermalprint
echo 'Saved .thermalprint labels are now associated with ThermalPrint Snap.'
