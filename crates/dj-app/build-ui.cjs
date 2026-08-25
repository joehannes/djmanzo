// Build the interface, for Tauri's beforeBuildCommand / beforeDevCommand.
//
// A script rather than a one-liner because the shells differ: `npm --prefix`
// behaves differently on Windows, and `cd ui && npm run build` needs a
// different separator in cmd.exe. Resolving the path in Node sidesteps both.
//
// Tauri runs this with the working directory set to `crates/` -- the parent of
// the directory holding tauri.conf.json -- which is why the config invokes it
// as `dj-app/build-ui.cjs`. That was five commits of trial and error to
// establish; it is written down here so it is not rediscovered.
//
// Path resolution does not depend on that, though: `__dirname` is always this
// script's own directory, so the interface is found wherever Tauri decides to
// stand.

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const cmd = process.argv[2] || 'build';
const uiDir = path.resolve(__dirname, '../../ui');

// A missing interface is a broken checkout, not a shortcut.
//
// This used to `process.exit(0)` with "assuming it's already built or not
// needed", which is the one thing it must never do: tauri-build would then
// package whatever stale `ui/dist` happened to be lying about, or nothing at
// all, and the failure would surface as a blank window on somebody else's
// machine rather than as an error here.
if (!fs.existsSync(uiDir)) {
  console.error(`No interface at ${uiDir}. This is not a djmanzo checkout, or it is incomplete.`);
  process.exit(1);
}

console.log(`Running npm run ${cmd} in ${uiDir}`);

try {
  // npm is a shell script on Windows, so it needs the .cmd shim.
  const npmCmd = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  execSync(`${npmCmd} run ${cmd}`, { cwd: uiDir, stdio: 'inherit' });
} catch (e) {
  console.error(`Failed to run npm run ${cmd}:`, e.message);
  process.exit(1);
}
