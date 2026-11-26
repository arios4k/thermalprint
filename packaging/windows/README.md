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

Commit the updated `Cargo.toml` version and `Cargo.lock`, then tag that commit with `windows-vMAJOR.MINOR.PATCH`. Use a version higher than the current Store package. For example, for a Windows-only fix released as version `0.16.1`:

```sh
git tag windows-v0.16.1
git push origin windows-v0.16.1
```

The **Windows 11** workflow requires the tag to match the Cargo version and all three Store identity variables to be set. The MSIX version for this example is `0.16.1.0`. A `windows-v*` tag starts the Windows build without releasing Linux packages. Use the regular `v*` tag when releasing both platforms.

### Automatic Store submission

After the build, the workflow's **store** job uploads the MSIX to Partner Center, applies [`store-listing.json`](store-listing.json) to every listing language, and submits the update for certification. The job runs [`submit-store.ps1`](submit-store.ps1) with the [Microsoft Store Developer CLI](https://learn.microsoft.com/windows/apps/publish/msstore-dev-cli/overview) (`msstore`). It keeps the price, trial, descriptions, and screenshots that are live.

Before each release, update `store-listing.json`: set `version` and write `releaseNotes` ("What's new in this version") in all four languages. Keep `searchTerms` to at most 7 terms per language, each no longer than 30 characters, with no more than 21 words in total and no word repeated. `tests/store_listing.rs` checks these limits and fails when the version does not match `Cargo.toml`.

To keep the submission as a draft for a final review in Partner Center, set the repository variable `THERMALPRINT_STORE_SUBMIT` to `draft`. To leave the Store alone for a release, for example a Linux-only fix while an earlier update is still in certification, set it to `skip`, and remove it afterwards.

#### One-time setup

1. In [Microsoft Entra admin center](https://entra.microsoft.com), register an application (**Identity → Applications → App registrations → New registration**) and create a client secret under **Certificates & secrets**. Copy the secret value when it is created.
2. In Partner Center, open **Account settings → User management → Microsoft Entra applications**, add the application, and give it the **Manager** role.
3. In GitHub, open **Settings → Environments**, create an environment named `microsoft-store`, and add these secrets. Add yourself as a required reviewer if each submission should wait for your approval.

| Secret | Where to find it |
| --- | --- |
| `MSSTORE_TENANT_ID` | Entra admin center → Overview → **Tenant ID** |
| `MSSTORE_CLIENT_ID` | The app registration → **Application (client) ID** |
| `MSSTORE_CLIENT_SECRET` | The client secret value from step 1 |
| `MSSTORE_SELLER_ID` | Partner Center → Account settings → **Seller ID** |

Without these secrets, the store job is skipped with a notice and you can submit by hand: download the **ThermalPrint-Windows-x64-store** artifact, upload the MSIX under **Partner Center → ThermalPrint → Start update → Packages**, paste the text from `store-listing.json` into each listing language, and submit for certification. Partner Center listing pages can stay on a loading spinner; pressing Esc (stop loading) usually shows them. If the listing editor will not open, use **Export listings**, edit the CSV, and **Import listings**. Wait until the import finishes before you submit.

ThermalPrint is paid, and its price is managed in Partner Center, so the Store reports the base price as `Base`, which the submission API will not accept back. The job therefore states the base price tier and trial from `pricing` in `store-listing.json`: `Tier1052` is US$4.99, and `SevenDays` is the 7-day trial. After saving the draft, it reads the draft back and stops without submitting if the price or trial differs. **When you change the price or trial in Partner Center, update `pricing` to match.** Tiers are listed in Partner Center under **Pricing and availability → view table**. ThermalPrint uses one default market group for all markets. If you add market-specific prices, review the first automated draft (`THERMALPRINT_STORE_SUBMIT=draft`) before letting it submit.

Local installation requires a signature trusted by the test machine. Microsoft signs the published Store package during certification.

The Store edition's US base price is **$4.99** with a **7-day time-limited trial**, set under **Pricing and availability**; updates keep these settings. The trial offers the full app so customers can test their printer and label stock. Linux remains free. [Microsoft Store pricing and trials](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/price-and-availability)

Before submitting, install a signed test package on Windows 11. Open a `.thermalprint` design through File Explorer, import a mixed-size PDF, and verify image and label exports. Print a 4 × 6 label and a Letter or A4 sheet. Check physical dimensions, offsets, copies, queue status, and barcode scans. Repeat with a RAW ZPL queue and test watched-folder printing with fictional labels.

## Store listing checklist

Keep these external Partner Center fields current for each release:

- **Privacy policy:** add https://github.com/arios4k/thermalprint/blob/main/docs/PRIVACY.md
- **Support contact:** https://github.com/arios4k/thermalprint/issues
- **Website:** https://github.com/arios4k/thermalprint
- **Category:** Productivity or Graphics and Design
- **Age rating:** complete the Store questionnaire (the app has no user-generated content or mature content)
- **Release notes and search terms:** kept in [`store-listing.json`](store-listing.json) and applied by the store job
- **Screenshots:** use current Windows screenshots and include fictional label data

The package declares English, Spanish, German, and French resources. Upload matching descriptions or retain an existing fallback in Partner Center.
