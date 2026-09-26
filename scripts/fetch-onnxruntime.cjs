// Stage the ONNX Runtime that the Windows and Apple Silicon packages carry.
//
// The Windows and macOS counterpart of fetch-onnxruntime.sh, which does the
// same for the Linux packages and says why djmanzo carries its own copy at
// all: `ort` opens ONNX Runtime at run time, asks for C API 27 (ONNX Runtime
// 1.27 or newer), and a machine that has not installed one by hand has none
// -- or, on Windows 11, has an older one in System32 that `ort` refuses.
// Without it the stems fall back to the built-in separator, which the owner
// asked about for karaoke on exactly these two platforms.
//
// Node rather than sh because it runs on Windows, and Tauri runs
// beforeBuildCommand through cmd.exe there. Run by the beforeBuildCommand in
// crates/dj-app/tauri.onnxruntime.conf.json, which the release workflow
// passes to the Windows and Apple Silicon builds; and by CI, so the stems
// tests on those runners load a real runtime rather than skipping.
//
// Microsoft's own release builds, the version fetch-onnxruntime.sh pins, each
// refused unless its SHA-256 is the one written here. There is no Intel Mac
// build: Microsoft stopped publishing one after 1.23, which is older than
// this build accepts, so the Intel app carries none and says so.
//
//   node scripts/fetch-onnxruntime.cjs [target-triple]
//
// The target is, in order: the argument, the TAURI_ENV_TARGET_TRIPLE Tauri
// sets for its hooks (so a cross-compiled build gets the target's library,
// not the machine's), and this machine.
//
// ONNXRUNTIME_ARCHIVE=<path> uses an archive already on disk instead of
// downloading one -- for a build without the network. It is checked against
// the same checksum.

"use strict";

const { execFileSync } = require("child_process");
const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");

const VERSION = "1.28.0";

// Per target: the archive Microsoft publishes, its SHA-256 as published on
// https://github.com/microsoft/onnxruntime/releases/tag/v1.28.0, and which of
// its files djmanzo carries under which name. The names are the ones
// dj_stems::availability looks for (BUNDLED_WINDOWS, BUNDLED_MACOS); a Rust
// test holds the two together.
const BUILDS = {
  "x86_64-pc-windows-msvc": {
    archive: `onnxruntime-win-x64-${VERSION}.zip`,
    sha256: "abef733dacbe2f571547a7150b479b5cb9cc0df22f96c24983a42cadb1b4f8bc",
    files: {
      "lib/onnxruntime.dll": "onnxruntime.dll",
      // The bridge ONNX Runtime opens from its own folder for execution
      // providers built as separate libraries. Unused by the CPU provider,
      // twenty kilobytes, and carried so the build is Microsoft's, whole.
      "lib/onnxruntime_providers_shared.dll": "onnxruntime_providers_shared.dll",
    },
  },
  "aarch64-apple-darwin": {
    archive: `onnxruntime-osx-arm64-${VERSION}.tgz`,
    sha256: "1268b359718099bde2cedb55787f182a130067bc4f31e8c88478c445b850d3d8",
    files: {
      // Under the name the library gives itself (its install name is
      // @rpath/libonnxruntime.1.dylib), as the Linux copy is under its SONAME.
      [`lib/libonnxruntime.${VERSION}.dylib`]: "libonnxruntime.1.dylib",
    },
  },
};

// Carried with every copy: the licence and notices ONNX Runtime's own MIT
// licence asks to travel with it, and the version for whoever reads them.
const NOTICES = ["LICENSE", "ThirdPartyNotices.txt", "VERSION_NUMBER"];

function hostTriple() {
  const arch = { x64: "x86_64", arm64: "aarch64" }[process.arch] ?? process.arch;
  if (process.platform === "win32") return `${arch}-pc-windows-msvc`;
  if (process.platform === "darwin") return `${arch}-apple-darwin`;
  return `${arch}-unknown-linux-gnu`;
}

function fail(message) {
  console.error(`fetch-onnxruntime: ${message}`);
  process.exit(1);
}

const target = process.argv[2] || process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
const build = BUILDS[target];
if (!build) {
  const why = target.endsWith("-apple-darwin")
    ? "Microsoft publishes no ONNX Runtime 1.27 or newer for Intel Macs"
    : target.includes("linux")
      ? "the Linux packages stage theirs with scripts/fetch-onnxruntime.sh"
      : "no ONNX Runtime build is pinned for it";
  fail(`nothing to stage for ${target}: ${why}`);
}

const stage = path.resolve(__dirname, "../crates/dj-app/onnxruntime");
const wanted = [...Object.values(build.files), ...NOTICES];
const versionFile = path.join(stage, "VERSION_NUMBER");
if (
  fs.existsSync(versionFile) &&
  fs.readFileSync(versionFile, "utf8").trim() === VERSION &&
  wanted.every((name) => fs.existsSync(path.join(stage, name)))
) {
  console.log(`fetch-onnxruntime: ONNX Runtime ${VERSION} for ${target} is already staged in ${stage}`);
  process.exit(0);
}

async function main() {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "onnxruntime-"));
  try {
    let archive = process.env.ONNXRUNTIME_ARCHIVE;
    if (!archive) {
      const url = `https://github.com/microsoft/onnxruntime/releases/download/v${VERSION}/${build.archive}`;
      console.log(`fetch-onnxruntime: downloading ${url}`);
      let body;
      for (let attempt = 1; ; attempt++) {
        try {
          const response = await fetch(url);
          if (!response.ok) throw new Error(`HTTP ${response.status}`);
          body = Buffer.from(await response.arrayBuffer());
          break;
        } catch (error) {
          if (attempt === 3) fail(`could not download ${url}: ${error.message}`);
          await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
        }
      }
      archive = path.join(work, build.archive);
      fs.writeFileSync(archive, body);
    }

    const digest = crypto.createHash("sha256").update(fs.readFileSync(archive)).digest("hex");
    if (digest !== build.sha256) {
      fail(`${build.archive} has SHA-256 ${digest}, not the ${build.sha256} written in this script; refusing it`);
    }

    // bsdtar reads both formats and is `tar` on Windows 10 and later and on
    // macOS; elsewhere -- only ever a developer trying this out -- a zip
    // takes unzip.
    if (build.archive.endsWith(".zip") && process.platform !== "win32" && process.platform !== "darwin") {
      execFileSync("unzip", ["-q", archive, "-d", work], { stdio: "inherit" });
    } else {
      execFileSync("tar", ["-xf", archive, "-C", work], { stdio: "inherit" });
    }
    const unpacked = path.join(work, build.archive.replace(/\.(zip|tgz)$/, ""));

    // Only the files this copy owns are replaced, so a folder that also holds
    // another platform's staging is left as it was.
    fs.mkdirSync(stage, { recursive: true });
    for (const [from, to] of Object.entries(build.files)) {
      fs.copyFileSync(path.join(unpacked, from), path.join(stage, to));
    }
    for (const notice of NOTICES) {
      fs.copyFileSync(path.join(unpacked, notice), path.join(stage, notice));
    }
    console.log(`fetch-onnxruntime: staged ONNX Runtime ${VERSION} for ${target} in ${stage}`);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

main().catch((error) => fail(error.stack || String(error)));
