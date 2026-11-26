<#
.SYNOPSIS
Uploads a Store MSIX to Partner Center, applies store-listing.json, and submits for certification.

.DESCRIPTION
Requires the Microsoft Store Developer CLI (msstore), already configured with `msstore reconfigure`.
The steps run in this order because `msstore publish` replaces any pending draft:
  1. Upload the package into a new draft (no commit).
  2. Read the draft, set release notes, search terms and short description for every language.
  3. Save the draft, then commit it for certification (unless -NoSubmit).
Prices, trial, screenshots and descriptions are kept from the live listing.
#>
param(
    [Parameter(Mandatory)] [string] $Msix,
    [string] $ListingFile = (Join-Path $PSScriptRoot 'store-listing.json'),
    [switch] $NoSubmit
)
$ErrorActionPreference = 'Stop'

function Invoke-MsStore {
    $output = & msstore @args
    if ($LASTEXITCODE -ne 0) {
        throw "msstore $($args[0]) $($args[1]) failed (exit code $LASTEXITCODE):`n$($output -join "`n")"
    }
    $output
}

function Set-Field($Object, [string] $Name, $Value) {
    if ($Object.PSObject.Properties[$Name]) {
        $Object.$Name = $Value
    } else {
        $Object | Add-Member -NotePropertyName $Name -NotePropertyValue $Value
    }
}

$listing = Get-Content -Raw -Encoding utf8 $ListingFile | ConvertFrom-Json
$productId = $listing.productId
if (!(Test-Path $Msix)) { throw "Package not found: $Msix" }
if ((Split-Path -Leaf $Msix) -notmatch [regex]::Escape($listing.version)) {
    throw "$Msix does not look like version $($listing.version) from $ListingFile."
}

Write-Host "Uploading $Msix to $productId as a draft"
Invoke-MsStore publish $Msix --appId $productId --noCommit | Write-Host

$submission = (Invoke-MsStore submission get $productId) -join "`n" | ConvertFrom-Json
foreach ($language in $listing.listings.PSObject.Properties.Name) {
    $wanted = $listing.listings.$language
    $current = $submission.Listings.$language
    if (!$current) {
        throw "Partner Center has no '$language' listing. Add the language in Partner Center or remove it from $ListingFile."
    }
    $base = $current.BaseListing
    Set-Field $base 'ReleaseNotes' $wanted.releaseNotes
    Set-Field $base 'Keywords' @($wanted.searchTerms)
    Set-Field $base 'ShortDescription' $wanted.shortDescription
    Write-Host "$language listing: release notes, $($wanted.searchTerms.Count) search terms, short description"
}

$json = $submission | ConvertTo-Json -Depth 100 -Compress
# Windows limits a command line to 32,767 characters.
if ($json.Length -gt 30000) {
    throw "The submission is $($json.Length) characters, too long to pass to msstore. Update the listing in Partner Center instead."
}
Invoke-MsStore submission update $productId $json | Write-Host

# Read it back so a field the API ignored fails here, not silently in the Store.
$saved = (Invoke-MsStore submission get $productId) -join "`n" | ConvertFrom-Json
foreach ($language in $listing.listings.PSObject.Properties.Name) {
    $base = $saved.Listings.$language.BaseListing
    $want = @($listing.listings.$language.searchTerms) -join '|'
    $have = @($base.Keywords) -join '|'
    if ($have -ne $want) { throw "$language search terms did not save (have '$have')." }
    if ($base.ReleaseNotes -ne $listing.listings.$language.releaseNotes) { throw "$language release notes did not save." }
}

if ($NoSubmit) {
    Write-Host "Draft saved. Submit it in Partner Center when ready."
} else {
    Invoke-MsStore submission publish $productId | Write-Host
    Write-Host "Submitted $productId $($listing.version) for certification."
}
