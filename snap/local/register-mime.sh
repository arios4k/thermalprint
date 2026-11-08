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
# The file icon. Desktop themes such as Zorin and Adwaita define their own generic file icon and
# the first theme in the chain that has any matching name wins, so an icon in hicolor alone loses.
# Put the icon into your user folder for hicolor and for every theme in your current theme's chain.
icon_name=application-x-thermalprint
icon_source="$snap_root/usr/share/icons/hicolor"
install_icon() {
    theme=$1
    for icon in scalable/mimetypes/$icon_name.svg 256x256/mimetypes/$icon_name.png; do
        [ -r "$icon_source/$icon" ] || continue
        directory=$(dirname "$icon")
        # Only themes that list this folder will look in it.
        if [ "$theme" != hicolor ]; then
            index=
            for base in /usr/share/icons "$data_home/icons"; do
                [ -r "$base/$theme/index.theme" ] && index="$base/$theme/index.theme" && break
            done
            [ -n "$index" ] && grep -q "^\[$directory\]" "$index" || continue
        fi
        mkdir -p "$data_home/icons/$theme/$directory"
        cp "$icon_source/$icon" "$data_home/icons/$theme/$icon"
    done
}
theme_chain() {
    seen=" $1 "
    queue=$1
    while [ -n "$queue" ]; do
        current=${queue%% *}
        queue=${queue#"$current"}
        queue=${queue# }
        echo "$current"
        for base in /usr/share/icons "$data_home/icons"; do
            if [ -r "$base/$current/index.theme" ]; then
                parents=$(sed -n 's/^Inherits=//p' "$base/$current/index.theme" | head -n 1 | tr ',' ' ')
                for parent in $parents; do
                    case "$seen" in *" $parent "*) ;; *) seen="$seen$parent "; queue="$queue $parent" ;; esac
                done
                break
            fi
        done
    done
}
current_theme=$(gsettings get org.gnome.desktop.interface icon-theme 2>/dev/null | tr -d "'" || true)
for theme in hicolor $( [ -n "$current_theme" ] && theme_chain "$current_theme" ); do
    case "$theme" in *[!A-Za-z0-9_.-]*|"") continue ;; esac
    install_icon "$theme"
done
# A cache written without a theme index would hide every other icon in this folder.
if [ -f "$data_home/icons/hicolor/icon-theme.cache" ] && [ ! -f "$data_home/icons/hicolor/index.theme" ]; then
    rm -f "$data_home/icons/hicolor/icon-theme.cache"
fi
xdg-mime default "${snap_instance}_thermalprint.desktop" application/x-thermalprint
echo 'Saved .thermalprint labels are now associated with ThermalPrint Snap.'
