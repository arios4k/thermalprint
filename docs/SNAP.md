# Snap release preparation

[Back to the project](../README.md)

ThermalPrint's Snap uses `core24`, strict confinement, the GNOME
desktop/GPU extension, and the printing-only `cups` interface. The first target
is amd64. The recipe is `grade: stable`. The pull-request workflow only builds
and inspects the package; the [Release Snap](../.github/workflows/release-snap.yml)
workflow publishes to the Snap Store stable channel for version tags.

## Build and inspect

Install Snapcraft and initialize its LXD build provider using the
[Snapcraft setup guide](https://ubuntu.com/docs/snapcraft/9/how-to/set-up-snapcraft/).
From a checkout of this repository, run:

```sh
sh scripts/build-snap.sh
python3 scripts/verify-snap.py target/snap/thermalprint_0.4.2_amd64.snap
```

Use the version from `Cargo.toml` in the filename. The build adopts that version
automatically and uses Cargo.lock. The helper creates a clean source snapshot
under `target/snap-source.*` so existing Cargo outputs are not copied into the
build container. It retains this tree for logs and resuming a failed build.
The recipe clears only the build-time `LD_LIBRARY_PATH` override so a newer
Rustup snap does not load the GNOME SDK's incompatible libc. GNOME's runtime
desktop/GPU setup remains enabled.

Artifacts are written under `target/snap/`; an optional first argument selects
a different output directory.

The **Snap package** GitHub Actions workflow builds and inspects a `.snap`
artifact for pull requests, relevant main-branch changes, and manual dispatches.
It requires no Store credentials. The inspection checks strict confinement,
printing permissions, bundled tools, version, launchers, MIME data, and licenses.
Python packaging checks require PyYAML (`python3-yaml` on Ubuntu).

## Install a local build

```sh
sudo snap install --dangerous target/snap/thermalprint_0.4.2_amd64.snap
snap connections thermalprint
snap run thermalprint
```

`--dangerous` permits a locally built package without Store assertions; it does
not disable strict confinement. Do not use `--devmode` for release testing.
The GNOME extension supplies desktop, accessibility, X11/Wayland, and GPU setup.
The recipe declares the CUPS provider dependency; for a sideload, if its
connection is missing, install and connect it explicitly:

```sh
sudo snap install cups
sudo snap connect thermalprint:cups cups:cups
```

If the printing socket is missing, ThermalPrint shows this command in its
status bar instead of a generic printer error.

The launcher defaults `CUPS_SERVER` to `/var/cups/cups.sock`, the socket exposed
by the printing interface, and preserves an explicitly supplied server.
Printer configuration remains in the host's printer settings or the CUPS
provider. `cups-control`, direct USB access, and printer-administration
permissions are not requested. Poppler and the CUPS client utilities are bundled;
PDFium is optional and is not downloaded by this recipe.

Features that read your files work in the Snap within these limits: batch
printing from CSV and the watch folder read files you choose through the file
chooser or in your home folder; the watch folder cannot read hidden folders
(such as `~/.cache`) or other drives unless `removable-media` is connected. The
app shows a hint when it chooses a folder the Snap cannot read. Preferences,
printer profiles, and print history live in the Snap's own user-data directory.

The GNOME extension enables file chooser portals. The `home` connection also
allows ordinary documents in the user's home directory. Direct access to USB
storage is optional and needs an explicit connection:

```sh
sudo snap connect thermalprint:removable-media
```

This is useful for saving designs atomically on external drives. Files outside
Snap's permitted locations may require selection through the file chooser or
an appropriate interface connection. Snap preferences live in the Snap's own
user-data directories; the native package's preferences are not imported.

## Saved-design associations

The exported desktop launcher advertises `application/x-thermalprint`, and
opening a label passes its path directly to Label Designer. Snapd gives the
launcher an instance-qualified filename (`thermalprint_thermalprint.desktop`);
the app uses the matching window ID for desktop grouping.

A strict Snap cannot install its custom MIME definition into the host's MIME
database. Hosts with the Debian/RPM package already have that definition. On a
Snap-only host, run this optional helper **from a normal desktop terminal**, not
through `snap run`:

```sh
sh /snap/thermalprint/current/usr/share/doc/thermalprint/register-mime.sh
```

It registers only `.thermalprint` in the user's MIME database and selects the
Snap's desktop launcher for that type. It does not claim ordinary JSON files,
PDFs, or images. It requires the host tools `update-mime-database` and `xdg-mime`
(`shared-mime-info` and `xdg-utils` on Ubuntu). For a parallel instance, pass its
`/snap/thermalprint_INSTANCE/current` directory as the helper's argument.

After uninstalling the Snap, choose a different default application for label
files if needed. The optional user MIME definition remains useful to other
ThermalPrint installations; Snap removal does not remove this host-side setup.

## Confinement acceptance checks

Before promoting the package beyond an experimental channel:

- Launch on X11 and Wayland and confirm icon, window grouping, clipboard,
  light/dark themes, screen-reader access, file chooser portals, and About links.
- Open PDF and image labels from the chooser and from the file manager; export
  PDF/PNG/SVG/ZPL and save/reopen a `.thermalprint` design with a space in its path.
- Verify optional host MIME registration and double-click opening on a machine
  that has never installed the native package.
- Verify Save/Discard/Cancel on New, Open, template replacement, and window close,
  including a cancelled save and a denied destination.
- Enumerate real thermal printer queues through the CUPS proxy. Check media,
  label dimensions, DPI options, job status, copies, and a physical test print.
- Test raw ZPL only with a known-compatible raw queue. Confirm that printer
  capability/driver-description lookup through the proxy works; the Snap does
  not read host PPD files under `/etc/cups/ppd`.
- Test optional removable-media access and saving a design on an external drive.
- Choose a watch folder in Downloads, drop in a PDF, and confirm it joins the queue;
  choose a hidden or external folder and confirm the hint appears.
- Open a CSV from Downloads in Batch print and confirm the preview renders.
- Review confinement denials and package/library lint results. Build success
  alone does not demonstrate printer or desktop compatibility.

Record the host distribution, snapd version, display session, printer/driver,
connection state, and results before calling the Snap stable.

## Store publication

Tagged releases publish automatically. One-time setup:

1. Sign in with `snapcraft login` and register the name if it is yours to take:
   `snapcraft register thermalprint`. Name availability is not assumed here.
2. Export limited credentials and keep the output out of the repository:

   ```sh
   snapcraft export-login --snaps thermalprint --channels stable \
     --acls package_access,package_push,package_update,package_release -
   ```

3. In GitHub **Settings → Secrets and variables → Actions**, save the output as
   the secret `SNAPCRAFT_STORE_CREDENTIALS`. Credentials expire; re-export and
   replace the secret when a publish fails to authenticate.
4. In **Settings → Environments**, create `snap-store`. Publishing is automatic;
   optionally add a required reviewer there if you want an approval gate.

To release: bump `version` in `Cargo.toml` (and `Cargo.lock`), merge to `main`,
then push a tag that exactly matches it, for example `git tag v0.4.2 && git push
origin v0.4.2`. The workflow checks the tag, runs the packaging tests, builds and
inspects the Snap, then releases it to **stable**. The same `v*` tag also
triggers the APT release ([release-apt.yml](../.github/workflows/release-apt.yml))
and the Fedora RPM release ([release-rpm.yml](../.github/workflows/release-rpm.yml)),
so one tag deploys to all three. Copr submission additionally needs the
`COPR_ENABLED` repository variable and `COPR_CONFIG` secret.

The first Store upload goes through automated review. To make
`removable-media` connect automatically, ask on the Snap Store forum; until
then it stays an optional manual connection. After the first release, install
from the Store on a clean machine and run `snap connections thermalprint` to
confirm the `cups` connection.

References: [CUPS interface](https://snapcraft.io/docs/reference/interfaces/cups-interface/),
[GNOME extension](https://ubuntu.com/docs/snapcraft/9/reference/extensions/gnome-extension/),
[desktop portals](https://snapcraft.io/docs/explanation/snap-development/xdg-desktop-portals/).
