# ThermalPrint

Created by Alexis Rios-Negron.

A Rust desktop app for preparing marketplace and carrier shipping labels for Linux thermal printers. Open a downloaded PDF or image, inspect the crop, choose matching paper and CUPS media, and print or export. Documents stay local.

## Build and run

Requires Rust 1.96+, a Linux Wayland or X11 desktop, CUPS client commands (`lp`, `lpstat`, `lpoptions`), and PDFium (`libpdfium.so`) or Poppler utilities (`pdfinfo`, `pdftoppm`). PDFium can be installed in a custom directory by setting `THERMALPRINT_PDFIUM_DIR`. Poppler is an automatic fallback if PDFium is unavailable. On Debian/Ubuntu install `poppler-utils cups-client`; on Fedora install `poppler-utils cups-client`; on Arch install `poppler cups`.

```sh
cargo run --release
cargo test
```

Install a printer queue through the distribution's printer settings. Match the output paper dimensions to an advertised driver media size, or choose Custom when the driver supports it. CUPS receives each dimensioned PDF with scaling disabled. ThermalPrint submits batch jobs sequentially and stops on the first error, showing how many were submitted. ThermalPrint reports successful submission, and the Job status button queries the CUPS queue; physical delivery depends on the printer and driver.

## Workflow

Open or drop one or more PDF, PNG, JPEG, TIFF, BMP, or WebP files. Multipage PDFs add every page to the print queue. Use the queue to select each label, adjust its crop, mark it reviewed, reorder or remove it, then choose Print all to submit jobs to CUPS in order. If automatic detection finds multiple regions on one page, Queue all detected regions adds each region as a separate label. For a PDF, select the page. Automatic mode trims outer whitespace with a generous buffer; separated content groups are offered as candidates and require review. Manual mode lets you draw, move, and resize a crop over the original. Full page mode keeps the entire page. Confirm crop edges, inspect the output, then print or export PDF/PNG. Ctrl+O opens; Ctrl+P prints.

The default is 4 × 6 inches at 203 DPI. Other presets, custom dimensions, rotation, margins, scaling, and 300 DPI are available. Actual and custom scaling block output if it would clip content. PNG exports contain physical resolution metadata; PDF exports contain an exact paper MediaBox.

## Limitations and safety

Automatic detection is conservative whitespace analysis, not carrier-specific or barcode-aware recognition. A separated label and instruction block can require manual selection. Barcode decoding and physical scanner validation are not implemented; always inspect the crop, quiet zones, print size, and a test label. PDF import rasterizes at 300 DPI, which limits effective detail for 300 DPI output and cannot restore low-quality originals. Images without physical resolution metadata are assumed to be 300 DPI for Actual size. Unsupported encrypted PDFs require re-export.

This project does not send files to marketplaces or store tracking numbers. Recent file paths and printer preferences are saved to the user config directory. Optional black-and-white thresholding is disabled by default because it can damage fine barcode detail. Raw ZPL is not implemented; CUPS handles all printer languages through the configured driver.

## Development

The modules are separated by stage: `document` loads at print resolution; `detection` proposes crops; `layout` creates output pixels; `export` writes paper-sized PDF and DPI-tagged PNG; `printing` queries CUPS and submits a job; `ui` manages the two-column desktop workflow. To add a format, extend `document::load`. The GUI uses background threads for I/O and rendering.

Licensed under MIT; see [LICENSE](LICENSE).

Printer type is shown as a best-effort hint based on the queue name and advertised label media. CUPS does not provide a definitive thermal-printer flag.
