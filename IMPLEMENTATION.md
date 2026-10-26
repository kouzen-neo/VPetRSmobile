# VPetRustMobile — Rencana Implementasi

**Status:** perancangan (`v0.0.1` bootstrap)  
**Target utama:** Android (API 26+ / Android 8.0 Oreo ke atas)  
**Tipe tampilan:** Floating Overlay Window (`SYSTEM_ALERT_WINDOW` / `TYPE_APPLICATION_OVERLAY`)  
**Prinsip:** 100% Rust pada core simulasi, content, renderer, dan networking AI. Zero .NET, CoreCLR, Mono, atau DLL. Android shell menggunakan Kotlin minimalis khusus integrasi OS.  
**Tanggal:** 2026-10-27

---

## 1. Ringkasan

VPetRustMobile adalah port/companion resmi VPet untuk Android berbasis arsitektur murni Rust yang diadopsi dari desktop VPetRS.

Pada desktop Linux, pet berjalan di atas Wayland layer-shell. Di Android, paradigma setara adalah **Floating Window Overlay** yang melayang di atas aplikasi lain menggunakan `WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY` yang didukung oleh `ForegroundService`.

### Karakteristik Utama

- **Floating Overlay:** Pet melayang bebas di layar smartphone, bisa digeser (drag), dielus, dan diberi makan saat pengguna membuka aplikasi lain (chat, browsing, video).
- **Core 100% Rust:** Logika stat (kenyang, haus, exp, leveling, mood), parser animasi LPS, rendering frame, serta koneksi AI chat ditangani seluruhnya oleh Rust via shared library native (`libvpet_mobile.so`).
- **Hemat Daya & Responsif:** Simulasi berjalan dengan adaptive tick rate; berhenti saat layar mati (`ACTION_SCREEN_OFF`) untuk mencegah pengurasan baterai.
- **Zero .NET:** Tidak ada runtime C#, Mono, atau Xamarin. Ukuran biner ramping, cold start instan.

---

## 2. Keputusan Arsitektur (ADR)

### ADR-001 — Rust Murni untuk Core, Zero .NET

**Keputusan:** seluruh simulasi game, decoding animasi, manajemen save, dan AI client ditulis di Rust. Kotlin hanya digunakan sebagai jembatan tipis ke API sistem Android (`WindowManager`, `Service`, `NotificationManager`).

**Konsekuensi:**
- Performa native tanpa overhead garbage collection pada loop simulasi.
- Biner shared library `.so` sangat kecil (~5-8 MB) dibanding runtime .NET MAUI / Avalonia (>80 MB).
- Tidak ada runtime C# atau library .NET di Android.

### ADR-002 — Floating Window Overlay via Foreground Service

**Keputusan:** pet ditampilkan menggunakan window overlay sistem (`TYPE_APPLICATION_OVERLAY`) di dalam `android.app.Service` foreground berstatus persisten.

**Alasan:**
- Pengguna mobile menginginkan pet menemani mereka saat chatting, browsing, atau bekerja, bukan terkurung dalam satu aplikasi fullscreen.
- Foreground service dengan persistent notification mencegah Android Low Memory Killer (OOM killer) mematikan proses saat RAM menipis.

**Konsekuensi:**
- Memerlukan izin runtime `android.permission.SYSTEM_ALERT_WINDOW` (pengguna mengaktifkan toggle *"Display over other apps"*).
- Memerlukan izin `android.permission.FOREGROUND_SERVICE` dan `android.permission.POST_NOTIFICATIONS`.

### ADR-003 — JNI Bridge Tipis (`vpet-android`)

**Keputusan:** komunikasi Kotlin ↔ Rust menggunakan binding JNI langsung lewat crate `jni-rs` tanpa framework abstraction tebal (seperti Flutter atau React Native).

**Alasan:** meminimalkan overhead latency pada input touch dan frame dispatch.

**Konsekuensi:**
- State game tetap hidup di memory heap native Rust; Kotlin hanya memegang pointer opaque (`jlong`).
- Operasi rendering menghasilkan pixel buffer (RGBA) yang di-copy langsung ke `android.graphics.Bitmap` via `AndroidBitmap_lockPixels` (NDK Bitmap API) atau software canvas.

### ADR-004 — Adaptive Tick & Battery Lifecycle

**Keputusan:** loop simulasi mendengarkan broadcast sistem `Intent.ACTION_SCREEN_OFF` dan `ACTION_SCREEN_ON`.
- Saat layar mati: simulasi masuk deep sleep, thread render berhenti, waktu offline diakumulasikan dan dihitung sekaligus saat layar menyala kembali.
- Saat idle: animasi berjalan di 10–15 FPS; naik ke 30 FPS hanya saat transisi sentuh atau drag.

**Alasan:** konsumsi baterai smartphone adalah metrik kritis. Aplikasi pet tidak boleh menghabiskan lebih dari 1–2% baterai per hari saat idle.

### ADR-005 — Penyimpanan Aset Lokal & Scoped Storage

**Keputusan:** aset inti animasi (`0000_core`) disimpan di internal app storage (`context.filesDir/content/0000_core`) atau diekstrak saat pertama kali dibuka dari asset archive terkompresi.

**Alasan:** mematuhi aturan Android Scoped Storage (API 29+) tanpa meminta izin akses broad external storage yang berbahaya.

### ADR-006 — AI Chat Asinkron di Rust Thread

**Keputusan:** request AI (OpenAI / Ollama / 9router) dijalankan di background thread native Rust via `ureq` + `rustls` dan ditransmisikan kembali via channel non-blocking.

**Alasan:** UI Android tidak pernah terblokir oleh latency jaringan, dan SSE chunk streaming bisa langsung diakumulasi menjadi teks balon bicara.

---

## 3. Goals dan Non-goals

### Goals (Fase 0 – 7)

- Pet melayang di atas semua aplikasi Android melalui `WindowManager`.
- Kontrol sentuh intuitif:
  - Tap kepala: animasi `Touch_Head` + dialogue bubble.
  - Tap badan: animasi `Touch_Body`.
  - Drag & drop: animasi `Raised_Dynamic` lalu jatuh ke posisi baru.
  - Double tap / long press: memunculkan quick action wheel (makan, minum, tidur, chat).
- Balon bicara anti-aliased untuk percakapan AI dan status pet.
- AI Chat terintegrasi via endpoint OpenAI-compatible (`http://localhost:20128/v1` atau remote).
- Sistem stat lengkap (hunger, thirst, health, feeling, exp, level, money) kompatibel dengan VPetRS.
- Save & load otomatis atomic MessagePack dengan backup di app storage.
- Click-through toggle (mode pasif tembus pandang jika mengganggu sentuhan aplikasi di bawahnya).
- Pengaturan via Android Activity: ubah ukuran skala pet, opacity, endpoint AI, dan reset posisi.
- 100% bebas dari .NET dan Mono.

### Non-goals

- Game mini 3D kompleks.
- Live Wallpaper (fokus saat ini adalah overlay interaktif; live wallpaper dapat menjadi fase lanjutan).
- Kompatibilitas plugin `.dll` .NET warisan Windows.
- Target iOS (iOS melarang overlay di atas aplikasi lain secara arsitektural).

---

## 4. Constraint dan Asumsi

- **Sistem Operasi:** Android 8.0 (API Level 26) hingga Android 15+.
- **Arsitektur CPU:** `aarch64-linux-android` (ARM 64-bit sebagai target utama). Fallback: `x86_64` untuk Android Studio emulator.
- **NDK:** Android NDK r26b+ (`cargo-ndk` terpasang).
- **Toolchain:** Rust 2024 edition, Gradle 8+, Java 17/21.
- **Memory Footprint:** RSS native < 40 MB RAM di perangkat mobile.

---

## 5. Arsitektur Sistem & Crate Graph

```text
                  ┌─────────────────────────────────────┐
                  │          Android App Layer          │
                  │              (Kotlin)               │
                  │  - PetOverlayService (Foreground)   │
                  │  - OverlayTouchView (Input/Gestures)│
                  │  - SettingsActivity / QuickMenu     │
                  └──────────────────┬──────────────────┘
                                     │ JNI Calls
                                     ▼
                  ┌─────────────────────────────────────┐
                  │            vpet-android             │
                  │          (cdylib JNI crate)         │
                  │  - JNI exports & type converters    │
                  │  - Native lifecycle bridge          │
                  │  - Bitmap lockPixels blitter        │
                  └──────────────────┬──────────────────┘
                                     │
           ┌─────────────────────────┼─────────────────────────┐
           │                         │                         │
    ┌──────▼──────┐           ┌──────▼──────┐           ┌──────▼──────┐
    │ vpet-render │           │vpet-content │           │  vpet-app   │
    │  tiny-skia  │           │ LPS parser  │           │ AI worker   │
    │ cosmic-text │           │ animation DB│           │ HTTP client │
    └──────┬──────┘           └──────┬──────┘           └──────┬──────┘
           │                         │                         │
           └─────────────────────────┼─────────────────────────┘
                                     │
                              ┌──────▼──────┐
                              │  vpet-core  │
                              │ simulasi    │
                              │ stat, save  │
                              └─────────────┘
```

---

## 6. Desain Floating Window & Input

### 6.1 Layout Overlay WindowManager

Window pet dibuat dengan parameter:
```kotlin
val params = WindowManager.LayoutParams(
    spriteWidthPx,
    spriteHeightPx,
    WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
    WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
    WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
    PixelFormat.TRANSLUCENT
)
```

### 6.2 Siklus Touch Dispatch

1. **Sentuhan Pertama (`ACTION_DOWN`):**
   - Cek posisi sentuh terhadap koordinat sprite pet.
   - Jika menyentuh area transparan (alpha < 10): lewatkan event ke view belakangnya jika click-through aktif.
2. **Geser (`ACTION_MOVE`):**
   - Jika jarak pergeseran > ambang drag (8dp): ubah state animasi pet ke `Raised_Dynamic` dan update koordinat window `params.x`, `params.y` via `windowManager.updateViewLayout`.
3. **Lepas (`ACTION_UP`):**
   - Jika durasi sentuh singkat tanpa drag: evaluasi hit region (kepala vs badan) dan trigger aksi sentuh.
   - Jika selesai di-drag: putar animasi jatuh `Raised_Static` dan simpan koordinat posisi baru.

---

## 7. Quality Gate

Sebelum rilis atau merge:
```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo ndk -t arm64-v8a build --release
./scripts/check-no-dotnet.sh
```
