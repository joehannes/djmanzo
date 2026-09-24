#!/bin/sh
# Stage the ONNX Runtime that the Linux packages carry.
#
# djmanzo separates stems with an ONNX model through the `ort` crate, which
# opens ONNX Runtime at run time rather than linking it (`load-dynamic`). A
# package that does not carry the library leaves the DJ with a stem feature
# that says "ONNX Runtime could not be loaded" on every machine that has not
# installed one by hand -- and the one a distribution offers is the wrong
# version: `ort` 2.0.0-rc.13 asks for C API 27, which is ONNX Runtime 1.27 or
# newer, and Ubuntu 26.04 carries 1.23.
#
# So the .deb, the .rpm and the AppImage carry Microsoft's own release build of
# the version `ort` was built against, as a private library in
# /usr/lib/djmanzo/, and dj_stems::availability finds it beside the executable.
# See docs/RESEARCH.md for why this and not a Debian dependency.
#
# Run by Tauri's beforeBuildCommand on Linux (crates/dj-app/tauri.linux.conf.json),
# so `tauri build` on a fresh checkout produces a complete package. It only
# downloads when the staged copy is missing or another version, and it refuses
# a download whose checksum is not the one written here.
set -eu

VERSION=1.28.0
# SHA-256 of onnxruntime-linux-x64-1.28.0.tgz as published on
# https://github.com/microsoft/onnxruntime/releases/tag/v1.28.0
SHA256=a3e1b79d7bb1bf09696ce675f49e4064e6c81f6202b8225624fff0e93f8d6407
NAME="onnxruntime-linux-x64-$VERSION"
URL="https://github.com/microsoft/onnxruntime/releases/download/v$VERSION/$NAME.tgz"

case "$(uname -m)" in
  x86_64 | amd64) ;;
  *)
    echo "fetch-onnxruntime: only x86_64 Linux packages carry ONNX Runtime so far; $(uname -m) is not one" >&2
    exit 1
    ;;
esac

here="$(cd "$(dirname "$0")" && pwd)"
stage="$here/../crates/dj-app/onnxruntime"

if [ -f "$stage/VERSION_NUMBER" ] && [ "$(cat "$stage/VERSION_NUMBER")" = "$VERSION" ] \
  && [ -f "$stage/libonnxruntime.so.1" ] && [ -f "$stage/libonnxruntime_providers_shared.so" ]; then
  echo "fetch-onnxruntime: ONNX Runtime $VERSION is already staged in $stage"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "fetch-onnxruntime: downloading $URL"
curl --fail --location --silent --show-error --retry 3 --output "$work/$NAME.tgz" "$URL"
echo "$SHA256  $work/$NAME.tgz" | sha256sum --check --status || {
  echo "fetch-onnxruntime: $NAME.tgz does not have the checksum written in this script; refusing it" >&2
  exit 1
}
tar -xzf "$work/$NAME.tgz" -C "$work"

rm -rf "$stage"
mkdir -p "$stage"
# Under its SONAME, the name the library gives itself, so the file djmanzo
# opens is the one a loader would look for; its version is in VERSION_NUMBER.
cp "$work/$NAME/lib/libonnxruntime.so.$VERSION" "$stage/libonnxruntime.so.1"
# The bridge ONNX Runtime opens from its own directory for execution providers
# built as separate libraries. Unused by the CPU provider djmanzo runs on, and
# fourteen kilobytes; carried so the build is Microsoft's, whole.
cp "$work/$NAME/lib/libonnxruntime_providers_shared.so" "$stage/"
cp "$work/$NAME/LICENSE" "$work/$NAME/ThirdPartyNotices.txt" "$work/$NAME/VERSION_NUMBER" "$stage/"
chmod 0644 "$stage"/*
echo "fetch-onnxruntime: staged ONNX Runtime $VERSION in $stage"
