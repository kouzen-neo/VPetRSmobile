# VPetRSmobile

A Rust rewrite of VPet-Simulator, built Android-first as a floating system overlay.

VPetRSmobile is a from-scratch reimplementation of [LorisYounger/VPet](https://github.com/LorisYounger/VPet) (the C#/WPF virtual desktop pet) built for Android. The simulation core, LinePutScript parser, animation state machine, `tiny-skia` compositor, and AI chat worker are written in **100% Rust** (`crates/`), paired with a lightweight Kotlin shell that hosts a persistent floating overlay (`SYSTEM_ALERT_WINDOW`). It has **zero .NET / CoreCLR / Mono / Avalonia dependencies**. It reads the original VPet content packs directly (`.lps` LinePutScript data and PNG animation folders), so you can use the complete `0000_core` assets and data mods without running any C# runtime.

> **Note:** VPetRSmobile is an independent community project. It is not an official VPet product and is not affiliated with or endorsed by the VPet authors. VPet artwork is not included in this repository (see [License & Asset Terms](#license--asset-terms)).

---

## Why an Android floating rewrite?

On mobile devices, running virtual pets or companion apps inside standard Android activity windows has severe drawbacks:

- **Locked to a single screen:** the pet disappears as soon as you switch apps, open a browser, or play a game;
- **Task clearing kills the companion:** swiping the app away from Android's recents tray terminates the process;
- **Heavy memory & battery footprint:** desktop WPF or Avalonia ports need heavy cross-platform runtimes (Mono/CoreCLR) that drain battery on ARM;
- **No true system overlay:** without a low-level overlay, the pet cannot wander freely across the screen bezel or react to global gestures.

VPetRSmobile solves this by decoupling the native Rust engine from the UI, rendering via a background `ForegroundService` on a transparent `TYPE_APPLICATION_OVERLAY`:

| Behavior | Standard Android Pet Apps | VPetRSmobile Floating Overlay |
|---|---|---|
| **Persistence** | Closes when switching apps | Floats across all apps, games, and the launcher (`stopWithTask = false`) |
| **System Overlay** | Confined inside an activity layout | Free movement across screen boundaries with `FLAG_LAYOUT_IN_SCREEN` |
| **Animation Engine** | WebView or high-overhead GUI | Native Rust `tiny-skia` CPU blit with zero-copy DirectBitmap presentation |
| **Edge Physics** | Bounces inside a rectangle | Wall climbing, ceiling crawling, gravity drops, and bezel SideHide |
| **Battery Life** | Heavy background runtime | Deterministic 10 Hz simulation; paused + autosaved on screen-off |
| **Runtime Footprint** | ~500 MB+ (.NET / Mono / Avalonia) | Lightweight native `libvpet_mobile.so` (~8.6 MB); zero .NET dependencies |

---

## Features

- **Native Floating Overlay:** Runs via `WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY` and a sticky `ForegroundService`. Persists when the settings app is cleared from recents.
- **100% Pure Rust Core:** Simulation, digestion, stat degradation, leveling, LPS parsing, animation caching, and LLM chat workers are powered by modular Rust crates (`vpet-core`, `vpet-content`, `vpet-render`, `vpet-android`).
- **Full VPet Content Compatibility:**
  - Localized catalog of all **151 foods, drinks, snacks, medicines, and gifts** with English titles and original item artwork.
  - All **13 work, study, and play activities** exposed dynamically with live countdown badges and reward payouts.
  - Complete support for all **25 animation categories**: `Default`, `Move`, `Idel`, `TouchHead`, `TouchBody`, `Sleep`, `Say`, `StateOne`, `StateTwo`, `StartUP`, `Shutdown`, `Work`, `SwitchUp`, `SwitchDown`, `SwitchHunger`, `SwitchThirsty`, `SideHide_Left_Main/Rise`, `SideHide_Right_Main/Rise`, `Pinch`, `Raise`, `Drink`, `Eat`, `Gift`, `Music`, `BDay`.
- **Natural Touch Gestures & Hit Zones:**
  - Head petting (`HitZone::Head`) and body rubbing (`HitZone::Body`) classified against upstream `vup.lps` hitboxes with localized speech bubble lines.
  - Cheek pinch gesture (`HitZone::Pinch`): pinching the pet's cheek triggers the animated cheek-pull reaction.
  - Drag & dangle physics: dragging plays `Raised_Dynamic` swinging and kicking; holding still for 1.4 seconds relaxes the pet into `Raised_Static` blinking; moving the finger resumes the kicking animation.
  - Ceiling climbing & falling: dropping the pet near the top of the screen triggers ceiling crawling (`climb.top.left` / `climb.top.right`); letting go drops the pet with gravity until landing on the floor.
  - Wall climbing & SideHide: auto-walking to the screen edges triggers wall climbing (`climb.left` / `climb.right`) or turns around; dragging and dropping the pet against the left or right bezel snaps into `SideHide` peek-a-boo mode. Tap to emerge.
- **In-Overlay Floating Menu:**
  - Long-press the pet to summon a tabbed overlay card right on screen:
    - **Chat:** Talk with the pet via AI, read speech bubble answers, or run dev cheats (`unlockall`).
    - **Food:** Browse and purchase from the complete 151-item food store, categorized with icons and stat tooltips.
    - **Work:** Start any of the 13 work, study, or play tasks with real-time countdown badges.
    - **Sleep:** Put the pet to bed to restore stamina or wake her up.
- **Live Scale Slider:** Resize the pet dynamically from 160 px to 500 px with instantaneous overlay layout adjustment and font scaling.
- **AI Chat (LLM):** Background non-blocking worker thread compatible with OpenAI, Ollama, DeepSeek, vLLM, and LM Studio endpoints (defaults to `http://localhost:20128/v1` or `http://10.0.2.2:20128/v1`). Supports standard JSON and streaming SSE chunks with speech bubble responses.
- **Crash-Safe Saves & Screen-Off Efficiency:**
  - Saves are serialized atomically with 5 rotating backups (`*.mpak`, `*.bak.1`–`*.bak.5`).
  - Automatically saves and pauses frame ticks on `ACTION_SCREEN_OFF` to eliminate background battery drain when the screen is locked.
- **Zero .NET:** Verified by quality gates: no Mono, CoreCLR, Avalonia, or C# assemblies.

---

## Requirements

- Android 8.0+ (API level 26 or higher)
- Architecture: `arm64-v8a` (64-bit ARM)
- Rust 1.89+ (stable) and `cargo-ndk`
- Android NDK r26b+ (auto-detected via `$ANDROID_NDK_HOME`, `$ANDROID_SDK_ROOT`, or `~/Android/Sdk`)
- The VPet `0000_core` content directory (provided by you or downloaded via helper script)

---

## Getting Started

### 1. Fetch VPet Core Assets

The `0000_core` content pack contains the animations, artwork, and localized texts. It is kept outside the git tree. Choose any of the following:

**Reusing an existing local clone or Steam install (instant, no download):**
```bash
./scripts/fetch-assets.sh --from /path/to/VPet/VPet-Simulator.Windows/mod/0000_core
```

**Downloading directly from upstream (sparse git checkout, ~1.1 GB):**
```bash
./scripts/fetch-assets.sh
```

Assets are placed at `~/.local/share/vpet-rs/content/0000_core` (shared with desktop VPetRS). Override with `VPET_CORE_DIR` or `--dest`.

### 2. Build the APK

The build script automatically detects your Android NDK, verifies assets, compiles the Rust crate for `aarch64-linux-android`, bundles `0000_core.zip`, and produces a release APK:

```bash
./scripts/build-apk.sh
# Output: vpet-mobile.apk (and android/app/build/outputs/apk/release/app-release.apk)
```

Run the quality gate locally:
```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-no-dotnet.sh
```

---

## Touch Gestures & Controls

| Action | Gesture | Result |
|---|---|---|
| **Pet Head** | Single tap on the head | Plays `TouchHead`, grants feeling & exp, shows speech bubble |
| **Rub Body** | Single tap on the body | Plays `TouchBody`, grants likability & feeling |
| **Pinch Cheek** | Single tap on the cheek | Triggers `Pinch` ("Don't pinch my cheeks!") |
| **Drag Pet** | Press and move finger | Kicks feet in the air (`RaisedDynamic`) |
| **Hold Still in Air** | Hold finger still for 1.4s | Relaxes into `RaisedStatic` blinking; resumes kicking on movement |
| **Ceiling Crawl** | Drop near top screen edge | Sticks to ceiling and crawls across (`climb.top.left/right`) |
| **Drop & Land** | Release anywhere in the air | Falls under gravity and lands on the floor (`fall.*` -> `Idle`) |
| **Wall Climb** | Auto-walk reaches bezel | Climbs up the side screen border (`climb.left/right`) |
| **SideHide** | Drag and drop against left/right bezel | Hides behind screen bezel with only eyes/ears visible. Tap to peek out |
| **Open Menu** | Long press on the pet | Opens floating popup card (Chat, Food, Work, Sleep) |
| **Dev Cheat** | Send `unlockall` in chat tab | Maxes level, money, and stats for debugging |

---

## Project Layout

| Component | Responsibility |
|---|---|
| `crates/core` | Pure deterministic simulation: stats, buffs, leveling, digestion, GameSave formulas (zero I/O) |
| `crates/content` | LPS parser, animation indexer, 151 localized foods, 13 work activities, text databases |
| `crates/render` | `tiny-skia` CPU compositing, frame decoders, LRU cache, `cosmic-text` chat bubble rasterizer |
| `crates/android` | JNI bridge (`libvpet_mobile.so`), `MobileEngine`, state machine, touch gesture classifier |
| `android/` | Minimal Kotlin shell: `PetOverlayService`, `PetView` overlay window, `MainActivity` settings |
| `scripts/` | `build-apk.sh`, `fetch-assets.sh`, and `check-no-dotnet.sh` verification gate |

---

## Credits

- **[VPet-Simulator](https://github.com/VPet-Simulator/VPet)** by LorisYounger and contributors: the original project, game design, data formats, and artwork. VPetRSmobile is an independent pure-Rust reimplementation for Android.
- Built with [tiny-skia](https://github.com/RazrFalcon/tiny-skia), [cosmic-text](https://github.com/pop-os/cosmic-text), [image](https://github.com/image-rs/image), and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk).

---

## License & Asset Terms

- **VPetRSmobile source code** is dual-licensed under [MIT](LICENSE-MIT) **or** [Apache-2.0](LICENSE-APACHE), at your option.
- **VPet assets** (`0000_core` artwork, animations, and text) are **not** included in this repository. They belong to the upstream [VPet](https://github.com/LorisYounger/VPet) project and are governed by its terms: attribution is required, they may not be sold, and commercial use requires permission from the original authors.
