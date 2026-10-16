// Build `djmanzo-listen` for the karaoke app: whisper.cpp in a helper of its
// own, beside an application built without it.
//
// The karaoke app is compiled for macOS 10.13 so a 2011 Mac can open it, and
// whisper.cpp needs 10.15's C++ library. So the helper is compiled apart, for
// 10.15, into a target folder of its own (the application's dependencies are
// compiled for 10.13, and one folder holding both would rebuild them back and
// forth), and copied to where `bundle.externalBin` in
// tauri.karaoke.conf.json says Tauri will find it: `binaries/`, with the
// target triple Tauri expects on its name. At run time the application asks
// the helper whether it runs on this Mac — see `dj_app::listening`.
//
// Run by Tauri as part of the karaoke config's beforeBuildCommand, after the
// interface, with the working directory set to `crates/`.

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const repo = path.resolve(__dirname, '../..');
const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  execSync('rustc -vV').toString().match(/^host: (.+)$/m)[1].trim();
const exe = triple.includes('windows') ? '.exe' : '';
const targetDir = path.join(repo, 'target', 'listen');

console.log(`Building djmanzo-listen for ${triple}, for macOS 10.15 and later`);
execSync(
  `cargo build --release -p dj-whisper --bin djmanzo-listen --target ${triple}`,
  {
    cwd: repo,
    stdio: 'inherit',
    env: {
      ...process.env,
      // Over the 10.13 the karaoke job compiles the application for.
      MACOSX_DEPLOYMENT_TARGET: '10.15',
      CARGO_TARGET_DIR: targetDir,
    },
  },
);

const built = path.join(targetDir, triple, 'release', `djmanzo-listen${exe}`);
const into = path.join(__dirname, 'binaries');
fs.mkdirSync(into, { recursive: true });
const placed = path.join(into, `djmanzo-listen-${triple}${exe}`);
fs.copyFileSync(built, placed);
fs.chmodSync(placed, 0o755);
console.log(`Placed ${placed}`);
