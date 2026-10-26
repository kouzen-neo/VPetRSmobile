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
