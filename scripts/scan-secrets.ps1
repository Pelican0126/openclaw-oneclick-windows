param(
  # If you extracted upstream OpenClaw under ./source/, keep it in the scan by default.
  # Set -SkipSource to skip scanning ./source to speed up.
  [switch]$SkipSource
)

$ErrorActionPreference = "Stop"

function RgFiles([string]$Pattern, [string[]]$ExtraGlobs) {
  $globs = @(
    "-g", "!node_modules/**",
    "-g", "!dist/**",
    "-g", "!.smoke/**",
    "-g", "!.smoke-temp/**",
    "-g", "!%TEMP%/**",
    "-g", "!~/**",
    "-g", "!src-tauri/target/**",
    "-g", "!src-tauri/target-alt/**",
    "-g", "!src-tauri/src-tauri/target-smoke/**"
  )
  if ($SkipSource) {
    $globs += @("-g", "!source/**")
  }
  if ($ExtraGlobs -and $ExtraGlobs.Count -gt 0) {
    $globs += $ExtraGlobs
  }

  # `-l` prints only file paths (no secret values).
  & rg -l -S $Pattern @globs .
}

Write-Host "== Secret scan (safe file-only output) =="
Write-Host ("Repo      : " + (Get-Location))
Write-Host ("SkipSource: " + $SkipSource)
Write-Host ""

$checks = @(
  @{ Name = "OpenAI/Kimi/Moonshot style key"; Pattern = "sk-[A-Za-z0-9]{20,}" },
  @{ Name = "Telegram bot token"; Pattern = "\b\d{6,12}:[A-Za-z0-9_-]{30,}\b" },
  @{ Name = "GitHub PAT (ghp_)"; Pattern = "ghp_[A-Za-z0-9]{20,}" },
  @{ Name = "Google API key (AIza)"; Pattern = "AIza[0-9A-Za-z_-]{30,}" },
  @{ Name = "AWS access key (AKIA)"; Pattern = "AKIA[0-9A-Z]{16}" },
  @{ Name = "Private key headers"; Pattern = "BEGIN (RSA|OPENSSH|EC|DSA) PRIVATE KEY" }
)

$foundAny = $false
foreach ($c in $checks) {
  $files = @(RgFiles -Pattern $c.Pattern -ExtraGlobs @())
  if ($files.Count -gt 0) {
    $foundAny = $true
    Write-Host ("[FOUND] " + $c.Name + " (" + $c.Pattern + ")")
    $files | ForEach-Object { Write-Host ("  - " + $_) }
    Write-Host ""
  } else {
    Write-Host ("[OK] " + $c.Name)
  }
}

Write-Host ""
Write-Host "== Config file presence check =="
$suspiciousFiles = @()
$suspiciousFiles += Get-ChildItem -Force -Recurse -File -Filter "*.env" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName
$suspiciousFiles += Get-ChildItem -Force -Recurse -File -Filter "openclaw.json" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName

if ($suspiciousFiles.Count -gt 0) {
  $foundAny = $true
  Write-Host "[FOUND] Config-like files (verify they do NOT contain secrets before uploading):"
  $suspiciousFiles | Sort-Object -Unique | ForEach-Object { Write-Host ("  - " + $_) }
} else {
  Write-Host "[OK] No *.env or openclaw.json files found in repo."
}

Write-Host ""
if ($foundAny) {
  Write-Host "[FAIL] Potential sensitive material detected. Fix/remove the files above before uploading."
  exit 1
}

Write-Host "[PASS] No obvious secrets detected by regex scan."
