# Changelog

All notable changes to VPetRustMobile are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v1.0.0] - 2026-10-27

### Added

- **Pure Rust Native Core Engine (`vpet-android` cdylib)**:
  - 100% Rust architecture with zero .NET/Mono dependencies.
  - Exported JNI interface for lifecycle, ticks, touches, stats, feeding, and saves.
  - Direct zero-copy bitmap blitting via Android NDK `AndroidBitmap_lockPixels`.
  - Built-in `AiWorker` supporting OpenAI-compatible chat endpoints and SSE stream parsing in a dedicated native thread.
  - Deterministic 10 Hz pet simulation and crash-safe MessagePack saves with automatic backups.
- **Android Floating Overlay Window (`android/`)**:
  - Implemented `PetOverlayService` using `WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY`.
  - Persistent foreground service with notification action buttons (`Feed`, `Sleep`, `Chat`, `Close`).
  - Broadcast receiver for `ACTION_SCREEN_OFF` / `ACTION_SCREEN_ON` pausing rendering to prevent battery drain.
  - Interactive touch handling in `PetView`: drag-and-drop repositioning with `Raised` animations, head/body pet reactions, and quick action dialog.
- **Auto-Extracting Starter Asset Pack**:
  - Bundled core animations (`Default`, `IDEL`, `Touch_Head`, `Touch_Body`, `Raise`, `Sleep`, `Say`, `Eat`, `Drink`, `vup.lps`, `food`, `lang`) in `assets/0000_core.zip`.
  - Automatically unpacks to app private storage on first launch.
- **Signed Release APK**:
  - Self-contained signed release build `vpet-mobile.apk`.
  - Script `scripts/send-to-poco.sh` for KDE Connect transfer to Poco F5.
