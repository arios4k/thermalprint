# Fedora RPM and Copr releases

ThermalPrint's Fedora packages are built from source. They install the desktop
launcher and icons, and depend on CUPS clients, Poppler PDF utilities, and the
X11/Wayland/OpenGL libraries loaded by the app. PDFium is optional; Poppler is
installed as the PDF rendering fallback.

## Build locally on Fedora

```sh
sudo dnf install cargo rust rpm-build dnf5-plugins python3
python3 scripts/prepare-rpm.py --srpm
sudo dnf builddep target/rpm/SRPMS/thermalprint-*.src.rpm
rpmbuild --rebuild --define "_topdir $PWD/target/rpm" target/rpm/SRPMS/thermalprint-*.src.rpm
sudo dnf install ./target/rpm/RPMS/x86_64/thermalprint-*.rpm
```

The source preparation step downloads the exact crates in `Cargo.lock`.
Pass `--offline` if every locked crate is already cached. It packages the app,
tests, vendored crates, Cargo source configuration, dependency license inventory,
and license files into one archive. The RPM build and tests run with `--frozen`
and an isolated Cargo home, without fetching dependencies. The generated spec
also records bundled crate versions.

The source-packaging checks can be run without RPM tools or network access:

```sh
python3 -m unittest discover -s scripts -p test_prepare_rpm.py
```

Rust 1.96 or later is required. The release workflow builds x86_64 packages on
Fedora 44 and Fedora 45, installs each resulting package, checks its installed
files, and validates its desktop entry. These are Copr/upstream packages;
submission to Fedora's official repositories is a separate review process.

## One-time Copr setup: arios/thermalprint

1. Sign in as `arios` at <https://copr.fedorainfracloud.org/>.
2. Get your API configuration from <https://copr.fedorainfracloud.org/api/>
   and save it as `~/.config/copr` with permissions `0600`. Do not commit it.
3. Install the client and create the project (skip creation if it exists):

   ```sh
   sudo dnf install copr-cli
   copr-cli create thermalprint --chroot fedora-44-x86_64 --chroot fedora-45-x86_64
   ```

   If the project already exists, enable those two build targets in its settings.
4. In GitHub repository **Settings → Secrets and variables → Actions**, add:
   - Secret `COPR_CONFIG`: the complete contents of the API configuration file.
   - Variable `COPR_ENABLED`: `true`.
5. Review the generated package and submit the first build:

   ```sh
   copr-cli build arios/thermalprint target/rpm/SRPMS/thermalprint-*.src.rpm
   ```

Copr signs packages and maintains the installable repository. The GitHub release
RPMs are separate, unsigned artifacts built in the workflow's Fedora containers.
Keep the API key current; Copr credentials can expire. A missing or expired key
fails the Copr job; it does not disable RPM artifacts or Debian/APT releases.

## Tagged releases

The [Fedora RPM workflow](../../.github/workflows/release-rpm.yml) runs on `v*`
tags. The tag must match `Cargo.toml`, for example `v0.1.2` for version `0.1.2`.
Use a new version/tag for each release; do not move an existing tag.

The workflow:

1. Prepares a locked, vendored source RPM.
2. Builds and tests it independently on Fedora 44 and 45.
3. Installs each binary RPM to verify dependencies and package contents.
4. Attaches the two binary RPMs and source RPM to the GitHub release.
5. If `COPR_ENABLED=true`, submits the source RPM to `arios/thermalprint` and
   waits for the Copr build result. Copr builds the targets enabled in the project.

Manual workflow runs and changes to the app or packaging on `main` build
downloadable Actions artifacts without publishing a release or submitting to
Copr. Pull requests changing the packaging or Cargo manifest/lockfile also run
the RPM builds. Existing Debian/APT automation remains
in its own workflow; both workflows can safely attach assets to the same release.

## User installation (after the first successful Copr build)

```sh
sudo dnf install dnf-plugins-core
sudo dnf copr enable arios/thermalprint
sudo dnf install thermalprint
```

Updates then arrive through normal DNF updates. Alternatively, download the RPM
matching your Fedora release from GitHub and install it with `sudo dnf install
./thermalprint-<version>-<revision>.fc<release>.x86_64.rpm`. Configure the printer separately
in the desktop's printer settings.
