const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const cmd = process.argv[2] || 'build';

// __dirname is the directory of this script (crates/dj-app)
let uiDir = path.resolve(__dirname, '../../ui');

if (!fs.existsSync(uiDir)) {
  console.log(`Could not find ui directory at ${uiDir}.`);
  
  // Attempt to find it relative to CWD just in case
  let altUiDir = path.resolve(process.cwd(), '../../ui');
  if (fs.existsSync(altUiDir)) {
    uiDir = altUiDir;
  } else {
    console.log("UI directory not found. Assuming it's already built or not needed.");
    process.exit(0);
  }
}

console.log(`Running npm run ${cmd} in ${uiDir}`);

try {
  // on Windows, npm might need to be npm.cmd
  const npmCmd = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  execSync(`${npmCmd} run ${cmd}`, { cwd: uiDir, stdio: 'inherit' });
} catch (e) {
  console.error(`Failed to run npm run ${cmd}:`, e.message);
  process.exit(1);
}
