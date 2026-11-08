# Release maintenance

[Back to the project](../README.md) · [Fedora and Copr](../packaging/rpm/README.md)

## Debian/Ubuntu APT release

The package name and executable are both `thermalprint`. The APT repository lives on the `gh-pages` branch of [arios4k/thermalprint](https://github.com/arios4k/thermalprint) and is served at `https://arios4k.github.io/thermalprint/`. `cups-client` supplies `lp`, `lpstat`, and `lpoptions`; `poppler-utils` supplies the PDF rendering fallback. The installer does not configure a printer or install a model-specific driver. Snap Store publishing for tagged releases and its acceptance checklist are documented in [Snap release preparation](SNAP.md). Flatpak is planned for later.

To install a downloaded `.deb` directly, use APT so package dependencies are installed:

```sh
sudo apt install ./thermalprint_0.6.0-1_amd64.deb
```

Replace the filename with the version you downloaded. The application appears as **ThermalPrint** in the desktop launcher, using the bundled color icon. You can also run `thermalprint` from a terminal.

### Release automation

The [release workflow](../.github/workflows/release-apt.yml) checks formatting, Clippy, and tests, builds the Debian package on Ubuntu 22.04, signs an APT index with `reprepro`, keeps the index and public signing key on `gh-pages`, deploys them with GitHub Pages Actions, and attaches the package to a GitHub release. The tag must exactly match the `Cargo.toml` version, for example `v0.4.0`. Only create a release tag after reviewing the build and package contents.

One-time repository setup is required: configure GitHub Pages to use **GitHub Actions**, permit Actions to write repository contents, and add `APT_GPG_PRIVATE_KEY_B64` (base64-encoded dedicated, unprotected CI signing key) and `APT_GPG_FINGERPRINT` as Actions secrets. Keep the private key outside the repository. The workflow creates the `gh-pages` branch on first release. It does not publish when ordinary commits land on `main`.

To prepare a dedicated signing key on a trusted machine, generate a separate GPG key for this repository, record its full fingerprint, and export its secret key to a local file. Base64-encode that file without line wrapping and save the result as `APT_GPG_PRIVATE_KEY_B64`; save the fingerprint as `APT_GPG_FINGERPRINT`. Delete the temporary secret-key export after setting the secret. Never commit it. In GitHub **Settings → Actions → General**, set workflow permissions to **Read and write**. In **Settings → Pages**, select **GitHub Actions** as the source. Verify `key.asc`, `dists/stable/InRelease`, and the package URL are reachable before announcing the APT install command.

