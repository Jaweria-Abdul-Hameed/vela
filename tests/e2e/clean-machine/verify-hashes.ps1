# Verifies a SHA-256 manifest (lines of "<hash>  <relative path>") against the files under -Root, using only built-in Windows
# facilities. Exit code 0 when every file matches, 1 otherwise. Prints one JSON object with the per-file result.
param(
  [Parameter(Mandatory = $true)][string]$Manifest,
  [Parameter(Mandatory = $true)][string]$Root
)
$ErrorActionPreference = 'Stop'
$results = @()
$bad = 0
foreach ($line in Get-Content -LiteralPath $Manifest) {
  if ($line.Trim() -eq '' -or $line.StartsWith('#')) { continue }
  $parts = $line -split '\s+', 2
  $expected = $parts[0].ToLowerInvariant()
  $rel = $parts[1].Trim().TrimStart('*')
  $path = Join-Path $Root $rel
  if (-not (Test-Path -LiteralPath $path)) {
    $results += [pscustomobject]@{ file = $rel; status = 'MISSING'; expected = $expected; actual = $null }
    $bad++
    continue
  }
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
  $ok = $actual -eq $expected
  if (-not $ok) { $bad++ }
  $results += [pscustomobject]@{ file = $rel; status = $(if ($ok) { 'OK' } else { 'MISMATCH' }); expected = $expected; actual = $actual }
}
[pscustomobject]@{ verified = ($bad -eq 0); files = $results } | ConvertTo-Json -Depth 4
if ($bad -gt 0) { exit 1 }
