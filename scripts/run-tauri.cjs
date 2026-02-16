const { spawnSync } = require("child_process");
const { existsSync } = require("fs");
const { homedir } = require("os");
const { delimiter, join } = require("path");

const args = process.argv.slice(2);
const env = { ...process.env };

if (process.platform === "win32") {
  // Windows env can expose PATH as "Path"; keep both keys in sync.
  const basePath = env.PATH || env.Path || "";
  const cargoBin = join(homedir(), ".cargo", "bin");
  const cargoExe = join(cargoBin, "cargo.exe");
  if (existsSync(cargoExe)) {
    const mergedPath = `${cargoBin}${delimiter}${basePath}`;
    env.PATH = mergedPath;
    env.Path = mergedPath;
  } else if (basePath) {
    env.PATH = basePath;
    env.Path = basePath;
  }
}

function resolveTauriCommand() {
  const cwd = process.cwd();
  const localJs = join(cwd, "node_modules", "@tauri-apps", "cli", "tauri.js");
  if (existsSync(localJs)) {
    return { command: process.execPath, args: [localJs, ...args] };
  }

  const localBin = process.platform === "win32"
    ? join(cwd, "node_modules", ".bin", "tauri.cmd")
    : join(cwd, "node_modules", ".bin", "tauri");
  if (existsSync(localBin)) {
    return { command: localBin, args };
  }

  const npx = process.platform === "win32" ? "npx.cmd" : "npx";
  return { command: npx, args: ["--yes", "tauri", ...args] };
}

const launch = resolveTauriCommand();
const result = spawnSync(launch.command, launch.args, {
  stdio: "inherit",
  shell: false,
  env
});

if (result.error) {
  console.error(`[ERROR] Failed to launch tauri: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
