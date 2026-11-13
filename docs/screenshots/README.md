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

The window opens at 1440 × 810 (`THERMALPRINT_SCREENSHOTS_SIZE=1440x810`). On a
smaller screen the desktop shrinks or maximizes it, and the images come out at
a different size.

The user manual uses more screens. Capture each in its own run, so that a dialog
left open by one screen does not show in the next, naming it in
`THERMALPRINT_SCREENSHOTS_ONLY`, for example
`THERMALPRINT_SCREENSHOTS_ONLY=batch`. `THERMALPRINT_SCREENSHOTS_LANGUAGE`
(`es`, `de`, `fr`) captures a translation. The numbered callouts on the
`*-annotated` manual images are drawn on afterwards.
