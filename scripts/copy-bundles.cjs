const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

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

function writeInstallNowCmd(outDir) {
  const cmdPath = path.join(outDir, "INSTALL_NOW.cmd");
  const content = [
    "@echo off",
    "setlocal enabledelayedexpansion",
    "cd /d \"%~dp0\"",
    "",
    "set \"SETUP=\"",
    "for %%F in (OpenClawInstaller-v*-setup.exe) do (",
    "  set \"SETUP=%%F\"",
    "  goto run_setup",
    ")",
    "",
    ":run_setup",
    "if defined SETUP (",
    "  echo [install] Launching !SETUP! ...",
    "  start \"\" \"!SETUP!\"",
    "  exit /b 0",
    ")",
    "",
    "set \"MSI=\"",
    "for %%F in (OpenClawInstaller-v*.msi) do (",
    "  set \"MSI=%%F\"",
    "  goto run_msi",
    ")",
    "",
    ":run_msi",
    "if defined MSI (",
    "  echo [install] Launching !MSI! ...",
    "  msiexec /i \"!MSI!\"",
    "  exit /b 0",
    ")",
    "",
    "echo [error] Installer package not found in this folder.",
    "echo [hint] Keep this cmd file with the .exe/.msi package files.",
    "pause"
  ].join("\r\n");
  fs.writeFileSync(cmdPath, `${content}\r\n`, "utf8");
  return cmdPath;
}

function writeQuickstartTxt(outDir, version) {
  const filePath = path.join(outDir, "QUICKSTART_CN_EN.txt");
  const text = [
    `OpenClaw Installer v${version} - Quick Start`,
    "",
    "[中文]",
    "1) 下载并解压 OpenClawInstaller-v*-windows.zip。",
    "2) 双击 INSTALL_NOW.cmd（或双击 .exe/.msi 安装包）。",
    "3) 安装后打开 OpenClaw Installer，按向导一直点下一步。",
    "",
    "提示：",
    "- 关闭窗口默认是最小化到托盘，不是退出。",
    "- 如果模型列表加载慢，可先用预置模型选项安装。",
    "",
    "[English]",
    "1) Download and extract OpenClawInstaller-v*-windows.zip.",
    "2) Double-click INSTALL_NOW.cmd (or run the .exe/.msi installer directly).",
    "3) Open OpenClaw Installer and complete the wizard step by step.",
    "",
    "Notes:",
    "- Closing window minimizes to tray; it does not fully exit.",
    "- If model list is slow, use preset model options first."
  ].join("\r\n");
  fs.writeFileSync(filePath, `${text}\r\n`, "utf8");
  return filePath;
}

function copyFileIfExists(src, dst, copied) {
  if (!src || !fs.existsSync(src)) return;
  fs.copyFileSync(src, dst);
  copied.push(dst);
}

function compressStageZip(stageDir, zipPath) {
  if (process.platform !== "win32") {
    console.warn("[WARN] ZIP creation is skipped on non-Windows platform.");
    return false;
  }
  const script = [
    "$ErrorActionPreference='Stop'",
    `$src=${JSON.stringify(path.join(stageDir, "*"))}`,
    `$dst=${JSON.stringify(zipPath)}`,
    "if (Test-Path $dst) { Remove-Item -Force $dst }",
    "Compress-Archive -Path $src -DestinationPath $dst -Force"
  ].join("; ");
  execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script], {
    stdio: "inherit"
  });
  return fs.existsSync(zipPath);
}

function sha256(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function writeChecksums(outDir, files) {
  const rows = files
    .filter((file) => fs.existsSync(file))
    .map((file) => `${sha256(file)}  ${path.basename(file)}`);
  const checksumPath = path.join(outDir, "SHA256SUMS.txt");
  fs.writeFileSync(checksumPath, `${rows.join("\n")}\n`, "utf8");
  return checksumPath;
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

  if (!exeName && !msiName) {
    console.error("[ERROR] No bundle artifacts found to copy.");
    console.error(`Checked: ${nsisDir}`);
    console.error(`         ${msiDir}`);
    process.exit(1);
  }

  const exeOut = path.join(outDir, `OpenClawInstaller-v${version}-setup.exe`);
  const msiOut = path.join(outDir, `OpenClawInstaller-v${version}.msi`);
  const zipOut = path.join(outDir, `OpenClawInstaller-v${version}-windows.zip`);

  const copied = [];
  if (exeName) {
    copyFileIfExists(path.join(nsisDir, exeName), exeOut, copied);
  }
  if (msiName) {
    copyFileIfExists(path.join(msiDir, msiName), msiOut, copied);
  }

  const installCmd = writeInstallNowCmd(outDir);
  const quickstart = writeQuickstartTxt(outDir, version);

  const stageDir = path.join(outDir, `_stage-v${version}`);
  fs.rmSync(stageDir, { recursive: true, force: true });
  ensureDir(stageDir);

  for (const file of [exeOut, msiOut, installCmd, quickstart]) {
    if (fs.existsSync(file)) {
      fs.copyFileSync(file, path.join(stageDir, path.basename(file)));
    }
  }

  const zipped = compressStageZip(stageDir, zipOut);
  if (zipped) {
    copied.push(zipOut);
  }

  const checksumFile = writeChecksums(outDir, [exeOut, msiOut, zipOut]);
  copied.push(checksumFile);

  console.log("[OK] Release artifacts ready:");
  for (const file of copied) {
    console.log(`- ${file}`);
  }
}

main();
