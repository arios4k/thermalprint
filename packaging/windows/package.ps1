param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$PdfiumDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$IdentityName = 'ThermalPrint.Development',
    [string]$Publisher = 'CN=ThermalPrint Development',
    [string]$PublisherDisplayName = 'ThermalPrint Development',
    [string]$MakeAppx
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (!$MakeAppx) {
    $sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
    $MakeAppx = Get-ChildItem -Path "$sdk/*/x64/makeappx.exe" |
        Sort-Object { [version]$_.Directory.Parent.Name } -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    if (!$MakeAppx) { throw 'Install the Windows SDK or pass -MakeAppx.' }
}

$versionMatch = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "([0-9]+\.[0-9]+\.[0-9]+)"$' | Select-Object -First 1
if (!$versionMatch) { throw 'Expected a three-part release version in Cargo.toml.' }
$version = $versionMatch.Matches[0].Groups[1].Value
$output = New-Item -ItemType Directory -Force -Path $OutputDirectory
$stage = Join-Path $output.FullName ([guid]::NewGuid().ToString())
$assets = New-Item -ItemType Directory -Path (Join-Path $stage 'Assets') -Force
try {
    Copy-Item -LiteralPath $Executable -Destination (Join-Path $stage 'thermalprint.exe')
    Copy-Item -LiteralPath (Join-Path $PdfiumDirectory 'bin/pdfium.dll') -Destination $stage
    $notices = New-Item -ItemType Directory -Path (Join-Path $stage 'PDFium')
    Copy-Item -LiteralPath (Join-Path $PdfiumDirectory 'LICENSE') -Destination $notices.FullName
    Copy-Item -LiteralPath (Join-Path $PdfiumDirectory 'licenses') -Destination $notices.FullName -Recurse
    Copy-Item -LiteralPath (Join-Path $repo 'LICENSE') -Destination (Join-Path $stage 'LICENSE-ThermalPrint.txt')
    foreach ($font in 'LICENSE.txt', 'LICENSE-Noto.txt', 'LICENSE-VarelaRound.txt') {
        Copy-Item -LiteralPath (Join-Path $repo "assets/fonts/$font") -Destination (Join-Path $stage "Fonts-$font")
    }

    Add-Type -AssemblyName System.Drawing
    $icon = [System.Drawing.Image]::FromFile((Join-Path $repo 'data/org.thermalprint.ThermalPrint.png'))
    try {
        foreach ($size in 44, 50, 150) {
            $name = if ($size -eq 50) { 'StoreLogo.png' } else { "Square${size}x${size}Logo.png" }
            $bitmap = [System.Drawing.Bitmap]::new($icon, $size, $size)
            try {
                $bitmap.Save((Join-Path $assets.FullName $name), [System.Drawing.Imaging.ImageFormat]::Png)
            } finally {
                $bitmap.Dispose()
            }
        }
    } finally {
        $icon.Dispose()
    }

    [xml]$manifest = Get-Content -Raw (Join-Path $PSScriptRoot 'AppxManifest.xml')
    $manifest.Package.Identity.Name = $IdentityName
    $manifest.Package.Identity.Publisher = $Publisher
    $manifest.Package.Identity.Version = "$version.0"
    $manifest.Package.Properties.PublisherDisplayName = $PublisherDisplayName
    $manifest.Save((Join-Path $stage 'AppxManifest.xml'))

    $package = Join-Path $output.FullName "ThermalPrint-$version-x64.msix"
    & $MakeAppx pack /d $stage /p $package /o
    if ($LASTEXITCODE -ne 0) { throw "MakeAppx failed with exit code $LASTEXITCODE." }
} finally {
    $resolved = (Resolve-Path -LiteralPath $stage).Path
    if (!$resolved.StartsWith($output.FullName.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected staging directory $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
