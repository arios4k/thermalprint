param([Parameter(Mandatory)][string]$Destination)

$ErrorActionPreference = 'Stop'
$version = '8125'
$hash = '9A964261760CFF52570AABF038AB346D3A5034DD8B30D0D5FE8861235D356B8B'
$directory = New-Item -ItemType Directory -Force -Path $Destination
$archive = Join-Path $directory.FullName 'pdfium-win-x64.tgz'
Invoke-WebRequest -Uri "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/$version/pdfium-win-x64.tgz" -OutFile $archive
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $hash) {
    throw 'PDFium download failed SHA-256 verification.'
}
& tar.exe -xf $archive -C $directory.FullName bin/pdfium.dll LICENSE
if ($LASTEXITCODE -ne 0) { throw 'Could not extract the PDFium runtime and license.' }
