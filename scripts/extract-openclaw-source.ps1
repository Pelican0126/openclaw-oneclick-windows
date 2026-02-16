param(
  # NPM spec to download. We use "pack" so this does NOT install globally and
  # will not affect your existing Windows OpenClaw installation.
  [string]$Spec = "openclaw@latest",
  # Output folder under this repo
  [string]$OutDir = (Join-Path (Split-Path -Parent $PSScriptRoot) "source"),
  # Keep the downloaded .tgz for debugging / offline use.
  [switch]$KeepTgz
)

$ErrorActionPreference = "Stop"

function Ensure-Dir([string]$Path) {
  if (-not (Test-Path $Path)) {
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
  }
}

function Get-TarCommand {
  $cmd = Get-Command tar -ErrorAction SilentlyContinue
  if ($null -eq $cmd) {
    throw "tar not found. Please use Windows 10/11 built-in tar, or install tar."
  }
  return $cmd
}

Ensure-Dir $OutDir

Write-Host "== Extract OpenClaw source (npm pack) =="
Write-Host "Spec  : $Spec"
Write-Host "OutDir: $OutDir"

# Resolve version (best-effort; some npm registries may not allow this call).
$version = ""
try {
  $version = (& npm view $Spec version 2>$null).Trim()
} catch {
  $version = ""
}
if ([string]::IsNullOrWhiteSpace($version)) {
  $version = "unknown"
}

Push-Location $OutDir
try {
  # Keep version in folder name so we never need to delete (safe in restricted shells).
  $safeVersion = if ($version -eq "unknown") { (Get-Date -Format "yyyyMMdd-HHmmss") } else { $version }
  $destRoot = Join-Path $OutDir ("openclaw-npm-" + $safeVersion)
  Ensure-Dir $destRoot

  # Idempotency: if the same version was already extracted, do not re-download.
  $already = Test-Path (Join-Path $destRoot "openclaw\\package.json")
  if ($already) {
    $latestPath = Join-Path $OutDir "OPENCLAW_SOURCE_LATEST.txt"
    $relative = "openclaw-npm-$safeVersion\\openclaw"
    Set-Content -Encoding UTF8 $latestPath $relative

    Write-Host "[OK] Already extracted: $destRoot"
    Write-Host "[OK] Latest pointer: $latestPath -> $relative"
    return
  }

  # Download tarball to OutDir (no global install).
  # NOTE: npm prints "notice" messages to stderr even on success.
  # With `$ErrorActionPreference="Stop"` those stderr lines become terminating errors.
  $prevEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  $packOut = & npm pack $Spec 2>&1
  $packCode = $LASTEXITCODE
  $ErrorActionPreference = $prevEap

  $packLines = @($packOut | ForEach-Object { $_.ToString() })
  if ($packCode -ne 0) {
    throw "npm pack failed (code=$packCode):`n$($packLines -join "`n")"
  }

  # `npm pack` prints the tgz filename (usually the last line). Detect by extension.
  $tgz = ($packLines | Where-Object { $_ -match "\.tgz$" } | Select-Object -Last 1).Trim()
  if ([string]::IsNullOrWhiteSpace($tgz)) {
    $tgz = ($packLines | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Last 1).Trim()
  }
  if ([string]::IsNullOrWhiteSpace($tgz) -or -not (Test-Path $tgz)) {
    throw "Cannot find packed tgz file from npm pack output. Output:`n$($packLines -join "`n")"
  }

  $tar = Get-TarCommand
  & $tar -xzf $tgz -C $destRoot
  if ($LASTEXITCODE -ne 0) {
    throw "tar extract failed for $tgz"
  }

  # npm tarballs extract into a top-level `package/` directory.
  $pkgDir = Join-Path $destRoot "package"
  if (Test-Path $pkgDir) {
    $finalDir = Join-Path $destRoot "openclaw"
    if (-not (Test-Path $finalDir)) {
      Move-Item -Force -Path $pkgDir -Destination $finalDir
    }
  }

  # Record metadata for traceability.
  $metaPath = Join-Path $destRoot "EXTRACTED_FROM.txt"
  @(
    "spec=$Spec"
    "version=$version"
    "packed=$tgz"
    ("date=" + (Get-Date).ToString("s"))
  ) | Set-Content -Encoding UTF8 $metaPath

  $latestPath = Join-Path $OutDir "OPENCLAW_SOURCE_LATEST.txt"
  $relative = "openclaw-npm-$safeVersion\\openclaw"
  Set-Content -Encoding UTF8 $latestPath $relative

  Write-Host "[OK] Extracted to: $destRoot"
  Write-Host "[OK] Latest pointer: $latestPath -> $relative"

  if (-not $KeepTgz) {
    Remove-Item -Force $tgz -ErrorAction SilentlyContinue
  }
} finally {
  Pop-Location
}
