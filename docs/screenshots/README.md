# Documentation screenshots

These PNGs capture the actual ThermalPrint interface at 1440 × 810 in light
and dark themes. They show the current source version. The printer queue,
order references, addresses, barcodes, and QR contents are fictional demo data;
no real shipping documents, printer names, or account information are included.
The example shipping label is explicitly marked **not postage**.

To regenerate all eight screens in both themes from a Linux desktop session:

```sh
sh scripts/capture-screenshots.sh
```

The script builds the app, uses temporary preference directories, and invokes
its opt-in demo capture mode. Each PNG is saved directly from the app's rendered
framebuffer, then the preview exits. It does not discover real printers or send
print jobs. Normal app launches are unaffected. You can pass an alternative
output directory as the script's first argument.
