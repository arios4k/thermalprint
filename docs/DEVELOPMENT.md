# Development

[Back to the project](../README.md)

The crate keeps stages separate so import formats and printer backends can be extended without coupling them to the GUI:

| Module | Responsibility |
| --- | --- |
| `document` | Decode images; render PDF pages through PDFium or Poppler; retain page dimensions and count. |
| `detection` | Propose conservative crop rectangles and separated candidate regions. |
| `layout` | Crop, rotate, scale, center, warn, and build the output canvas. |
| `export` | Write dimensioned PDF and DPI-tagged PNG files. |
| `printing` | Discover CUPS queues and capabilities, submit jobs, and query status. |
| `settings` | Validate and persist user preferences. |
| `ui` | Manage the two-column egui workflow, previews, queue, dialogs, and background work. |
| `design` | Versioned editable label model, validation, and built-in templates. |
| `design_render` | Render designer objects, real barcodes, QR codes, text, and shapes at print DPI. |
| `designer_ui` | Interactive canvas, object inspector, editing history, and designer actions. |
| `zpl` | Raster ZPL output for compatible raw printer queues. |

Build and check changes with:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

The repository includes unit tests for conservative crop proposals and integration tests for output dimensions, PDF paper size, and clipping behavior. GitHub Actions runs formatting, Clippy, and tests. Hardware, driver, barcode-scan, and large-corpus label tests still need to be added.

Contributions and reproducible sample labels with private addresses and tracking information removed are welcome. For defects, include the input format, page dimensions, intended label size, printer queue/media, and steps to reproduce. Do not publish real shipping labels or tracking numbers in issues.


## Documentation captures

Run `sh scripts/capture-screenshots.sh` from a Linux desktop session to regenerate the real UI screenshots with fictional demo data. See [screenshot notes](screenshots/README.md).
