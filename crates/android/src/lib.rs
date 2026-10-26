pub mod ai;
pub mod anim;
pub mod persistence;

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ai::{AiConfig, AiWorker};
use anim::{ActionState, AdvanceViewport, HitZone, InteractionController, classify_hit_zone};
use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{JNI_FALSE, JNI_TRUE, jboolean, jfloat, jint, jlong, jstring},
};
use log::{error, info};
use persistence::SaveStore;
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};
use vpet_content::{anim::AnimationIndex, anim::PetConfig, food::FoodItem, scan::ContentDatabase};
use vpet_core::{ModeType, World, item::FoodEffect};
use vpet_render::{
    cache::FrameCache, loader::AsyncClipLoader, text::BubbleRenderer, text::BubbleStyle,
};

#[cfg(target_os = "android")]
#[repr(C)]
#[derive(Debug, Default)]
pub struct AndroidBitmapInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: i32,
    pub flags: u32,
}

#[cfg(target_os = "android")]
unsafe extern "C" {
    fn AndroidBitmap_getInfo(
        env: *mut jni::sys::JNIEnv,
        jbitmap: jni::sys::jobject,
        info: *mut AndroidBitmapInfo,
    ) -> i32;

    fn AndroidBitmap_lockPixels(
        env: *mut jni::sys::JNIEnv,
        jbitmap: jni::sys::jobject,
        addrPtr: *mut *mut std::ffi::c_void,
    ) -> i32;

    fn AndroidBitmap_unlockPixels(env: *mut jni::sys::JNIEnv, jbitmap: jni::sys::jobject) -> i32;
}

pub struct MobileEngine {
    pub world: World,
    pub content: ContentDatabase,
    pub pet_cfg: PetConfig,
    pub anim_index: AnimationIndex,
    pub anim: InteractionController,
    pub cache: FrameCache,
    pub loader: AsyncClipLoader,
    pub bubble_renderer: Option<BubbleRenderer>,
    pub bubble_style: BubbleStyle,
    pub save_store: SaveStore,
    pub ai_worker: AiWorker,
    pub target_px: u32,
    pub bubble_pad_px: u32,
    pub pos: (i32, i32),
    pub last_clip_key: String,
    pub last_frame_idx: usize,
    pub last_bubble: Option<String>,
    pub cached_sprite: Option<Arc<Pixmap>>,
    pub cached_bubble: Option<Pixmap>,
    pub composited: Option<Pixmap>,
    pub last_autosave: Instant,
}

impl MobileEngine {
    pub fn new(
        content_root: &Path,
        save_dir: &Path,
        target_px: u32,
        ai_base: &str,
        ai_model: &str,
    ) -> Result<Self, String> {
        let mut content = ContentDatabase::load_root(content_root);
        content.localize("en");

        let pet_cfg = content
            .pets
            .get("vup")
            .cloned()
            .or_else(|| content.pets.values().next().cloned())
            .unwrap_or_default();

        let anim_index = content
            .animations
            .get(&pet_cfg.id)
            .cloned()
            .unwrap_or_default();

        let save_store = SaveStore::new(save_dir);
        let mut world = save_store.load_or_recover("default").unwrap_or_else(|| {
            info!("initializing fresh default world for mobile");
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0x5650_4554, |d| d.as_nanos() as u64);
            World::new("default", "Loris", seed)
        });

        let mut anim = InteractionController::new(target_px, false);
        anim.ensure_clip(&anim_index, &mut world);

        let cache = FrameCache::new(64 * 1024 * 1024);
        let loader = AsyncClipLoader::new();

        let ai_cfg = AiConfig {
            enabled: true,
            api_base: if ai_base.is_empty() {
                "http://10.0.2.2:20128/v1".to_owned()
            } else {
                ai_base.to_owned()
            },
            api_key: String::new(),
            model: if ai_model.is_empty() {
                "9routerall".to_owned()
            } else {
                ai_model.to_owned()
            },
            system_prompt: "You are Loris, a lively and loving desktop pet. Keep your replies concise (1-2 short sentences)."
                .to_owned(),
            timeout_secs: 25,
        };
        let ai_worker = AiWorker::spawn(ai_cfg);

        let mut bubble_renderer = BubbleRenderer::new();
        let theme_fonts_dir = content_root.join("theme/fonts");
        if let Ok(entries) = std::fs::read_dir(&theme_fonts_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension()
                    .is_some_and(|ext| ext == "ttf" || ext == "otf")
                {
                    info!("loading bundled theme font: {}", p.display());
                    bubble_renderer.load_font_file(&p);
                }
            }
        }
        for sys_font in [
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/DroidSans.ttf",
            "/system/fonts/NotoSansCJK-Regular.ttc",
        ] {
            let p = Path::new(sys_font);
            if p.exists() {
                bubble_renderer.load_font_file(p);
            }
        }

        let bubble_style = BubbleStyle {
            bg_rgba: [255, 255, 255, 245],
            border_rgba: [3, 169, 244, 255],
            text_rgba: [15, 23, 42, 255],
            font_px: (target_px as f32 * 0.065).clamp(14.0, 22.0),
            padding_px: 10,
            corner_radius: 10.0,
        };

        let bubble_pad_px = target_px / 3;

        Ok(Self {
            world,
            content,
            pet_cfg,
            anim_index,
            anim,
            cache,
            loader,
            bubble_renderer: Some(bubble_renderer),
            bubble_style,
            save_store,
            ai_worker,
            target_px,
            bubble_pad_px,
            pos: (0, 0),
            last_clip_key: String::new(),
            last_frame_idx: usize::MAX,
            last_bubble: None,
            cached_sprite: None,
            cached_bubble: None,
            composited: None,
            last_autosave: Instant::now(),
        })
    }

    pub fn tick(&mut self, dt_seconds: f32) -> bool {
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

        self.world.tick(f64::from(dt_seconds), now_unix);

        let dt_ms = (dt_seconds * 1000.0).round() as u32;
        let durs: Option<Vec<u32>> = self
            .anim
            .active_clip()
            .and_then(|active| self.cache.get(&active.cache_key))
            .map(|clip| clip.frames.iter().map(|f| f.duration_ms).collect());

        let vp = AdvanceViewport {
            bounds: (1080, 2400),
            sprite_logical: (self.target_px, self.target_px),
            output_name: "screen",
        };

        self.anim.advance(
            dt_ms.max(16),
            durs.as_deref(),
            &mut self.pos,
            vp,
            &self.anim_index,
            &mut self.world,
        );

        if let Some(res) = self.ai_worker.poll_reply() {
            match res.reply {
                Ok(reply) => {
                    self.anim.show_bubble(reply, 8_000);
                    self.world.pet.add_likability(0.8);
                    self.world.pet.add_feeling(2.5);
                    self.world.pet.add_exp(2.0);
                    self.anim.request_state(
                        ActionState::Saying,
                        false,
                        &self.anim_index,
                        &mut self.world,
                    );
                }
                Err(err) => {
                    self.anim.show_bubble(format!("AI: {err}"), 5_000);
                }
            }
        }

        if self.last_autosave.elapsed() >= Duration::from_secs(60) {
            let _ = self.save_store.save_world(&self.world);
            self.last_autosave = Instant::now();
        }

        self.refresh_composited()
    }

    fn refresh_composited(&mut self) -> bool {
        self.loader.poll_into(&mut self.cache);

        let mut changed = false;

        if let Some(active) = self.anim.active_clip() {
            let key = active.cache_key.clone();
            let frame_idx = active.frame_index;
            let source = active.source.clone();

            if !self.cache.contains(&key) {
                self.loader.request(key.clone(), source, self.target_px);
            }

            if let Some(clip) = self.cache.get(&key)
                && !clip.frames.is_empty()
            {
                let safe_idx = frame_idx % clip.frames.len();
                if self.last_clip_key != key || self.last_frame_idx != safe_idx {
                    self.cached_sprite = Some(clip.frames[safe_idx].pixmap.clone());
                    self.last_clip_key = key;
                    self.last_frame_idx = safe_idx;
                    changed = true;
                }
            }
        }

        let cur_bubble = self.anim.bubble_text().map(str::to_owned);
        if cur_bubble != self.last_bubble {
            self.cached_bubble = cur_bubble.as_deref().and_then(|txt| {
                self.bubble_renderer
                    .as_mut()
                    .and_then(|br| br.render_bubble(txt, self.target_px, &self.bubble_style))
            });
            self.last_bubble = cur_bubble;
            changed = true;
        }

        if changed || self.composited.is_none() {
            let total_w = self.target_px;
            let total_h = self.target_px + self.bubble_pad_px;
            let mut canvas = Pixmap::new(total_w, total_h)
                .unwrap_or_else(|| Pixmap::new(250, 330).expect("canvas alloc"));
            canvas.fill(Color::TRANSPARENT);

            if let Some(bubble) = &self.cached_bubble {
                let bx = ((total_w as f32 - bubble.width() as f32) / 2.0).max(0.0);
                let by = (self.bubble_pad_px as f32 - bubble.height() as f32).max(0.0);
                canvas.draw_pixmap(
                    bx as i32,
                    by as i32,
                    bubble.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }

            if let Some(sprite) = &self.cached_sprite {
                let sx = ((total_w as f32 - sprite.width() as f32) / 2.0).max(0.0);
                let sy = self.bubble_pad_px as f32;
                canvas.draw_pixmap(
                    sx as i32,
                    sy as i32,
                    sprite.as_ref().as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }

            self.composited = Some(canvas);
            true
        } else {
            false
        }
    }

    pub fn handle_touch(&mut self, event: &str, rel_x: f32, rel_y: f32) -> String {
        let pet_y = rel_y - self.bubble_pad_px as f32;
        if pet_y < 0.0 {
            return "bubble_tap".to_owned();
        }

        let zone = classify_hit_zone(
            &self.pet_cfg,
            (self.target_px, self.target_px),
            f64::from(rel_x),
            f64::from(pet_y),
        );

        match event {
            "tap" => {
                self.anim.on_click_zone(
                    zone,
                    &self.content.texts,
                    &self.anim_index,
                    &mut self.world,
                );
                "clicked".to_owned()
            }
            "tap_head" => {
                self.anim.on_click_zone(
                    HitZone::Head,
                    &self.content.texts,
                    &self.anim_index,
                    &mut self.world,
                );
                "touched_head".to_owned()
            }
            "tap_body" => {
                self.anim.on_click_zone(
                    HitZone::Body,
                    &self.content.texts,
                    &self.anim_index,
                    &mut self.world,
                );
                "touched_body".to_owned()
            }
            "drag" => {
                self.anim.on_drag_start(&self.anim_index, &mut self.world);
                "dragging".to_owned()
            }
            "drop" => {
                self.anim
                    .on_drag_end("screen", self.pos, &self.anim_index, &mut self.world);
                "dropped".to_owned()
            }
            _ => "unknown".to_owned(),
        }
    }

    pub fn feed(&mut self, food_name: &str) -> String {
        let item = self.content.foods.get(food_name);
        let eff = if let Some(it) = item {
            food_item_to_effect(it)
        } else {
            default_snack_effect()
        };

        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

        let label = eff.name.clone();
        match self.world.feed(&eff, now_unix) {
            Ok(_) => {
                self.anim.show_bubble(format!("Yum! ({label})"), 3_000);
                format!("fed {label}")
            }
            Err(e) => {
                self.anim.show_bubble(format!("Cannot eat: {e}"), 3_000);
                format!("feed_error: {e}")
            }
        }
    }

    pub fn toggle_sleep(&mut self) -> bool {
        self.anim.toggle_sleep(&self.anim_index, &mut self.world)
    }

    pub fn start_work(&mut self, work_name: &str) -> String {
        let work = self.pet_cfg.works.iter().find(|w| {
            w.name.eq_ignore_ascii_case(work_name) || w.graph.eq_ignore_ascii_case(work_name)
        });
        if let Some(w) = work {
            let name = w.name.clone();
            self.anim.request_state(
                ActionState::Working {
                    graph: w.graph.clone(),
                },
                true,
                &self.anim_index,
                &mut self.world,
            );
            self.anim.show_bubble(format!("Working: {name}!"), 4_000);
            format!("started {name}")
        } else {
            self.anim.request_state(
                ActionState::Working {
                    graph: "work".to_owned(),
                },
                true,
                &self.anim_index,
                &mut self.world,
            );
            self.anim.show_bubble("Working hard!", 4_000);
            "started work".to_owned()
        }
    }

    pub fn cancel_work(&mut self) -> bool {
        self.anim
            .request_state(ActionState::Idle, true, &self.anim_index, &mut self.world);
        self.anim.show_bubble("Break time!", 3_000);
        true
    }

    pub fn get_status_json(&self) -> String {
        let p = &self.world.pet;
        let mode_str = match p.mode {
            ModeType::Happy => "Happy",
            ModeType::Nomal => "Normal",
            ModeType::PoorCondition => "PoorCondition",
            ModeType::Ill => "Ill",
        };
        serde_json::json!({
            "name": p.name,
            "level": p.level,
            "exp": p.exp,
            "money": p.money,
            "food": p.strength_food,
            "drink": p.strength_drink,
            "health": p.health,
            "feeling": p.feeling,
            "likability": p.likability,
            "strength": p.strength,
            "mode": mode_str,
            "sleeping": p.sleeping,
            "bubble": self.anim.bubble_text().unwrap_or(""),
        })
        .to_string()
    }
}

fn food_item_to_effect(item: &FoodItem) -> FoodEffect {
    FoodEffect {
        name: item.name.clone(),
        price: item.price,
        exp: item.exp,
        strength: item.strength,
        strength_food: item.strength_food,
        strength_drink: item.strength_drink,
        feeling: item.feeling,
        health: item.health,
        likability: item.likability,
        buff_id: item.buff.clone(),
        buff_time_mins: item.buff_time_mins,
    }
}

fn default_snack_effect() -> FoodEffect {
    FoodEffect {
        name: "Snack".to_owned(),
        price: 0.0,
        exp: 8.0,
        strength: 15.0,
        strength_food: 25.0,
        strength_drink: 15.0,
        feeling: 10.0,
        health: 1.0,
        likability: 0.5,
        buff_id: None,
        buff_time_mins: 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JNI Exported Functions
// ─────────────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    content_root: JString,
    save_dir: JString,
    target_px: jint,
    ai_base: JString,
    ai_model: JString,
) -> jlong {
    #[cfg(target_os = "android")]
    {
        let _ = android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("VPetMobile"),
        );
    }

    let root_str: String = env
        .get_string(&content_root)
        .map(|s| s.into())
        .unwrap_or_default();
    let save_str: String = env
        .get_string(&save_dir)
        .map(|s| s.into())
        .unwrap_or_default();
    let base_str: String = env
        .get_string(&ai_base)
        .map(|s| s.into())
        .unwrap_or_default();
    let model_str: String = env
        .get_string(&ai_model)
        .map(|s| s.into())
        .unwrap_or_default();

    match MobileEngine::new(
        Path::new(&root_str),
        Path::new(&save_str),
        (target_px as u32).max(100),
        &base_str,
        &model_str,
    ) {
        Ok(engine) => Box::into_raw(Box::new(engine)) as jlong,
        Err(err) => {
            error!("nativeInit failed: {err}");
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
) {
    if engine_ptr != 0 {
        let engine = unsafe { Box::from_raw(engine_ptr as *mut MobileEngine) };
        let _ = engine.save_store.save_world(&engine.world);
        info!("engine destroyed cleanly and state saved");
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeTick(
    _env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    dt_seconds: jfloat,
) -> jboolean {
    if engine_ptr == 0 {
        return JNI_FALSE;
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    if engine.tick(dt_seconds) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeRender(
    env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    bitmap: JObject,
) -> jboolean {
    if engine_ptr == 0 {
        return JNI_FALSE;
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    let Some(composited) = &engine.composited else {
        return JNI_FALSE;
    };

    #[cfg(target_os = "android")]
    {
        let env_raw = env.get_raw();
        let jbitmap = bitmap.as_raw();
        let mut info = AndroidBitmapInfo::default();
        if unsafe { AndroidBitmap_getInfo(env_raw, jbitmap, &mut info) } != 0 {
            return JNI_FALSE;
        }

        let mut pixels_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        if unsafe { AndroidBitmap_lockPixels(env_raw, jbitmap, &mut pixels_ptr) } != 0 {
            return JNI_FALSE;
        }

        let stride = info.stride as usize;
        let comp_w = composited.width() as usize;
        let comp_h = composited.height() as usize;
        let min_w = comp_w.min(info.width as usize);
        let min_h = comp_h.min(info.height as usize);
        let src_bytes = composited.data();

        let dst = pixels_ptr as *mut u8;
        for row in 0..min_h {
            let src_row = &src_bytes[row * comp_w * 4..(row * comp_w + min_w) * 4];
            unsafe {
                std::ptr::copy_nonoverlapping(src_row.as_ptr(), dst.add(row * stride), min_w * 4);
            }
        }

        unsafe { AndroidBitmap_unlockPixels(env_raw, jbitmap) };
        JNI_TRUE
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (env, bitmap, composited);
        JNI_TRUE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeTouch(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    event_type: JString,
    rel_x: jfloat,
    rel_y: jfloat,
) -> jstring {
    if engine_ptr == 0 {
        return env.new_string("").unwrap().into_raw();
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    let event: String = env
        .get_string(&event_type)
        .map(|s| s.into())
        .unwrap_or_default();
    let res = engine.handle_touch(&event, rel_x, rel_y);
    env.new_string(res).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeGetStateJson(
    env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
) -> jstring {
    if engine_ptr == 0 {
        return env.new_string("{}").unwrap().into_raw();
    }
    let engine = unsafe { &*(engine_ptr as *const MobileEngine) };
    let json = engine.get_status_json();
    env.new_string(json).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeFeed(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    food_name: JString,
) -> jstring {
    if engine_ptr == 0 {
        return env.new_string("").unwrap().into_raw();
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    let food: String = env
        .get_string(&food_name)
        .map(|s| s.into())
        .unwrap_or_default();
    let res = engine.feed(&food);
    env.new_string(res).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeStartWork(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    work_name: JString,
) -> jstring {
    if engine_ptr == 0 {
        return env.new_string("").unwrap().into_raw();
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    let work: String = env
        .get_string(&work_name)
        .map(|s| s.into())
        .unwrap_or_default();
    let res = engine.start_work(&work);
    env.new_string(res).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeCancelWork(
    _env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
) -> jboolean {
    if engine_ptr == 0 {
        return JNI_FALSE;
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    if engine.cancel_work() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeToggleSleep(
    _env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
) -> jboolean {
    if engine_ptr == 0 {
        return JNI_FALSE;
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    if engine.toggle_sleep() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeAskAi(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    prompt: JString,
) -> jboolean {
    if engine_ptr == 0 {
        return JNI_FALSE;
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    let p: String = env
        .get_string(&prompt)
        .map(|s| s.into())
        .unwrap_or_default();
    engine.anim.show_bubble("...", 8_000);
    engine.anim.request_state(
        ActionState::Saying,
        false,
        &engine.anim_index,
        &mut engine.world,
    );
    if engine.ai_worker.ask(p) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeSay(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    text: JString,
) {
    if engine_ptr == 0 {
        return;
    }
    let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
    let t: String = env.get_string(&text).map(|s| s.into()).unwrap_or_default();
    engine.anim.show_bubble(t, 5_000);
    engine.anim.request_state(
        ActionState::Saying,
        false,
        &engine.anim_index,
        &mut engine.world,
    );
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeSave(
    _env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
) {
    if engine_ptr != 0 {
        let engine = unsafe { &mut *(engine_ptr as *mut MobileEngine) };
        let _ = engine.save_store.save_world(&engine.world);
    }
}
