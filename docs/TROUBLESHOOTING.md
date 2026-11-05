# Troubleshooting

[Back to the project](../README.md)

| Symptom | What to check |
| --- | --- |
| PDF cannot open | Install PDFium or Poppler's `pdfinfo` and `pdftoppm`. Try re-exporting encrypted or damaged PDFs. |
| No printers listed | Confirm CUPS is running, a queue exists, and `lpstat -p` works for your user. Export can still be used. |
| No suitable media choice | Configure the printer queue for label stock. Custom dimensions require a driver that advertises Custom media. |
| Print button is disabled | Finish rendering, review the crop, choose a printer and matching media, and resolve clipping. |
| Print all is blocked | Select and review each queue entry. Each PDF page and detected region has its own review state. |
| Label is too small | Crop excess whitespace first, then inspect Fit versus Actual size and the paper dimensions. |
| Edges are cut on paper | Increase margins, check printer calibration and driver media, then try a test print. |
| CUPS accepted the job but nothing printed | Use Job status, inspect the CUPS queue and printer, and check stock, connection, and driver configuration. |
| Barcode looks weak | Disable enhancement, verify the source quality and quiet zones, and test a physical print. |

Set `RUST_LOG=thermalprint=debug` when running from a terminal to see application diagnostics. Avoid posting logs together with private shipping labels or local file paths.

