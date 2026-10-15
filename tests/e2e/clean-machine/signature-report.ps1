# Reports the Authenticode signature of each file with Get-AuthenticodeSignature (built in). A development-signed file is expected
# to be reported as not trusted (UnknownError / NotTrusted); the report records the actual status, signer and thumbprint.
param([Parameter(Mandatory = $true)][string[]]$Path)
$ErrorActionPreference = 'Stop'
$out = foreach ($p in $Path) {
  $s = Get-AuthenticodeSignature -LiteralPath $p
  [pscustomobject]@{
    file = $p
    status = [string]$s.Status
    signer = $(if ($s.SignerCertificate) { $s.SignerCertificate.Subject } else { $null })
    thumbprint = $(if ($s.SignerCertificate) { $s.SignerCertificate.Thumbprint } else { $null })
    timestamped = [bool]$s.TimeStamperCertificate
  }
}
@($out) | ConvertTo-Json -Depth 3
