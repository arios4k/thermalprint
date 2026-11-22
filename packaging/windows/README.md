# ThermalPrint on Windows 11

The Windows x64 build uses the shared Rust app and the Windows print spooler. Install your printer's Windows driver and add its queue in **Settings → Bluetooth & devices → Printers & scanners**. Set up the label stock in the driver so ThermalPrint can discover its dimensions. RAW ZPL requires a ZPL-compatible printer and a queue that accepts RAW jobs.

## Build and package

Use Rust with the `x86_64-pc-windows-msvc` target, Visual Studio C++ Build Tools, and the Windows SDK. Run these commands in PowerShell from the repository root.

```powershell
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --locked --target x86_64-pc-windows-msvc
./packaging/windows/get-pdfium.ps1 -Destination target/pdfium
./packaging/windows/package.ps1 -Executable target/x86_64-pc-windows-msvc/release/thermalprint.exe -PdfiumDirectory target/pdfium -OutputDirectory target/msix
```

The command builds a development package by default. For an unpackaged test, copy `target/pdfium/bin/pdfium.dll` next to `thermalprint.exe`. To run the Windows PDF import test, supply the extracted `bin` directory through `THERMALPRINT_PDFIUM_DIR`.

## Microsoft Store releases

Set the following GitHub Actions repository variables using the values shown under the existing app's **Product identity** in Partner Center.

| GitHub variable | Partner Center field |
| --- | --- |
| `THERMALPRINT_STORE_IDENTITY_NAME` | Package/Identity/Name |
| `THERMALPRINT_STORE_PUBLISHER` | Package/Identity/Publisher |
| `THERMALPRINT_STORE_PUBLISHER_DISPLAY_NAME` | Package/Properties/PublisherDisplayName |

### Build a Windows-only Store update

Commit the updated `Cargo.toml` version and `Cargo.lock`, then tag that commit with `windows-vMAJOR.MINOR.PATCH`. Use a version higher than the current Store package. For example, for version `0.14.0`:

```sh
git tag windows-v0.14.0
git push origin windows-v0.14.0
```

The **Windows 11** workflow requires the tag to match the Cargo version and all three Store identity variables to be set. The MSIX version for this example is `0.14.0.0`. A `windows-v*` tag starts the Windows build without releasing Linux packages. Use the regular `v*` tag when releasing both platforms.

Download the **ThermalPrint-Windows-x64-store** artifact, extract the MSIX, and upload it through **Partner Center → ThermalPrint → Start update → Packages**. Submit the update for certification. Local installation requires a signature trusted by the test machine. Microsoft signs the published Store package during certification.

For the initial Store release, set the US base price to **$4.99** and select a **7-day time-limited trial** under **Pricing and availability**. The trial offers the full app so customers can test their printer and label stock. Linux remains free. [Microsoft Store pricing and trials](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/price-and-availability)

Before submitting, install a signed test package on Windows 11. Open a `.thermalprint` design through File Explorer, import a mixed-size PDF, and verify image and label exports. Print a 4 × 6 label and a Letter or A4 sheet. Check physical dimensions, offsets, copies, queue status, and barcode scans. Repeat with a RAW ZPL queue and test watched-folder printing with fictional labels.
