# Lists installed software from the Uninstall registry keys (built-in facilities only) and flags development tooling that Vela must
# not need: Rust, Node.js, Visual Studio and its Build Tools. Exit code 1 when any is present. -FromJson reads a saved listing
# instead of the registry (used by the build host's self-test).
param([string]$FromJson)
$ErrorActionPreference = 'Stop'
if ($FromJson) {
  $names = @((Get-Content -LiteralPath $FromJson -Raw | ConvertFrom-Json) | ForEach-Object { $_.name })
} else {
  $keys = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
  )
  $names = @(Get-ItemProperty -Path $keys -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName } | ForEach-Object { $_.DisplayName })
}
$pattern = '(?i)\b(rust|rustup|rustc|cargo|node\.?js|visual studio|build tools|msvc|vs_buildtools)\b'
$dev = @($names | Where-Object { $_ -match $pattern })
[pscustomobject]@{ count = $names.Count; software = ($names | Sort-Object); development_tooling = $dev; clean = ($dev.Count -eq 0) } | ConvertTo-Json -Depth 4
if ($dev.Count -gt 0) { exit 1 }
