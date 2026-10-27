#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "=== 1. Building Rust native library with cargo-ndk ==="
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-/home/kouzen/Android/Sdk/ndk/26.3.11579264}"
cargo ndk -t arm64-v8a build --release -p vpet-android

echo "=== 2. Syncing native .so to Android jniLibs ==="
mkdir -p "$ROOT/android/app/src/main/jniLibs/arm64-v8a"
cp "$ROOT/target/aarch64-linux-android/release/libvpet_mobile.so" \
   "$ROOT/android/app/src/main/jniLibs/arm64-v8a/"

echo "=== 3. Checking starter assets ==="
echo "Packaging complete assets into 0000_core.zip..."
python3 -c "
import os, zipfile
core_dir = '/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core'
out_zip = '$ROOT/android/app/src/main/assets/0000_core.zip'
os.makedirs(os.path.dirname(out_zip), exist_ok=True)
included_vup = {
    'Default', 'IDEL', 'Touch_Head', 'Touch_Body', 'Pinch', 'Raise',
    'Sleep', 'Say', 'Eat', 'Drink', 'State', 'WORK', 'MOVE',
    'StartUP', 'Switch', 'Think', 'LevelUP',
    'SideHide_Left_Main', 'SideHide_Left_Rise', 'SideHide_Right_Main', 'SideHide_Right_Rise'
}
with zipfile.ZipFile(out_zip, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as z:
    for root, dirs, files in os.walk(core_dir):
        rel = os.path.relpath(root, core_dir)
        parts = rel.split(os.sep)
        if parts[0] == 'pet' and len(parts) >= 2 and parts[1] == 'vup':
            if len(parts) > 2 and parts[2] not in included_vup:
                continue
        elif parts[0] in {'file', 'photo'}:
            continue
        for f in files:
            full_path = os.path.join(root, f)
            rel_path = os.path.relpath(full_path, core_dir)
            if parts[0] == 'pet' and len(parts) == 2 and parts[1] == 'vup' and f not in included_vup and f != 'vup.lps':
                continue
            z.write(full_path, rel_path)
print('Assets packaged successfully into 0000_core.zip')
"

echo "=== 4. Assembling Release APK with Gradle ==="
cd "$ROOT/android"
./gradlew assembleRelease

APK_OUT="$ROOT/android/app/build/outputs/apk/release/app-release.apk"
if [ -f "$APK_OUT" ]; then
    cp "$APK_OUT" "$ROOT/vpet-mobile.apk"
    echo "Build complete! APK ready at: $ROOT/vpet-mobile.apk"
fi
