<div align="center">

<img src="data/org.thermalprint.ThermalPrint.png" width="112" alt="ThermalPrint app icon">

# ThermalPrint

**Your labels. Your printer. Your Linux desktop.**

Design custom labels, prepare downloaded PDFs, and print on thermal stock—all in one local workspace. ThermalPrint is made for thermal label printers, but it prints through CUPS, so it also works with regular laser and inkjet printers (choose the paper size, such as A4 or Letter, and use the sheet printing to place several labels on a page).

[![Latest release](https://img.shields.io/github/v/release/arios4k/thermalprint?label=download&color=207568)](https://github.com/arios4k/thermalprint/releases/latest)
[![Rust CI](https://github.com/arios4k/thermalprint/actions/workflows/ci.yml/badge.svg)](https://github.com/arios4k/thermalprint/actions/workflows/ci.yml)
[![Fedora RPM](https://github.com/arios4k/thermalprint/actions/workflows/release-rpm.yml/badge.svg)](https://github.com/arios4k/thermalprint/actions/workflows/release-rpm.yml)
[![Install with APT](https://img.shields.io/badge/install-APT-ffb44d?logo=debian&logoColor=white)](#debian--ubuntu)
[![Fedora Copr](https://img.shields.io/badge/Fedora-Copr-51a2da?logo=fedora&logoColor=white)](https://copr.fedorainfracloud.org/coprs/arios/thermalprint/)
[![Manual](https://img.shields.io/badge/Manual-read-2ea44f)](https://arios4k.github.io/thermalprint/manual/)
[![Snap Store](https://snapcraft.io/thermalprint/badge.svg)](https://snapcraft.io/thermalprint)
[![License: MIT](https://img.shields.io/badge/license-MIT-3187e7)](LICENSE)

[User manual](https://arios4k.github.io/thermalprint/manual/) · [Install on Fedora](#fedora) · [Install on Debian / Ubuntu](#debian--ubuntu) · [Download packages](https://github.com/arios4k/thermalprint/releases/latest)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/designer-dark.png">
  <img src="docs/screenshots/designer-light.png" width="1000" alt="ThermalPrint Label Designer with an editable shipping label, barcode, QR code, icon tools, and barcode properties">
</picture>

<sub>Actual app screenshots from the current source, with fictional sample data. The demo label is not postage. Light and dark images follow your GitHub theme.</sub>

</div>

## A workspace for everyday labels

- **Create your own.** Design labels from templates with text, addresses, logos, shapes, shipping symbols, real barcodes (including GS1) and QR codes, with six built-in fonts chosen for labels (or any font on your computer), precise alignment tools and saved templates.
- **Prepare existing labels.** Import PDFs or images from carriers and marketplaces, crop away instructions and whitespace, and review the result before printing.
- **Get the size right.** Start at **4 × 6 inches** or any size you need; ThermalPrint explains printer media and matches the output to it. Wide labels can be designed on their side and rotated 90° when they print.
- **Print or export.** Print through CUPS, or save a PDF, PNG, SVG or raster ZPL.
- **Print many at once.** Numbered runs, several labels on an A4 or Letter sheet, and one label per row of a spreadsheet.
- **Save clicks.** Watch a folder such as Downloads so new labels join the queue, and optionally print them by themselves.
- **Stay comfortable.** Light and dark themes; English, Spanish, German and French; keyboard shortcuts; and a guide to printing terms.

Document processing happens on your computer. ThermalPrint works with local files from carriers and marketplaces, including Vinted, eBay, Etsy, Shopify, USPS, UPS, and FedEx, without connecting to their accounts.

## Manual

**To learn how to use ThermalPrint, read the [ThermalPrint Manual](https://arios4k.github.io/thermalprint/manual/).** It explains every part of the app step by step: importing labels, the Label Designer, barcodes, batch printing, Print Center, automation, export, settings and troubleshooting. It also opens from the app with **F1** or the **?** button.

## Install

Packages include the application, desktop launcher, icons, CUPS client tools, and Poppler PDF utilities. Configure your printer in your desktop's printer settings, then launch **ThermalPrint** from the applications menu.

### Fedora

For **Fedora 44 and 45, x86_64**, enable the community Copr repository once:

```sh
sudo dnf install dnf5-plugins
sudo dnf copr enable arios/thermalprint
sudo dnf install thermalprint
```

Updates arrive through normal DNF updates. [View the Copr repository](https://copr.fedorainfracloud.org/coprs/arios/thermalprint/). Copr is separate from Fedora's official package repositories.

### Debian / Ubuntu

The signed APT repository provides **amd64** packages:

```sh
sudo apt update
sudo apt install curl
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://arios4k.github.io/thermalprint/key.asc | sudo tee /etc/apt/keyrings/thermalprint.asc >/dev/null
echo 'deb [arch=amd64 signed-by=/etc/apt/keyrings/thermalprint.asc] https://arios4k.github.io/thermalprint/ stable main' | sudo tee /etc/apt/sources.list.d/thermalprint.list >/dev/null
sudo apt update
sudo apt install thermalprint
```

Updates arrive through normal APT updates.

### Snap

Install from the [Snap Store](https://snapcraft.io/thermalprint) (amd64). Tagged releases are published there automatically, and Snap updates itself:

```sh
sudo snap install thermalprint
```

Printing, removable drives and double-clicking `.thermalprint` files need a one-time connection step; see [Snap](https://arios4k.github.io/thermalprint/manual/snap.html) in the manual.

### Standalone packages and source builds

[GitHub Releases](https://github.com/arios4k/thermalprint/releases) contains the published packages. Tagged releases with the Fedora workflow produce a binary RPM for each supported Fedora version and a source RPM, alongside the Debian package.

Install a downloaded package using your distribution's package manager so its dependencies are installed too:

```sh
# Debian / Ubuntu
sudo apt install ./thermalprint_*.deb

# Fedora — choose the RPM matching your Fedora release
sudo dnf install ./thermalprint-*.x86_64.rpm
```

Run these from the download folder, keeping only the RPM matching your Fedora release there. Other Linux users can [build from source](#build-from-source).

## Formats and compatibility

| Task | Supported formats or options |
| --- | --- |
| Import | PDF, PNG, JPEG, TIFF, BMP, WebP; multipage PDFs join the queue; CSV, semicolon, or tab-separated tables for batch printing |
| Editable designs | Versioned `.thermalprint` files with embedded images |
| Barcodes | Code 128, Code 39, Code 93, EAN-13, EAN-8, UPC-A, ITF, Codabar, Data Matrix, QR |
| Output | PDF, PNG; design SVG with embedded raster output; raster ZPL |
| Layout | Automatic/manual/full-page extraction, rotation, fit/actual/custom scale, margins |
| Print resolution | 203, 300, or 600 DPI |
| Desktop | Linux with X11 or Wayland; configured CUPS printer |

ThermalPrint prepares labels; it does not purchase postage or generate carrier tracking numbers. Raw ZPL requires a compatible printer and deliberately configured raw queue. This is an early release: check a physical test print and scan before relying on a shipping workflow. The app does not currently decode imported barcodes or verify physical scan quality. See the [manual](https://arios4k.github.io/thermalprint/manual/) for processing limits and printer details, and [Troubleshooting](https://arios4k.github.io/thermalprint/manual/troubleshooting.html) when a print comes out wrong.

## Build from source

Requires **Rust 1.96+**, a C/C++ build toolchain, and the usual Linux development libraries. Install CUPS client tools and Poppler utilities for printing and PDF fallback support.

```sh
git clone https://github.com/arios4k/thermalprint.git
cd thermalprint
cargo run --release --locked
```

PDFium is optional. To use a custom installation, set `THERMALPRINT_PDFIUM_DIR` to the directory containing `libpdfium.so`.

[User manual](https://arios4k.github.io/thermalprint/manual/) · [Snap packaging](docs/SNAP.md) · [Development and tests](docs/DEVELOPMENT.md) · [Fedora packaging / Copr setup](packaging/rpm/README.md) · [APT release maintenance](docs/RELEASING.md) · [Regenerate screenshots](docs/screenshots/README.md)

## Support ThermalPrint

If ThermalPrint has been useful to you, please consider [making a donation of any amount](https://paypal.me/AlexisRios) to help keep the project running and support its continued development. Every contribution is appreciated.

The intention is to keep ThermalPrint free and available to everyone. Thank you for using it and for any support you can offer!

[![Support development with PayPal](https://img.shields.io/badge/Support_development-PayPal-0070BA?logo=paypal&logoColor=white)](https://paypal.me/AlexisRios)

If ThermalPrint is useful to you, a [GitHub star](https://github.com/arios4k/thermalprint), a reproducible [bug report](https://github.com/arios4k/thermalprint/issues), or sharing it with another Linux user helps the project grow. Keep personal addresses and tracking information out of public reports.

Created by **Alexis Rios-Negron**. Also explore [Voltkit](https://github.com/arios4k/voltkit) and [Battery Health Center](https://github.com/arios4k/battery-health-center).

## License

[MIT](LICENSE) © 2026 Alexis Rios-Negron. Bundled fonts and dependencies retain their own licenses.
