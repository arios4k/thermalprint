<#
.SYNOPSIS
Uploads a Store MSIX to Partner Center, applies store-listing.json, and submits for certification.

.DESCRIPTION
Requires the Microsoft Store Developer CLI (msstore), already configured with `msstore reconfigure`.
The steps run in this order because `msstore publish` replaces any pending draft:
  1. Upload the package into a new draft (no commit).
  2. Read the draft, set release notes, search terms and short description for every language.
  3. Save the draft, then commit it for certification (unless -NoSubmit).
Descriptions and screenshots are kept from the live listing. The base price tier and trial
come from store-listing.json and are checked in the draft before it is submitted.
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

$priceId = $listing.pricing.priceId
$trialPeriod = $listing.pricing.trialPeriod
if ($priceId -notmatch '^Tier\d+$' -or [string]::IsNullOrWhiteSpace($trialPeriod)) {
    throw "Set pricing.priceId (TierNNNN) and pricing.trialPeriod in $ListingFile."
}

# The price is managed in Partner Center, so the Store reports it as "Base", which the
# submission API will not take back. State the same base tier explicitly instead; the draft
# is checked below before anything is submitted.
Write-Host "Uploading $Msix to $productId as a draft (base price $priceId, trial $trialPeriod)"
Invoke-MsStore publish $Msix --appId $productId --noCommit --priceId $priceId | Write-Host

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
if (!$submission.Pricing) { throw 'The draft has no pricing. Nothing was submitted; check Pricing and availability in Partner Center.' }
Set-Field $submission.Pricing 'PriceId' $priceId
Set-Field $submission.Pricing 'TrialPeriod' $trialPeriod

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
# Never commit a draft whose price or trial differs from what the app is sold at.
if ($saved.Pricing.PriceId -ne $priceId) {
    throw "The draft's base price is '$($saved.Pricing.PriceId)', not $priceId. Nothing was submitted; review the draft in Partner Center."
}
if ($saved.Pricing.TrialPeriod -ne $trialPeriod) {
    throw "The draft's trial is '$($saved.Pricing.TrialPeriod)', not $trialPeriod. Nothing was submitted; review the draft in Partner Center."
}
$overrides = if ($saved.Pricing.MarketSpecificPricings) { @($saved.Pricing.MarketSpecificPricings.PSObject.Properties).Count } else { 0 }
Write-Host "Pricing: base $priceId, trial $trialPeriod, $overrides market-specific prices"

if ($NoSubmit) {
    Write-Host "Draft saved. Submit it in Partner Center when ready."
} else {
    Invoke-MsStore submission publish $productId | Write-Host
    Write-Host "Submitted $productId $($listing.version) for certification."
}
