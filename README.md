# VPetRustMobile

A pure Rust mobile port and companion of [VPet-Simulator](https://github.com/VPet-Simulator/VPet) for Android with **Floating Overlay Window** (`SYSTEM_ALERT_WINDOW`).

## Architecture

- **100% Rust Core:** Simulation, digestion, leveling, LPS parsing, animation caching, and AI chat worker are powered by native Rust crates.
- **Floating Overlay:** Runs on top of all applications via Android `WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY` and a persistent `ForegroundService`.
- **Zero .NET:** Completely free of CoreCLR, Mono, Avalonia, or C# runtime dependencies.

## Project Structure

```text
VPetRustMobile/
├── android/          # Android Studio / Gradle project (Kotlin shell)
├── crates/
│   ├── android/      # JNI bridge (cdylib exporting to Kotlin)
│   ├── content/      # LPS parser and animation indexer
│   ├── core/         # Headless deterministic simulation
│   └── render/       # tiny-skia CPU frame renderer
├── scripts/          # Quality gate & build scripts
├── CHANGELOG.md      # SemVer changelog
├── IMPLEMENTATION.md # Architecture Decision Records & design
└── TODO.md           # Work board & phase checklist
```

## Requirements

- Android 8.0+ (API 26+)
- Rust 2024 edition + `cargo-ndk`
- Android NDK r26b+
- Android target: `aarch64-linux-android`

## Assets

The VPet `0000_core` artwork, animations, and text are **not** part of this
repository. They belong to the upstream [VPet](https://github.com/LorisYounger/VPet)
project, so they are downloaded on demand and kept outside git:

```bash
# Reuse your existing VPet installation (fastest, no download)
./scripts/fetch-assets.sh --from ~/path/to/VPet-Simulator.Windows/mod/0000_core

# Or reuse a Steam install / download ~1.1 GB of assets from upstream
./scripts/fetch-assets.sh
```

The default location is `~/.local/share/vpet-rs/content/0000_core`, shared with
the desktop VPetRS build. Override it with `VPET_CORE_DIR` or `--dest`.
`scripts/build-apk.sh` calls this automatically when the folder is missing, then
packages the content into `android/app/src/main/assets/0000_core.zip`, which the
app extracts on first launch. The folder `file/` and `photo/` are desktop-only
and are skipped.

Never commit `0000_core.zip`, extracted content, or a built APK (`*.zip` and
`*.apk` are already ignored).

## Building

```bash
# NDK is auto-detected from $ANDROID_NDK_HOME, $ANDROID_SDK_ROOT, or ~/Android/Sdk
./scripts/build-apk.sh
```

The script resolves the assets, builds `libvpet_mobile.so` with `cargo-ndk`,
assembles the signed release APK, and copies it to `vpet-mobile.apk`. Output is
`android/app/build/outputs/apk/release/app-release.apk`.

