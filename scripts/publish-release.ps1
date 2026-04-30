param(
  # Branch that already has the v1.1.0 commit (default matches the development branch).
  [string]$Branch = "claude/optimize-frontend-ux-wMlYC",
  # Tag to publish.
  [string]$Tag = "v1.1.0",
  # GitHub repo "owner/name". Override if you fork.
  [string]$Repo = "Pelican0126/openclaw-oneclick-windows",
  # Skip running the local secret scan before publish (not recommended).
  [switch]$SkipSecretScan,
  # Skip building - just tag and publish whatever is in release/.
  [switch]$SkipBuild,
  # Mark the GitHub release as a draft (useful for rehearsal).
  [switch]$Draft,
  # Mark as pre-release on GitHub.
  [switch]$Prerelease
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

function Step($msg) {
  Write-Host ""
  Write-Host "== $msg ==" -ForegroundColor Cyan
}

function Require-Cmd($name, $hint) {
  if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $name. $hint"
  }
}

# 1. Pre-flight: required tooling
Step "Checking tooling"
Require-Cmd git "Install Git for Windows: https://git-scm.com/download/win"
Require-Cmd npm "Install Node.js LTS: https://nodejs.org/"
Require-Cmd gh  "Install GitHub CLI: https://cli.github.com/ then 'gh auth login'"

# Make sure gh is logged in.
& gh auth status 1>$null 2>$null
if ($LASTEXITCODE -ne 0) {
  throw "gh is not authenticated. Run 'gh auth login' first."
}

# 2. Sync the working tree to the requested branch
Step "Fetching origin and checking out $Branch"
git fetch origin --tags
git checkout $Branch
git pull --ff-only origin $Branch

# Confirm we are at the right version
$pkgVersion = (Get-Content package.json -Raw | ConvertFrom-Json).version
$expected = $Tag.TrimStart("v")
if ($pkgVersion -ne $expected) {
  throw "package.json version is '$pkgVersion' but tag is '$Tag'. Bump versions or pass a matching -Tag."
}
Write-Host "package.json version: $pkgVersion (matches $Tag)" -ForegroundColor Green

# 3. Optional: secret scan
if (-not $SkipSecretScan) {
  Step "Scanning for accidentally committed secrets"
  & "$PSScriptRoot\scan-secrets.ps1"
  if ($LASTEXITCODE -ne 0) {
    throw "Secret scan failed. Resolve findings or rerun with -SkipSecretScan."
  }
}

# 4. Build the Tauri bundles
$releaseDir = Join-Path $repoRoot "release"
if (-not (Test-Path $releaseDir)) {
  New-Item -ItemType Directory -Path $releaseDir | Out-Null
}

if (-not $SkipBuild) {
  Step "Installing npm dependencies"
  npm install

  Step "Running tauri:build (this takes a few minutes)"
  npm run tauri:build
  if ($LASTEXITCODE -ne 0) {
    throw "tauri:build failed."
  }
}

# 5. Locate the produced artifacts
Step "Collecting release assets"
$expectedAssets = @(
  "OpenClawInstaller-$Tag-windows.zip",
  "OpenClawInstaller-$Tag-setup.exe",
  "OpenClawInstaller-$Tag.msi"
)
$assetPaths = @()
foreach ($name in $expectedAssets) {
  $path = Join-Path $releaseDir $name
  if (-not (Test-Path $path)) {
    throw "Missing artifact: $path. Inspect the build output or rerun without -SkipBuild."
  }
  $assetPaths += $path
}

# 6. Compute SHA256 sums
Step "Writing SHA256SUMS.txt"
$sumsPath = Join-Path $releaseDir "SHA256SUMS.txt"
$lines = foreach ($path in $assetPaths) {
  $hash = (Get-FileHash -Algorithm SHA256 -Path $path).Hash.ToLowerInvariant()
  $name = Split-Path -Leaf $path
  "$hash  $name"
}
$lines | Set-Content -Encoding ASCII -Path $sumsPath
Write-Host "Wrote $sumsPath" -ForegroundColor Green
$assetPaths += $sumsPath

# 7. Tag (idempotent)
Step "Tagging $Tag"
$existing = git tag --list $Tag
if ($existing) {
  Write-Host "Local tag $Tag already exists; reusing it." -ForegroundColor Yellow
} else {
  git tag -a $Tag -m "OpenClaw Installer $Tag"
}

# Push the tag (skip if already pushed).
$remoteTag = git ls-remote --tags origin $Tag
if ($remoteTag) {
  Write-Host "Remote tag $Tag already exists; skipping tag push." -ForegroundColor Yellow
} else {
  git push origin $Tag
}

# 8. Create the GitHub release
Step "Creating GitHub release $Tag on $Repo"
$existingRelease = & gh release view $Tag --repo $Repo 2>$null
if ($existingRelease) {
  Write-Host "Release $Tag already exists; uploading/overwriting assets only." -ForegroundColor Yellow
  & gh release upload $Tag $assetPaths --repo $Repo --clobber
} else {
  $bodyFile = Join-Path $releaseDir "github-release-body-$Tag.md"
  if (-not (Test-Path $bodyFile)) {
    throw "Missing release body file: $bodyFile"
  }
  $args = @(
    "release", "create", $Tag,
    "--repo", $Repo,
    "--title", "OpenClaw Installer $Tag",
    "--notes-file", $bodyFile
  )
  if ($Draft) { $args += "--draft" }
  if ($Prerelease) { $args += "--prerelease" }
  $args += $assetPaths
  & gh @args
}

if ($LASTEXITCODE -ne 0) {
  throw "gh release command failed."
}

Step "Done"
Write-Host "Release page: https://github.com/$Repo/releases/tag/$Tag" -ForegroundColor Green
