#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ---------------------------------------------------------------------------
# Toolchain discovery (no machine-specific paths baked into the repository)
# ---------------------------------------------------------------------------
if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    SDK_DIR="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"
    if [[ -d "$SDK_DIR/ndk" ]]; then
        ANDROID_NDK_HOME="$SDK_DIR/ndk/$(ls -1 "$SDK_DIR/ndk" | sort -V | tail -1)"
        export ANDROID_NDK_HOME
    fi
fi
if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "error: Android NDK not found. Set ANDROID_NDK_HOME to your NDK directory." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 1. VPet 0000_core content pack (never committed; fetched or reused locally)
# ---------------------------------------------------------------------------
CORE_DIR="${VPET_CORE_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/vpet-rs/content/0000_core}"

echo "=== 1. Resolving VPet 0000_core assets ==="
if [[ ! -f "$CORE_DIR/info.lps" ]]; then
    ./scripts/fetch-assets.sh --dest "$CORE_DIR"
fi
if [[ ! -f "$CORE_DIR/info.lps" || ! -d "$CORE_DIR/pet/vup" ]]; then
    echo "error: $CORE_DIR does not look like a VPet 0000_core folder." >&2
    echo "       Pass --from /path/to/0000_core to scripts/fetch-assets.sh." >&2
    exit 1
fi
echo "Using assets from: $CORE_DIR"

# ---------------------------------------------------------------------------
# 2. Build and stage the Rust native library
# ---------------------------------------------------------------------------
echo "=== 2. Building Rust native library with cargo-ndk ==="
cargo ndk -t arm64-v8a build --release -p vpet-android

echo "=== 3. Syncing native .so to Android jniLibs ==="
mkdir -p "$ROOT/android/app/src/main/jniLibs/arm64-v8a"
cp "$ROOT/target/aarch64-linux-android/release/libvpet_mobile.so" \
   "$ROOT/android/app/src/main/jniLibs/arm64-v8a/"

# ---------------------------------------------------------------------------
# 3. Package the starter assets consumed by MainActivity on first launch
# ---------------------------------------------------------------------------
echo "=== 4. Packaging assets into 0000_core.zip ==="
python3 - "$CORE_DIR" "$ROOT/android/app/src/main/assets/0000_core.zip" <<'PY'
import os
import sys
import zipfile

core_dir, out_zip = sys.argv[1], sys.argv[2]
os.makedirs(os.path.dirname(out_zip), exist_ok=True)

# `file` and `photo` hold desktop-only mod payloads and are not used on Android.
SKIP_TOP_LEVEL = {"file", "photo"}
count = 0
with zipfile.ZipFile(out_zip, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
    for root, _dirs, files in os.walk(core_dir):
        rel_root = os.path.relpath(root, core_dir)
        if rel_root.split(os.sep)[0] in SKIP_TOP_LEVEL:
            continue
        for name in files:
            full_path = os.path.join(root, name)
            zf.write(full_path, os.path.relpath(full_path, core_dir))
            count += 1

size_mb = os.path.getsize(out_zip) / (1024 * 1024)
print(f"Packaged {count} files ({size_mb:.1f} MiB) into {out_zip}")
PY

# ---------------------------------------------------------------------------
# 4. Assemble the signed release APK
# ---------------------------------------------------------------------------
echo "=== 5. Assembling Release APK with Gradle ==="
cd "$ROOT/android"
./gradlew assembleRelease

APK_OUT="$ROOT/android/app/build/outputs/apk/release/app-release.apk"
if [ -f "$APK_OUT" ]; then
    cp "$APK_OUT" "$ROOT/vpet-mobile.apk"
    echo "Build complete! APK ready at: $ROOT/vpet-mobile.apk"
fi
