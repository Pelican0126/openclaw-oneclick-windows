const fs = require("fs");
const path = require("path");

// Copies Tauri bundle outputs (NSIS exe + MSI) into a top-level folder.
// Purpose: make the installer packages easy to find after `npm run tauri:build`.
//
// Output:
//   release/OpenClawInstaller-v{version}-setup.exe
//   release/OpenClawInstaller-v{version}.msi
//
// Notes:
// - This script is intentionally conservative: it prefers files that match the
//   current version, and falls back to the newest file by mtime.
// - `release/` is git-ignored to avoid accidentally committing large binaries.

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function listFiles(dir) {
  try {
    return fs.readdirSync(dir);
  } catch {
    return [];
  }
}

function pickNewest(dir, files) {
  if (files.length === 0) return null;
  const sorted = [...files].sort((a, b) => {
    const aStat = fs.statSync(path.join(dir, a));
    const bStat = fs.statSync(path.join(dir, b));
    return (bStat.mtimeMs || 0) - (aStat.mtimeMs || 0);
  });
  return sorted[0];
}

function pickBundle(dir, ext, versionNeedle) {
  const lowerExt = ext.toLowerCase();
  const all = listFiles(dir).filter((name) => name.toLowerCase().endsWith(lowerExt));
  if (all.length === 0) return null;

  const versionMatches = all.filter((name) => name.includes(versionNeedle));
  return pickNewest(dir, versionMatches.length > 0 ? versionMatches : all);
}

function main() {
  const repoRoot = path.resolve(__dirname, "..");
  const pkgJsonPath = path.join(repoRoot, "package.json");
  const pkg = JSON.parse(fs.readFileSync(pkgJsonPath, "utf8"));
  const version = pkg.version;

  const bundleRoot = path.join(repoRoot, "src-tauri", "target", "release", "bundle");
  const nsisDir = path.join(bundleRoot, "nsis");
  const msiDir = path.join(bundleRoot, "msi");

  const outDir = path.join(repoRoot, "release");
  ensureDir(outDir);

  const needle = `_${version}_`;
  const exeName = pickBundle(nsisDir, ".exe", needle);
  const msiName = pickBundle(msiDir, ".msi", needle);

  const copied = [];

  if (exeName) {
    const src = path.join(nsisDir, exeName);
    const dst = path.join(outDir, `OpenClawInstaller-v${version}-setup.exe`);
    fs.copyFileSync(src, dst);
    copied.push(dst);
  }

  if (msiName) {
    const src = path.join(msiDir, msiName);
    const dst = path.join(outDir, `OpenClawInstaller-v${version}.msi`);
    fs.copyFileSync(src, dst);
    copied.push(dst);
  }

  if (copied.length === 0) {
    console.error("[ERROR] No bundle artifacts found to copy.");
    console.error(`Checked: ${nsisDir}`);
    console.error(`         ${msiDir}`);
    process.exit(1);
  }

  console.log("[OK] Copied bundle artifacts:");
  for (const file of copied) console.log(`- ${file}`);
}

main();

