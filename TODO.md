# VPetRustMobile — TODO & Rencana Kerja

Dokumen ini adalah papan kerja proyek VPetRustMobile (Android Floating Pet).
`IMPLEMENTATION.md` menjelaskan *arsitektur* dan *mengapa*; dokumen ini menjelaskan *urutan pelaksanaan*, *kriteria selesai*, dan *commit* untuk setiap langkah.

**Status:** Perancangan & Bootstrap repo (`v0.0.1`).  
**Terakhir diperbarui:** 2026-10-27

---

## 1. Aturan Commit

Repo ini memakai **satu commit per fase** (atau sub-commit berstruktur logis dalam fase). Setiap commit harus:

- Mengikuti [Conventional Commits](https://www.conventionalcommits.org/):
  `type(scope): subject`
  - `type`: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`, `build`, `ci`
  - `scope`: `core`, `content`, `render`, `android`, `overlay`, `ai`, `packaging`, `docs`
- Subject ≤ 72 karakter, imperative, tanpa titik di akhir.
- Body berupa bullet list ringkas tentang apa yang berubah.
- **Tanpa** `Co-Authored-By` atau trailer atribusi apa pun.
- Lolos quality gate sebelum commit:
  ```bash
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ./scripts/check-no-dotnet.sh
  ```
- Memperbarui `CHANGELOG.md` di bawah `## [Unreleased]`.
- Mencetak kotak progres di dokumen ini.

---

## 2. Papan Progres

| Fase | Nama | Status | Tag |
|---|---|---|---|
| 0 | Bootstrap repo, workspace & docs | `[x]` selesai | `v0.0.1` |
| 1 | Native NDK bridge & JNI bindings (`vpet-android`) | `[ ]` belum | `v0.1.0` |
| 2 | Android Foreground Service & Floating Overlay | `[ ]` belum | `v0.2.0` |
| 3 | Content loader & frame rendering di Android Canvas | `[ ]` belum | `v0.3.0` |
| 4 | Sentuhan interaktif (drag/drop, elus kepala/badan) | `[ ]` belum | `v0.4.0` |
| 5 | Simulasi state, stat bar & balon percakapan | `[ ]` belum | `v0.5.0` |
| 6 | AI Chat (LLM) & Quick Action Wheel | `[ ]` belum | `v0.6.0` |
| 7 | Optimasi baterai, low-memory test & APK release | `[ ]` belum | `v1.0.0` |

Legenda status: `[ ]` belum, `[~]` dikerjakan, `[x]` selesai, `[!]` terblokir.

---

## 3. Commit Map

| Fase | Commit | Tag |
|---|---|---|
| 0 | `chore: bootstrap workspace, documentation, and android skeleton` | `v0.0.1` |
| 1 | `feat(android): implement native jni bridge with jni-rs`<br>`feat(build): configure cargo-ndk for aarch64 and x86_64 targets`<br>`test(android): verify jni lifecycle and state initialization` | `v0.1.0` |
| 2 | `feat(overlay): implement foreground service with persistent notification`<br>`feat(overlay): create system alert window with type application overlay`<br>`feat(overlay): handle screen on/off broadcast to pause rendering` | `v0.2.0` |
| 3 | `feat(content): adapt lps parser and animation indexer for android storage`<br>`feat(render): blit decoded tiny-skia frames to android bitmap via ndk`<br>`perf(render): throttle animation ticks to 15 fps during idle` | `v0.3.0` |
| 4 | `feat(input): detect touch down, drag gesture, and hit zones`<br>`feat(input): update window manager layout coordinates during drag`<br>`feat(anim): trigger raised dynamic and drop landing animations` | `v0.4.0` |
| 5 | `feat(core): tick deterministic simulation in background worker`<br>`feat(render): render anti-aliased speech bubbles with cosmic-text`<br>`feat(core): persist pet save states atomically to internal storage` | `v0.5.0` |
| 6 | `feat(ai): integrate background http worker for openai and 9router chat`<br>`feat(ui): add quick radial action wheel for feeding and commands`<br>`feat(ui): implement settings activity for scale and overlay permissions` | `v0.6.0` |
| 7 | `perf(android): profile battery consumption during 12-hour background run`<br>`fix(overlay): handle android doze mode and low memory trim callbacks`<br>`chore(packaging): assemble signed release apk and verify zero .net` | `v1.0.0` |

---

## 4. Rincian Fase Kerja

### Fase 0 — Bootstrap Repo & Workspace
- [x] Inisialisasi git repository `VPetRustMobile`.
- [x] Tambahkan `.gitignore` untuk Rust & Android Gradle.
- [x] Tambahkan `.gitattributes` untuk normalisasi line-ending dan Linguist override.
- [x] Buat dokumen `IMPLEMENTATION.md`, `TODO.md`, dan `CHANGELOG.md`.
- [x] Siapkan script verifikasi `./scripts/check-no-dotnet.sh`.

### Fase 1 — Native NDK Bridge & JNI Bindings
- [ ] Setup `Cargo.toml` workspace dengan crate `vpet-android` (`crate-type = ["cdylib", "rlib"]`).
- [ ] Buat interface JNI murni via crate `jni`:
  - `nativeInit(assetPath: String, saveDir: String)`
  - `nativeTick(dtSeconds: Float): Boolean`
  - `nativeRenderBitmap(bitmap: Object)`
  - `nativeTouch(action: Int, x: Float, y: Float): Boolean`
  - `nativeGetPetState(): String`
- [ ] Konfigurasi kompilasi `cargo-ndk` untuk target `aarch64-linux-android`.

### Fase 2 — Android Foreground Service & Floating Overlay
- [ ] Setup Android project structure (Gradle, AndroidManifest.xml).
- [ ] Implementasikan `PetOverlayService`:
  - Notification channel & persistent notification "VPet is active".
  - Minta permission `SYSTEM_ALERT_WINDOW`.
  - Pasang view di `WindowManager` menggunakan `TYPE_APPLICATION_OVERLAY`.
- [ ] Tangani broadcast receiver `ACTION_SCREEN_OFF` dan `ACTION_SCREEN_ON` untuk stop/resume render loop.

### Fase 3 — Content Loader & Frame Rendering
- [ ] Salin/tautkan engine decoding `vpet-content` dan `vpet-render`.
- [ ] Ekstrak/baca aset `0000_core` di Android internal storage.
- [ ] Implementasikan zero-overhead blitting ke Android `Bitmap` via `AndroidBitmap_lockPixels`.
- [ ] Terapkan frame rate throttling (15 FPS idle, 30 FPS active).

### Fase 4 — Sentuhan Interaktif
- [ ] Tangani `MotionEvent` di custom view:
  - Drag threshold: jika digeser > 10dp, pindahkan posisi overlay window via `WindowManager.updateViewLayout`.
  - Tap: bedakan sentuhan kepala (`Touch_Head`) vs badan (`Touch_Body`).
- [ ] Terapkan animasi angkat (`Raised_Dynamic`) dan jatuh (`Raised_Static`).

### Fase 5 — Simulasi State & Balon Percakapan
- [ ] Tautkan `vpet-core` untuk kalkulasi status (lapar, haus, stamina, level, mood).
- [ ] Render balon percakapan (`BubbleStyle`) di atas kepala pet saat berinteraksi.
- [ ] Simpan state permainan secara atomik ke file `.mpak` di storage privat aplikasi.

### Fase 6 — AI Chat & Quick Action Wheel
- [ ] Hubungkan `AiWorker` Rust via `ureq` ke endpoint chat (`http://localhost:20128/v1` atau proxy internet).
- [ ] Tampilkan balon teks AI responsif saat chat selesai.
- [ ] Buat menu aksi melayang ringkas (Beri makan, Minum, Tidur, Ajak ngobrol).
- [ ] Buat Activity konfigurasi (ukuran pet, URL AI, reset posisi).

### Fase 7 — Release APK & Quality Hardening
- [ ] Uji soak test daya baterai (baterai tidak terkuras saat layar mati).
- [ ] Tangani `onTrimMemory` saat Android kehabisan RAM.
- [ ] Verifikasi `./scripts/check-no-dotnet.sh` pada shared library `.so`.
- [ ] Build biner final Release APK.
