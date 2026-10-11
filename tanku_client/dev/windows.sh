#!/usr/bin/env bash
# Runs the build for Windows on a Linux machine, under wine: a scripted game, or the tests.
#
# It tells whether the game starts, draws and computes on Windows as far as wine can tell.
# Wine is not Windows: a window, a sound device, the keys and a firewall are only checked on
# a real one.
#
# No window opens, no sound plays and nothing is written outside `target`: wine is given no
# display and no sound driver, and a prefix of its own in `target/windows`. It takes the
# compiler of mingw-w64 (`x86_64-w64-mingw32-gcc`) and a wine that has Vulkan.
#
# Usage: windows.sh SCRIPT
#        TESTS=1 windows.sh
#
# SCRIPT is a development script, as `TANKU_DEV_SCRIPT` takes it. A path in it is as wine sees
# it: one without a drive is under the directory this is called from.
#
#   tanku_client/dev/windows.sh "start TANKU 1; wait 2; shot target/windows/solo.png; quit"
#
# With TESTS=1, the tests of the rules, of the sound and of the simulation run as Windows
# programs instead: a seed must give the same level there, and the music the same sound.
set -euo pipefail
cd "$(dirname "$0")/../.."
[ $# -ge 1 ] || [ "${TESTS:-}" = 1 ] || { echo "Usage: windows.sh SCRIPT, or TESTS=1 windows.sh"; exit 1; }

out=target/windows
mkdir -p "$out"
export WINEPREFIX="$PWD/$out/prefix" WINEDEBUG=-all
# Nothing in the menus of the desktop, no browser and no .NET to install, and no sound.
export WINEDLLOVERRIDES="winemenubuilder.exe=d;mscoree=d;mshtml=d;winepulse.drv=d;winealsa.drv=d;wineoss.drv=d"
headless() { env -u WAYLAND_DISPLAY -u DISPLAY "$@"; }
# Only the wine of this prefix is told to end.
trap 'headless wineserver -k 2>/dev/null || true' EXIT

target=x86_64-pc-windows-gnu
if [ "${TESTS:-}" = 1 ]; then
    # A development build links the engine dynamically, which crashes on Windows, so the
    # tests of the server leave the development features out. Cranelift is not tried there.
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm
    headless cargo test -p tanku_core -p tanku_audio --target "$target"
    headless cargo test -p tanku_server --no-default-features --features net --target "$target"
    exit
fi

cargo build -q -p tanku_client --release --no-default-features --features script --target "$target"
# The build that takes scripts is kept apart from the one for players, whose place it took.
cp "target/$target/release/tanku_client.exe" "$out/tanku_script.exe"
headless env TANKU_DEV_SCRIPT="$1" timeout 600 wine "$out/tanku_script.exe"
echo "target/$target/release/tanku_client.exe is now the build that takes scripts."
echo "Build for players again before handing it out:"
echo "  cargo build --release --no-default-features -p tanku_client --target $target"
