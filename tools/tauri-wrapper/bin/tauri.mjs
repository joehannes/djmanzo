#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");
const app = resolve(root, "crates/dj-app");
const input = process.argv.slice(2);

const aliases = new Map([
  ["build:deb", ["build", "--bundles", "deb"]],
  ["build:dmg", ["build", "--bundles", "dmg"]],
  ["build:app", ["build", "--bundles", "app"]],
  ["build:appimage", ["build", "--bundles", "appimage"]],
  ["build:linux", ["build", "--bundles", "deb,appimage"]],
  ["build:macos", ["build", "--bundles", "dmg"]],
]);

const args = aliases.has(input[0]) ? [...aliases.get(input[0]), ...input.slice(1)] : input;

if (args.length === 0 || args.includes("help") || args.includes("--help") || args.includes("-h")) {
  console.log(`djmanzo Tauri wrapper\n\nUsage:\n  npx tauri build:deb       Build target/release/bundle/deb/*.deb\n  npx tauri build:dmg       Build target/release/bundle/dmg/*.dmg on macOS\n  npx tauri build:appimage  Build target/release/bundle/appimage/*.AppImage\n  npx tauri build:linux     Build deb and AppImage bundles\n  npx tauri build:macos     Build a dmg bundle on macOS\n\nAll other arguments pass through to: cargo tauri <args>\nThe command runs from crates/dj-app so tauri.conf.json relative paths stay correct.`);
  process.exit(0);
}

const available = spawnSync("cargo", ["tauri", "--version"], {
  cwd: app,
  stdio: "ignore",
  env: process.env,
});

if (available.status !== 0) {
  console.error("cargo-tauri is not installed. Install it once with:");
  console.error("  cargo install tauri-cli --version '^2'");
  process.exit(1);
}

const result = spawnSync("cargo", ["tauri", ...args], {
  cwd: app,
  stdio: "inherit",
  env: process.env,
});

if (result.error) {
  console.error(result.error.message);
  console.error("Install the Tauri Cargo subcommand once with: cargo install tauri-cli --version '^2'");
  process.exit(1);
}
process.exit(result.status ?? 1);
