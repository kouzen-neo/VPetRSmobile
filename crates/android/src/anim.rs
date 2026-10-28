#![allow(dead_code)]
//! Priority animation state machine, `A_Start -> B_Loop -> C_End` segment player,
//! `500x500` logical hit-region classifier, auto-walk, and headless interaction controller.

use std::path::PathBuf;

use vpet_content::{
    AnimatType, AnimationEntry, AnimationIndex, AnimationSource, GraphType, PetConfig,
};
use vpet_core::{ModeType, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    Head,
    Body,
    Pinch,
}

/// Map surface-relative sprite click `(rel_x, rel_y)` into VPet `500x500` logical space
/// and classify against `vup.lps` hit rectangles.
#[must_use]
pub fn classify_hit_zone(
    cfg: &PetConfig,
    sprite_logical: (u32, u32),
    rel_x: f64,
    rel_y: f64,
) -> HitZone {
    let sw = f64::from(sprite_logical.0.max(1));
    let sh = f64::from(sprite_logical.1.max(1));
    let vx = ((rel_x / sw) * 500.0).round() as i32;
    let vy = ((rel_y / sh) * 500.0).round() as i32;

    if cfg.pinch.contains(vx, vy) {
        HitZone::Pinch
    } else if cfg.touch_head.contains(vx, vy) {
        HitZone::Head
    } else {
        // Both explicit `touchbody` and remaining visible silhouette pixels count as body touch.
        let _ = cfg.touch_body.contains(vx, vy);
        HitZone::Body
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchKind {
    Head,
    Body,
}

impl TouchKind {
    #[must_use]
    pub const fn graph_type(self) -> GraphType {
        match self {
            Self::Head => GraphType::TouchHead,
            Self::Body => GraphType::TouchBody,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClimbKind {
    LeftWall,
    RightWall,
    TopCeilingLeft,
    TopCeilingRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchKind {
    Up,
    Down,
    Hunger,
    Thirsty,
}

/// High-level behavioral state ordered by priority:
/// `Raised (100) > Shutdown (95) > StartUp (85) > LevelUp (80) > Pinch (75) > Touch (70) > Eating (65) > Switch (60) > Working (50) > Thinking (45) > Saying (40) > SideHide (35) > Climbing (35) > Falling (35) > Moving (30) > StateOne (25) > IdleFidget (22) > Idle (20) > Sleeping (10)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionState {
    StartUp,
    Shutdown {
        exiting: bool,
    },
    Idle,
    IdleFidget {
        name: Option<String>,
    },
    StateOne,
    Sleeping {
        waking: bool,
    },
    Moving {
        dir_x: i32,
        speed_px_per_sec: i32,
        loops_remaining: u32,
    },
    Climbing {
        kind: ClimbKind,
        loops_remaining: u32,
    },
    Falling {
        left: bool,
    },
    SideHide {
        left: bool,
        emerging: bool,
    },
    Saying,
    Thinking,
    Working {
        graph: String,
    },
    Touch {
        kind: TouchKind,
    },
    Pinch,
    Eating {
        graph: String,
        food_name: Option<String>,
        icon_path: Option<PathBuf>,
    },
    LevelUp,
    Switch {
        kind: SwitchKind,
    },
    Raised {
        dropping: bool,
        is_static: bool,
    },
}

impl ActionState {
    #[must_use]
    pub const fn priority(&self) -> u8 {
        match self {
            Self::Sleeping { .. } => 10,
            Self::Idle => 20,
            Self::IdleFidget { .. } => 22,
            Self::StateOne => 25,
            Self::Moving { .. } => 30,
            Self::SideHide { .. } => 35,
            Self::Climbing { .. } => 35,
            Self::Falling { .. } => 35,
            Self::Saying => 40,
            Self::Thinking => 45,
            Self::Working { .. } => 50,
            Self::Switch { .. } => 60,
            Self::Eating { .. } => 65,
            Self::Touch { .. } => 70,
            Self::Pinch => 75,
            Self::LevelUp => 80,
            Self::StartUp => 85,
            Self::Shutdown { .. } => 95,
            Self::Raised { .. } => 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentPhase {
    AStart,
    BLoop { remaining_loops: Option<u32> },
    CEnd,
    Single { looped: bool },
}

#[derive(Debug, Clone)]
pub struct ActiveClip {
    pub cache_key: String,
    pub graph_type: GraphType,
    pub name: Option<String>,
    pub mode: ModeType,
    pub phase: SegmentPhase,
    pub source: AnimationSource,
    pub frame_index: usize,
    pub frame_elapsed_ms: u32,
    pub seed: u64,
}

#[must_use]
pub fn clip_cache_key(entry: &AnimationEntry, target_px: u32) -> String {
    format!(
        "{}:{}:{:?}:{:?}:{}:{}",
        entry.pet_id, entry.source_folder, entry.graph_type, entry.animat, entry.name, target_px
    )
}

#[derive(Debug, Clone, Copy)]
pub struct AdvanceViewport<'a> {
    pub bounds: (u32, u32),
    pub sprite_logical: (u32, u32),
    pub output_name: &'a str,
}

#[derive(Debug)]
pub struct InteractionController {
    state: ActionState,
    active_clip: Option<ActiveClip>,
    target_px: u32,
    auto_walk_enabled: bool,
    idle_elapsed_ms: u32,
    bubble_text: Option<String>,
    bubble_remaining_ms: u32,
    walk_subpx: f64,
    shutdown_ready: bool,
    prev_mode: ModeType,
    prev_food: f64,
    prev_drink: f64,
    drag_still_elapsed_ms: u32,
}

impl InteractionController {
    #[must_use]
    pub fn new(target_px: u32, auto_walk_enabled: bool) -> Self {
        Self {
            state: ActionState::Idle,
            active_clip: None,
            target_px: target_px.max(32),
            auto_walk_enabled,
            idle_elapsed_ms: 0,
            bubble_text: None,
            bubble_remaining_ms: 0,
            walk_subpx: 0.0,
            shutdown_ready: false,
            prev_mode: ModeType::Nomal,
            prev_food: 100.0,
            prev_drink: 100.0,
            drag_still_elapsed_ms: 0,
        }
    }

    #[must_use]
    pub const fn is_shutdown_ready(&self) -> bool {
        self.shutdown_ready
    }

    pub fn trigger_shutdown(&mut self, index: &AnimationIndex, world: &mut World) {
        self.request_state(ActionState::Shutdown { exiting: true }, true, index, world);
    }

    pub fn on_feed(
        &mut self,
        graph: impl Into<String>,
        food_name: Option<String>,
        icon_path: Option<PathBuf>,
        index: &AnimationIndex,
        world: &mut World,
    ) {
        self.request_state(
            ActionState::Eating {
                graph: graph.into(),
                food_name,
                icon_path,
            },
            true,
            index,
            world,
        );
    }

    pub fn on_level_up(&mut self, index: &AnimationIndex, world: &mut World) {
        self.show_bubble("Level UP! ~", 4_000);
        self.request_state(ActionState::LevelUp, true, index, world);
    }

    pub fn on_think(&mut self, index: &AnimationIndex, world: &mut World) {
        self.request_state(ActionState::Thinking, false, index, world);
    }

    pub fn on_switch(&mut self, kind: SwitchKind, index: &AnimationIndex, world: &mut World) {
        self.request_state(ActionState::Switch { kind }, false, index, world);
    }

    #[must_use]
    pub const fn state(&self) -> &ActionState {
        &self.state
    }

    #[must_use]
    pub const fn active_clip(&self) -> Option<&ActiveClip> {
        self.active_clip.as_ref()
    }

    #[must_use]
    pub fn bubble_text(&self) -> Option<&str> {
        self.bubble_text.as_deref()
    }

    pub fn set_auto_walk(&mut self, enabled: bool) {
        self.auto_walk_enabled = enabled;
    }

    pub fn set_target_px(&mut self, target_px: u32) {
        let clamped = target_px.max(32);
        if self.target_px != clamped {
            self.target_px = clamped;
            self.active_clip = None;
        }
    }

    pub fn show_bubble(&mut self, text: impl Into<String>, duration_ms: u32) {
        let t = text.into();
        if !t.trim().is_empty() {
            self.bubble_text = Some(t);
            self.bubble_remaining_ms = duration_ms.max(500);
        }
    }

    /// Ensure an initial clip is selected for the current state.
    pub fn ensure_clip(&mut self, index: &AnimationIndex, world: &mut World) {
        if self.active_clip.is_none() {
            self.enter_state_clip(index, world);
        }
    }

    /// Attempt to transition into `next`, respecting priority unless `force` is set.
    pub fn request_state(
        &mut self,
        next: ActionState,
        force: bool,
        index: &AnimationIndex,
        world: &mut World,
    ) -> bool {
        if !force && next.priority() < self.state.priority() {
            return false;
        }
        self.state = next;
        self.idle_elapsed_ms = 0;
        self.walk_subpx = 0.0;
        self.enter_state_clip(index, world);
        true
    }

    pub fn on_drag_start(&mut self, index: &AnimationIndex, world: &mut World) {
        self.drag_still_elapsed_ms = 0;
        self.request_state(
            ActionState::Raised {
                dropping: false,
                is_static: false,
            },
            true,
            index,
            world,
        );
    }

    /// Keep the held animation alive while the finger moves and cancel the relaxed pose.
    pub fn on_drag_motion(&mut self, index: &AnimationIndex, world: &mut World) {
        self.drag_still_elapsed_ms = 0;
        if let ActionState::Raised {
            dropping: false,
            is_static,
        } = &mut self.state
            && *is_static
        {
            *is_static = false;
            self.enter_state_clip(index, world);
        }
    }

    pub fn on_drag_end(
        &mut self,
        output_name: &str,
        mut pos: (i32, i32),
        bounds: (u32, u32),
        sprite_logical: (u32, u32),
        index: &AnimationIndex,
        world: &mut World,
    ) {
        let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
        let offset_left = ((219_f32 * sprite_logical.0 as f32) / 500.0).round() as i32;
        let offset_right = (((500.0 - 281_f32) * sprite_logical.0 as f32) / 500.0).round() as i32;
        let offset_top = ((150_f32 * sprite_logical.1 as f32) / 500.0).round() as i32;
        // The overlay draws a bubble strip above the sprite, so "near the top" is measured
        // in window space: sprite y plus that strip must still fall inside the top band.
        let bubble_pad = sprite_logical.1 as i32 / 3;
        if pos.1 <= 60 + bubble_pad {
            pos.1 = -offset_top;
            world.output_positions.insert(output_name.to_owned(), pos);
            let kind = if pos.0 < max_x / 2 {
                ClimbKind::TopCeilingRight
            } else {
                ClimbKind::TopCeilingLeft
            };
            self.request_state(
                ActionState::Climbing {
                    kind,
                    loops_remaining: 6,
                },
                true,
                index,
                world,
            );
        } else if pos.0 <= 60 {
            pos.0 = -offset_left;
            world.output_positions.insert(output_name.to_owned(), pos);
            self.request_state(
                ActionState::SideHide {
                    left: true,
                    emerging: false,
                },
                true,
                index,
                world,
            );
        } else if pos.0 >= max_x.saturating_sub(60) {
            pos.0 = max_x + offset_right;
            world.output_positions.insert(output_name.to_owned(), pos);
            self.request_state(
                ActionState::SideHide {
                    left: false,
                    emerging: false,
                },
                true,
                index,
                world,
            );
        } else {
            world.output_positions.insert(output_name.to_owned(), pos);
            self.request_state(
                ActionState::Raised {
                    dropping: true,
                    is_static: false,
                },
                true,
                index,
                world,
            );
        }
    }

    pub fn on_click_zone(
        &mut self,
        zone: HitZone,
        texts: &vpet_content::TextDatabase,
        index: &AnimationIndex,
        world: &mut World,
    ) {
        if zone == HitZone::Pinch {
            let _ = world.pet_head();
            self.show_bubble("Don't pinch my cheeks!", 3_000);
            self.request_state(ActionState::Pinch, false, index, world);
            return;
        }

        let kind = match zone {
            HitZone::Head => {
                let _ = world.pet_head();
                TouchKind::Head
            }
            HitZone::Body => {
                let _ = world.pet_body();
                TouchKind::Body
            }
            HitZone::Pinch => unreachable!(),
        };

        let seed = world.rng.next_u64();
        if let Some(line) =
            texts.pick_click_text(world.pet.mode, world.pet.likability, "Nomal", seed)
        {
            world.pet.add_feeling(line.feeling_delta);
            world.pet.add_likability(line.likability_delta);
            if line.exp_delta != 0.0 {
                let _ = world.pet.add_exp(line.exp_delta);
            }
            world.pet.money = (world.pet.money + line.money_delta).max(0.0);
            self.show_bubble(line.text.clone(), 4_000);
        }

        self.request_state(ActionState::Touch { kind }, false, index, world);
    }

    pub fn toggle_sleep(&mut self, index: &AnimationIndex, world: &mut World) -> bool {
        if world.pet.sleeping {
            world.set_sleeping(false);
            self.request_state(ActionState::Sleeping { waking: true }, true, index, world);
            false
        } else {
            world.set_sleeping(true);
            self.request_state(ActionState::Sleeping { waking: false }, true, index, world);
            true
        }
    }

    /// Advance animation frames, segment transitions (`A -> B -> C`), speech bubble timers,
    /// and auto-walk movement by `dt_ms` milliseconds.
    ///
    /// `frame_durations_ms` provides the decoded frame durations of the currently active clip
    /// (`None` if still decoding on the background thread, in which case a `125 ms` placeholder
    /// tick is used so state transitions never stall headlessly).
    pub fn advance(
        &mut self,
        dt_ms: u32,
        frame_durations_ms: Option<&[u32]>,
        pos: &mut (i32, i32),
        vp: AdvanceViewport<'_>,
        index: &AnimationIndex,
        world: &mut World,
    ) {
        let bounds = vp.bounds;
        let sprite_logical = vp.sprite_logical;
        let output_name = vp.output_name;
        if self.bubble_remaining_ms > 0 {
            self.bubble_remaining_ms = self.bubble_remaining_ms.saturating_sub(dt_ms);
            if self.bubble_remaining_ms == 0 {
                self.bubble_text = None;
            }
        }

        // Sync external sleep state if set via World/IPC.
        if world.pet.sleeping && matches!(self.state, ActionState::Idle) {
            self.request_state(ActionState::Sleeping { waking: false }, true, index, world);
        }

        // Check for stat shifts / mode switch while idle
        if matches!(self.state, ActionState::Idle) && !world.pet.sleeping {
            if world.pet.mode != self.prev_mode {
                let improved = matches!(world.pet.mode, ModeType::Happy)
                    || matches!(self.prev_mode, ModeType::Ill);
                let kind = if improved {
                    SwitchKind::Up
                } else {
                    SwitchKind::Down
                };
                self.prev_mode = world.pet.mode;
                self.on_switch(kind, index, world);
            } else if world.pet.strength_food <= 20.0 && self.prev_food > 20.0 {
                self.on_switch(SwitchKind::Hunger, index, world);
            } else if world.pet.strength_drink <= 20.0 && self.prev_drink > 20.0 {
                self.on_switch(SwitchKind::Thirsty, index, world);
            }
            self.prev_food = world.pet.strength_food;
            self.prev_drink = world.pet.strength_drink;
        }

        // Trigger occasional auto-walk, idle fidget, or alternate state from Default Idle.
        if matches!(self.state, ActionState::Idle) && !world.pet.sleeping {
            self.idle_elapsed_ms = self.idle_elapsed_ms.saturating_add(dt_ms);
            if self.idle_elapsed_ms >= 5_000 {
                self.idle_elapsed_ms = 0;
                let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
                let roll = world.rng.next_u64() % 100;
                if roll < 50 && self.auto_walk_enabled {
                    let towards_wall = world.rng.next_u64() % 100 < 35
                        || pos.0 < bounds.0 as i32 / 3
                        || pos.0 > (bounds.0 as i32 * 2) / 3;

                    let dir_x = if towards_wall {
                        if pos.0 < max_x / 2 { -1 } else { 1 }
                    } else if world.rng.next_u64() & 1 == 0 {
                        -1
                    } else {
                        1
                    };

                    let loops_remaining = if towards_wall { 10 } else { 4 };
                    self.request_state(
                        ActionState::Moving {
                            dir_x,
                            speed_px_per_sec: 56,
                            loops_remaining,
                        },
                        false,
                        index,
                        world,
                    );
                } else if roll < 78 {
                    self.request_state(ActionState::IdleFidget { name: None }, false, index, world);
                } else {
                    self.request_state(ActionState::StateOne, false, index, world);
                }
            }
        }

        // While the pet is held in the air, keep the swing/kick animation running and only
        // relax into the static blinking pose after the finger has been still for a while.
        if let ActionState::Raised {
            dropping: false,
            is_static,
        } = &mut self.state
        {
            self.drag_still_elapsed_ms = self.drag_still_elapsed_ms.saturating_add(dt_ms);
            if !*is_static && self.drag_still_elapsed_ms >= 1_400 {
                *is_static = true;
                self.enter_state_clip(index, world);
            }
        }

        // Hold edge position during SideHide so character aligns exactly with screen bezel.
        if let ActionState::SideHide { left, .. } = self.state {
            let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
            let offset_left = ((219_f32 * sprite_logical.0 as f32) / 500.0).round() as i32;
            let offset_right =
                (((500.0 - 281_f32) * sprite_logical.0 as f32) / 500.0).round() as i32;
            pos.0 = if left {
                -offset_left
            } else {
                max_x + offset_right
            };
        }

        // Climb up a wall, crawl across the ceiling, then drop back down to the desktop.
        if let ActionState::Climbing { kind, .. } = self.state {
            let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
            let offset_left = ((219_f32 * sprite_logical.0 as f32) / 500.0).round() as i32;
            let offset_right =
                (((500.0 - 281_f32) * sprite_logical.0 as f32) / 500.0).round() as i32;
            let offset_top = ((150_f32 * sprite_logical.1 as f32) / 500.0).round() as i32;
            match kind {
                ClimbKind::LeftWall => {
                    pos.0 = -offset_left;
                    self.walk_subpx += 95.0 * (f64::from(dt_ms) / 1000.0);
                    let step = self.walk_subpx.trunc() as i32;
                    if step > 0 {
                        self.walk_subpx -= f64::from(step);
                        pos.1 = pos.1.saturating_sub(step);
                        world.statistics.pixels_walked += u64::from(step.unsigned_abs());
                        world.output_positions.insert(output_name.to_owned(), *pos);
                        if pos.1 <= -offset_top {
                            pos.1 = -offset_top;
                            world.output_positions.insert(output_name.to_owned(), *pos);
                            self.request_state(
                                ActionState::Climbing {
                                    kind: ClimbKind::TopCeilingRight,
                                    loops_remaining: 6,
                                },
                                true,
                                index,
                                world,
                            );
                            return;
                        }
                    }
                }
                ClimbKind::RightWall => {
                    pos.0 = max_x + offset_right;
                    self.walk_subpx += 95.0 * (f64::from(dt_ms) / 1000.0);
                    let step = self.walk_subpx.trunc() as i32;
                    if step > 0 {
                        self.walk_subpx -= f64::from(step);
                        pos.1 = pos.1.saturating_sub(step);
                        world.statistics.pixels_walked += u64::from(step.unsigned_abs());
                        world.output_positions.insert(output_name.to_owned(), *pos);
                        if pos.1 <= -offset_top {
                            pos.1 = -offset_top;
                            world.output_positions.insert(output_name.to_owned(), *pos);
                            self.request_state(
                                ActionState::Climbing {
                                    kind: ClimbKind::TopCeilingLeft,
                                    loops_remaining: 6,
                                },
                                true,
                                index,
                                world,
                            );
                            return;
                        }
                    }
                }
                ClimbKind::TopCeilingRight => {
                    pos.1 = -offset_top;
                    self.walk_subpx += 65.0 * (f64::from(dt_ms) / 1000.0);
                    let step = self.walk_subpx.trunc() as i32;
                    if step > 0 {
                        self.walk_subpx -= f64::from(step);
                        pos.0 = (pos.0 + step).min(max_x);
                        world.statistics.pixels_walked += u64::from(step.unsigned_abs());
                        world.output_positions.insert(output_name.to_owned(), *pos);
                        if pos.0 >= max_x / 2 {
                            self.request_state(
                                ActionState::Falling { left: false },
                                true,
                                index,
                                world,
                            );
                            return;
                        }
                    }
                }
                ClimbKind::TopCeilingLeft => {
                    pos.1 = -offset_top;
                    self.walk_subpx += 65.0 * (f64::from(dt_ms) / 1000.0);
                    let step = self.walk_subpx.trunc() as i32;
                    if step > 0 {
                        self.walk_subpx -= f64::from(step);
                        pos.0 = (pos.0 - step).max(0);
                        world.statistics.pixels_walked += u64::from(step.unsigned_abs());
                        world.output_positions.insert(output_name.to_owned(), *pos);
                        if pos.0 <= max_x / 2 {
                            self.request_state(
                                ActionState::Falling { left: true },
                                true,
                                index,
                                world,
                            );
                            return;
                        }
                    }
                }
            }
        }

        // Gravity pulls the pet back to the floor after letting go of the ceiling.
        if let ActionState::Falling { .. } = self.state {
            let floor_y = (bounds.1 as i32 - sprite_logical.1 as i32).max(0);
            self.walk_subpx += 380.0 * (f64::from(dt_ms) / 1000.0);
            let step = self.walk_subpx.trunc() as i32;
            if step > 0 {
                self.walk_subpx -= f64::from(step);
                pos.1 = (pos.1 + step).min(floor_y);
                world.output_positions.insert(output_name.to_owned(), *pos);
                if pos.1 >= floor_y {
                    self.finish_transient_state(index, world);
                    return;
                }
            }
        }

        // Apply horizontal movement during `Moving` and bounce/climb cleanly at screen edges.
        if let ActionState::Moving {
            dir_x,
            speed_px_per_sec,
            loops_remaining: _,
        } = self.state
        {
            let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
            if pos.0 < 0 && dir_x > 0 {
                pos.0 = 0;
            } else if pos.0 > max_x && dir_x < 0 {
                pos.0 = max_x;
            }
            self.walk_subpx += f64::from(dir_x * speed_px_per_sec) * (f64::from(dt_ms) / 1000.0);
            let step_px = self.walk_subpx.trunc() as i32;
            if step_px != 0 {
                self.walk_subpx -= f64::from(step_px);
                let next_x = (pos.0 + step_px).clamp(0, max_x);
                let traveled = (next_x - pos.0).unsigned_abs();
                world.statistics.pixels_walked += u64::from(traveled);
                pos.0 = next_x;
                world.output_positions.insert(output_name.to_owned(), *pos);

                if next_x <= 25 && dir_x < 0 {
                    // Auto-walk never hides at the bezel: climb the wall or turn around.
                    if world.rng.next_u64() & 1 == 0 {
                        let offset_left =
                            ((219_f32 * sprite_logical.0 as f32) / 500.0).round() as i32;
                        pos.0 = -offset_left;
                        self.request_state(
                            ActionState::Climbing {
                                kind: ClimbKind::LeftWall,
                                loops_remaining: 8,
                            },
                            true,
                            index,
                            world,
                        );
                    } else {
                        pos.0 = 0;
                        self.request_state(
                            ActionState::Moving {
                                dir_x: 1,
                                speed_px_per_sec,
                                loops_remaining: 4,
                            },
                            true,
                            index,
                            world,
                        );
                    }
                    return;
                } else if next_x >= max_x.saturating_sub(25) && dir_x > 0 {
                    if world.rng.next_u64() & 1 == 0 {
                        let offset_right =
                            (((500.0 - 281_f32) * sprite_logical.0 as f32) / 500.0).round() as i32;
                        pos.0 = max_x + offset_right;
                        self.request_state(
                            ActionState::Climbing {
                                kind: ClimbKind::RightWall,
                                loops_remaining: 8,
                            },
                            true,
                            index,
                            world,
                        );
                    } else {
                        pos.0 = max_x;
                        self.request_state(
                            ActionState::Moving {
                                dir_x: -1,
                                speed_px_per_sec,
                                loops_remaining: 4,
                            },
                            true,
                            index,
                            world,
                        );
                    }
                    return;
                }
            }
        }

        self.ensure_clip(index, world);

        let Some(clip) = self.active_clip.as_mut() else {
            return;
        };

        // If real frame durations are not available yet from cache (e.g. background worker is decoding),
        // wait for the clip to finish decoding rather than prematurely finishing in 125ms!
        let Some(durs) = frame_durations_ms.filter(|s| !s.is_empty()) else {
            clip.frame_elapsed_ms = clip.frame_elapsed_ms.saturating_add(dt_ms);
            // Safety timeout: if still not loaded after 5 seconds, finish the transient state
            if clip.frame_elapsed_ms >= 5000 {
                self.finish_transient_state(index, world);
            }
            return;
        };

        if clip.frame_index >= durs.len() {
            clip.frame_index = 0;
        }
        clip.frame_elapsed_ms = clip.frame_elapsed_ms.saturating_add(dt_ms);

        let mut clip_finished = false;
        while clip.frame_elapsed_ms >= durs[clip.frame_index] {
            clip.frame_elapsed_ms -= durs[clip.frame_index];
            if clip.frame_index + 1 < durs.len() {
                clip.frame_index += 1;
            } else {
                clip.frame_index = 0;
                match &mut clip.phase {
                    SegmentPhase::AStart | SegmentPhase::CEnd => {
                        clip_finished = true;
                        break;
                    }
                    SegmentPhase::BLoop { remaining_loops } => {
                        if let Some(rem) = remaining_loops {
                            *rem = rem.saturating_sub(1);
                            if *rem == 0 {
                                clip_finished = true;
                                break;
                            }
                        }
                    }
                    SegmentPhase::Single { looped } => {
                        if !*looped {
                            clip_finished = true;
                            break;
                        }
                    }
                }
            }
        }

        if clip_finished {
            self.on_segment_finished(index, world);
        }
    }

    fn enter_state_clip(&mut self, index: &AnimationIndex, world: &mut World) {
        let mode = world.pet.mode;
        let seed = world.rng.next_u64();

        let resolved = match &self.state {
            ActionState::StartUp => {
                resolve_chain_start(index, GraphType::StartUp, None, mode, seed, Some(1), false)
            }
            ActionState::Shutdown { .. } => {
                resolve_chain_start(index, GraphType::Shutdown, None, mode, seed, Some(1), false)
            }
            ActionState::Idle => {
                resolve_chain_start(index, GraphType::Default, None, mode, seed, None, true)
            }
            ActionState::IdleFidget { name } => resolve_chain_start(
                index,
                GraphType::Idel,
                name.as_deref(),
                mode,
                seed,
                Some(1),
                false,
            ),
            ActionState::StateOne => {
                resolve_chain_start(index, GraphType::StateOne, None, mode, seed, Some(1), false)
            }
            ActionState::SideHide { left, emerging } => {
                let gt = match (left, emerging) {
                    (true, false) => GraphType::SideHideLeftMain,
                    (true, true) => GraphType::SideHideLeftRise,
                    (false, false) => GraphType::SideHideRightMain,
                    (false, true) => GraphType::SideHideRightRise,
                };
                resolve_chain_start(index, gt, None, mode, seed, Some(1), false)
                    .or_else(|| resolve_c_end_or_single(index, gt, None, mode, seed))
            }
            ActionState::Sleeping { waking: false } => {
                resolve_chain_start(index, GraphType::Sleep, None, mode, seed, None, true)
            }
            ActionState::Sleeping { waking: true } => {
                resolve_c_end_or_single(index, GraphType::Sleep, None, mode, seed)
            }
            ActionState::Moving {
                dir_x,
                loops_remaining,
                ..
            } => {
                let walk_name = if *dir_x < 0 {
                    "walk.left"
                } else {
                    "walk.right"
                };
                resolve_chain_start(
                    index,
                    GraphType::Move,
                    Some(walk_name),
                    mode,
                    seed,
                    Some(*loops_remaining),
                    false,
                )
            }
            ActionState::Climbing {
                kind,
                loops_remaining,
            } => {
                let climb_name = match kind {
                    ClimbKind::LeftWall => "climb.left",
                    ClimbKind::RightWall => "climb.right",
                    ClimbKind::TopCeilingLeft => "climb.top.left",
                    ClimbKind::TopCeilingRight => "climb.top.right",
                };
                resolve_chain_start(
                    index,
                    GraphType::Move,
                    Some(climb_name),
                    mode,
                    seed,
                    Some(*loops_remaining),
                    false,
                )
            }
            ActionState::Falling { left } => {
                let fall_name = if *left { "fall.left" } else { "fall.right" };
                resolve_chain_start(
                    index,
                    GraphType::Move,
                    Some(fall_name),
                    mode,
                    seed,
                    None,
                    false,
                )
            }
            ActionState::Saying => {
                resolve_chain_start(index, GraphType::Say, None, mode, seed, Some(1), false)
            }
            ActionState::Thinking => resolve_chain_start(
                index,
                GraphType::Common,
                Some("think"),
                mode,
                seed,
                None,
                true,
            )
            .or_else(|| resolve_chain_start(index, GraphType::Say, None, mode, seed, None, true)),
            ActionState::Working { graph } => resolve_chain_start(
                index,
                GraphType::Work,
                Some(graph.as_str()),
                mode,
                seed,
                None,
                true,
            ),
            ActionState::Touch { kind } => {
                resolve_chain_start(index, kind.graph_type(), None, mode, seed, Some(1), false)
            }
            ActionState::Pinch => resolve_chain_start(
                index,
                GraphType::Common,
                Some("pinch"),
                mode,
                seed,
                Some(1),
                false,
            ),
            ActionState::Eating {
                graph,
                food_name,
                icon_path,
            } => {
                let name = match graph.to_ascii_lowercase().as_str() {
                    "drink" => "drink",
                    "gift" => "gift",
                    _ => "eat",
                };
                if let Some((entry, phase, name_opt)) = resolve_chain_start(
                    index,
                    GraphType::Common,
                    Some(name),
                    mode,
                    seed,
                    Some(1),
                    false,
                ) {
                    let mut source = entry.source.clone();
                    if let AnimationSource::Food { item_icon_path, .. } = &mut source {
                        *item_icon_path = icon_path.clone();
                    }
                    let cache_key = if let Some(fn_name) = food_name {
                        format!(
                            "{}:{}:{}",
                            clip_cache_key(entry, self.target_px),
                            fn_name,
                            self.target_px
                        )
                    } else {
                        clip_cache_key(entry, self.target_px)
                    };
                    self.active_clip = Some(ActiveClip {
                        cache_key,
                        graph_type: entry.graph_type,
                        name: name_opt,
                        mode,
                        phase,
                        source,
                        frame_index: 0,
                        frame_elapsed_ms: 0,
                        seed,
                    });
                    return;
                }
                None
            }
            ActionState::LevelUp => resolve_chain_start(
                index,
                GraphType::Common,
                Some("levelup"),
                mode,
                seed,
                Some(1),
                false,
            ),
            ActionState::Switch { kind } => {
                let gt = match kind {
                    SwitchKind::Up => GraphType::SwitchUp,
                    SwitchKind::Down => GraphType::SwitchDown,
                    SwitchKind::Hunger => GraphType::SwitchHunger,
                    SwitchKind::Thirsty => GraphType::SwitchThirsty,
                };
                resolve_chain_start(index, gt, None, mode, seed, Some(1), false)
            }
            ActionState::Raised {
                dropping: false,
                is_static: false,
            } => resolve_chain_start(
                index,
                GraphType::RaisedDynamic,
                None,
                mode,
                seed,
                None,
                true,
            ),
            ActionState::Raised {
                dropping: false,
                is_static: true,
            } => resolve_chain_start(index, GraphType::RaisedStatic, None, mode, seed, None, true),
            ActionState::Raised { dropping: true, .. } => {
                resolve_c_end_or_single(index, GraphType::RaisedStatic, None, mode, seed)
            }
        };

        self.active_clip = resolved.map(|(entry, phase, name_opt)| ActiveClip {
            cache_key: clip_cache_key(entry, self.target_px),
            graph_type: entry.graph_type,
            name: name_opt,
            mode,
            phase,
            source: entry.source.clone(),
            frame_index: 0,
            frame_elapsed_ms: 0,
            seed,
        });

        // If a drop/wake/transient transition had no dedicated clip, return immediately to base state.
        if self.active_clip.is_none()
            && matches!(
                self.state,
                ActionState::Raised { dropping: true, .. }
                    | ActionState::Sleeping { waking: true }
                    | ActionState::Touch { .. }
                    | ActionState::Pinch
                    | ActionState::Eating { .. }
                    | ActionState::LevelUp
                    | ActionState::Switch { .. }
                    | ActionState::IdleFidget { .. }
                    | ActionState::StateOne
                    | ActionState::StartUp
                    | ActionState::Saying
                    | ActionState::Thinking
            )
        {
            self.finish_transient_state(index, world);
        }
    }

    fn on_segment_finished(&mut self, index: &AnimationIndex, world: &mut World) {
        let Some(prev) = self.active_clip.clone() else {
            return;
        };

        let loop_budget = match &self.state {
            ActionState::Touch { .. } | ActionState::Saying | ActionState::SideHide { .. } => {
                Some(1)
            }
            ActionState::Moving {
                loops_remaining, ..
            }
            | ActionState::Climbing {
                loops_remaining, ..
            } => Some(*loops_remaining),
            _ => None,
        };

        match prev.phase {
            SegmentPhase::AStart => {
                if let Some(b_entry) = index.pick(
                    prev.graph_type,
                    prev.name.as_deref(),
                    AnimatType::BLoop,
                    prev.mode,
                    prev.seed,
                ) {
                    self.active_clip = Some(ActiveClip {
                        cache_key: clip_cache_key(b_entry, self.target_px),
                        phase: SegmentPhase::BLoop {
                            remaining_loops: loop_budget,
                        },
                        source: b_entry.source.clone(),
                        frame_index: 0,
                        frame_elapsed_ms: 0,
                        ..prev
                    });
                    return;
                }
                if let Some(c_entry) = index.pick(
                    prev.graph_type,
                    prev.name.as_deref(),
                    AnimatType::CEnd,
                    prev.mode,
                    prev.seed,
                ) {
                    self.active_clip = Some(ActiveClip {
                        cache_key: clip_cache_key(c_entry, self.target_px),
                        phase: SegmentPhase::CEnd,
                        source: c_entry.source.clone(),
                        frame_index: 0,
                        frame_elapsed_ms: 0,
                        ..prev
                    });
                    return;
                }
                self.finish_transient_state(index, world);
            }
            SegmentPhase::BLoop { .. } => {
                // Wall climbs loop forever until the pet reaches the ceiling.
                if matches!(
                    self.state,
                    ActionState::Climbing {
                        kind: ClimbKind::LeftWall | ClimbKind::RightWall,
                        ..
                    }
                ) {
                    return;
                }
                // Held still: keep blinking until the finger moves or the pet is dropped.
                if matches!(
                    self.state,
                    ActionState::Raised {
                        dropping: false,
                        is_static: true
                    }
                ) {
                    return;
                }
                if let Some(c_entry) = index.pick(
                    prev.graph_type,
                    prev.name.as_deref(),
                    AnimatType::CEnd,
                    prev.mode,
                    prev.seed,
                ) {
                    self.active_clip = Some(ActiveClip {
                        cache_key: clip_cache_key(c_entry, self.target_px),
                        phase: SegmentPhase::CEnd,
                        source: c_entry.source.clone(),
                        frame_index: 0,
                        frame_elapsed_ms: 0,
                        ..prev
                    });
                    return;
                }
                self.finish_transient_state(index, world);
            }
            SegmentPhase::CEnd => {
                self.finish_transient_state(index, world);
            }
            SegmentPhase::Single { looped } => {
                // A held pet keeps cycling the swing/kick animation without ever settling.
                if let ActionState::Raised {
                    dropping: false,
                    is_static: false,
                } = self.state
                {
                    let seed = world.rng.next_u64();
                    if let Some((entry, phase, name_opt)) = resolve_chain_start(
                        index,
                        GraphType::RaisedDynamic,
                        None,
                        world.pet.mode,
                        seed,
                        None,
                        true,
                    ) {
                        self.active_clip = Some(ActiveClip {
                            cache_key: clip_cache_key(entry, self.target_px),
                            graph_type: entry.graph_type,
                            name: name_opt,
                            mode: world.pet.mode,
                            phase,
                            source: entry.source.clone(),
                            frame_index: 0,
                            frame_elapsed_ms: 0,
                            seed,
                        });
                        return;
                    }
                }
                if !looped {
                    self.finish_transient_state(index, world);
                }
            }
        }
    }

    fn finish_transient_state(&mut self, index: &AnimationIndex, world: &mut World) {
        match self.state {
            ActionState::Raised {
                dropping: false, ..
            } => {
                return;
            }
            ActionState::Shutdown { .. } => {
                self.shutdown_ready = true;
                return;
            }
            ActionState::Climbing { kind, .. } => match kind {
                ClimbKind::TopCeilingLeft => {
                    self.state = ActionState::Falling { left: true };
                    self.active_clip = None;
                    self.enter_state_clip(index, world);
                    return;
                }
                ClimbKind::TopCeilingRight => {
                    self.state = ActionState::Falling { left: false };
                    self.active_clip = None;
                    self.enter_state_clip(index, world);
                    return;
                }
                ClimbKind::LeftWall | ClimbKind::RightWall => {}
            },
            ActionState::SideHide {
                left,
                emerging: false,
            } => {
                self.state = ActionState::SideHide {
                    left,
                    emerging: true,
                };
                self.active_clip = None;
                self.enter_state_clip(index, world);
                return;
            }
            ActionState::SideHide {
                left,
                emerging: true,
            } => {
                let dir_x = if left { 1 } else { -1 };
                self.state = ActionState::Moving {
                    dir_x,
                    speed_px_per_sec: 48,
                    loops_remaining: 3,
                };
                self.active_clip = None;
                self.enter_state_clip(index, world);
                return;
            }
            _ => {}
        }

        self.state = if world.pet.sleeping {
            ActionState::Sleeping { waking: false }
        } else if let Some(work) = &world.active_work {
            ActionState::Working {
                graph: work.graph.clone(),
            }
        } else {
            ActionState::Idle
        };
        self.active_clip = None;
        self.enter_state_clip(index, world);
    }
}

fn resolve_chain_start<'a>(
    index: &'a AnimationIndex,
    graph_type: GraphType,
    name: Option<&str>,
    mode: ModeType,
    seed: u64,
    loop_budget: Option<u32>,
    loop_single: bool,
) -> Option<(&'a AnimationEntry, SegmentPhase, Option<String>)> {
    let owned_name = name.map(str::to_owned);
    if let Some(a) = index.pick(graph_type, name, AnimatType::AStart, mode, seed) {
        return Some((a, SegmentPhase::AStart, owned_name));
    }
    if let Some(b) = index.pick(graph_type, name, AnimatType::BLoop, mode, seed) {
        return Some((
            b,
            SegmentPhase::BLoop {
                remaining_loops: loop_budget,
            },
            owned_name,
        ));
    }
    if let Some(s) = index.pick(graph_type, name, AnimatType::Single, mode, seed) {
        return Some((
            s,
            SegmentPhase::Single {
                looped: loop_single,
            },
            owned_name,
        ));
    }
    None
}

fn resolve_c_end_or_single<'a>(
    index: &'a AnimationIndex,
    graph_type: GraphType,
    name: Option<&str>,
    mode: ModeType,
    seed: u64,
) -> Option<(&'a AnimationEntry, SegmentPhase, Option<String>)> {
    let owned_name = name.map(str::to_owned);
    if let Some(c) = index.pick(graph_type, name, AnimatType::CEnd, mode, seed) {
        return Some((c, SegmentPhase::CEnd, owned_name));
    }
    if let Some(s) = index.pick(graph_type, name, AnimatType::Single, mode, seed) {
        return Some((s, SegmentPhase::Single { looped: false }, owned_name));
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use vpet_content::TextDatabase;

    #[test]
    fn hit_zone_scales_from_500x500_vup_coordinates() {
        let cfg = PetConfig::default();
        // At 250x250 display size (0.5x of 500), head (159..348, 16..194) maps to ~(80..174, 8..97).
        assert_eq!(
            classify_hit_zone(&cfg, (250, 250), 120.0, 50.0),
            HitZone::Head
        );
        // Body (166..329, 206..342) maps to ~(83..164, 103..171).
        assert_eq!(
            classify_hit_zone(&cfg, (250, 250), 120.0, 130.0),
            HitZone::Body
        );
        // Pinch (149..205, 128..187) maps to ~(75..102, 64..93).
        assert_eq!(
            classify_hit_zone(&cfg, (250, 250), 85.0, 75.0),
            HitZone::Pinch
        );
    }

    #[test]
    fn headless_state_machine_chains_abc_segments_and_edge_bounces_on_real_vup() {
        let vup_dir =
            Path::new("/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup");
        if !vup_dir.exists() {
            return;
        }

        let idx = AnimationIndex::scan_pet_dir("vup", vup_dir);
        let texts = TextDatabase::default();
        let mut world = World::new("test", "Loris", 1234);
        let mut ctrl = InteractionController::new(250, false);
        let mut pos = (10, 760);

        ctrl.ensure_clip(&idx, &mut world);
        assert_eq!(ctrl.state(), &ActionState::Idle);

        // Head click enters TouchHead AStart -> BLoop -> CEnd -> back to Idle.
        ctrl.on_click_zone(HitZone::Head, &texts, &idx, &mut world);
        assert_eq!(
            ctrl.state(),
            &ActionState::Touch {
                kind: TouchKind::Head
            }
        );
        assert_eq!(ctrl.active_clip().unwrap().phase, SegmentPhase::AStart);

        // Lower-priority Moving cannot preempt active Touch!
        assert!(!ctrl.request_state(
            ActionState::Moving {
                dir_x: -1,
                speed_px_per_sec: 100,
                loops_remaining: 2,
            },
            false,
            &idx,
            &mut world
        ));

        // Step through AStart -> BLoop -> CEnd -> Idle.
        for _ in 0..10 {
            ctrl.advance(
                150,
                Some(&[100, 100]),
                &mut pos,
                AdvanceViewport {
                    bounds: (1920, 1080),
                    sprite_logical: (250, 250),
                    output_name: "eDP-1",
                },
                &idx,
                &mut world,
            );
        }
        assert_eq!(ctrl.state(), &ActionState::Idle);

        // Higher-priority Drag/Raised preempts anything and saves output position on drop.
        ctrl.on_drag_start(&idx, &mut world);
        assert_eq!(
            ctrl.state(),
            &ActionState::Raised {
                dropping: false,
                is_static: false
            }
        );
        ctrl.on_drag_motion(&idx, &mut world);
        ctrl.on_drag_end(
            "eDP-1",
            (320, 640),
            (1920, 1080),
            (250, 250),
            &idx,
            &mut world,
        );
        assert_eq!(world.output_positions.get("eDP-1"), Some(&(320, 640)));

        // Dropping within 60px of the screen edge triggers SideHide immediately.
        ctrl.on_drag_start(&idx, &mut world);
        ctrl.on_drag_end(
            "eDP-1",
            (40, 640),
            (1920, 1080),
            (250, 250),
            &idx,
            &mut world,
        );
        assert!(matches!(
            ctrl.state(),
            ActionState::SideHide { left: true, .. }
        ));

        // Auto-walk hitting the left bezel climbs the wall or turns around, never SideHide.
        pos = (2, 760);
        ctrl.request_state(
            ActionState::Moving {
                dir_x: -1,
                speed_px_per_sec: 100,
                loops_remaining: 2,
            },
            true,
            &idx,
            &mut world,
        );
        ctrl.advance(
            100,
            Some(&[125, 125]),
            &mut pos,
            AdvanceViewport {
                bounds: (1920, 1080),
                sprite_logical: (250, 250),
                output_name: "eDP-1",
            },
            &idx,
            &mut world,
        );
        assert!(matches!(
            ctrl.state(),
            ActionState::Climbing {
                kind: ClimbKind::LeftWall,
                ..
            } | ActionState::Moving { dir_x: 1, .. }
        ));

        // Dropping the pet at the top bezel makes her climb across the ceiling.
        ctrl.on_drag_start(&idx, &mut world);
        ctrl.on_drag_end(
            "eDP-1",
            (900, 10),
            (1920, 1080),
            (250, 250),
            &idx,
            &mut world,
        );
        assert!(matches!(
            ctrl.state(),
            ActionState::Climbing {
                kind: ClimbKind::TopCeilingLeft,
                ..
            }
        ));
        assert_eq!(world.output_positions.get("eDP-1"), Some(&(900, -75)));

        // Test pinch face triggers Pinch animation state
        ctrl.on_click_zone(HitZone::Pinch, &texts, &idx, &mut world);
        assert_eq!(ctrl.state(), &ActionState::Pinch);

        // Test feeding triggers Eating animation state
        ctrl.on_feed("eat", Some("Apple".to_owned()), None, &idx, &mut world);
        assert!(matches!(ctrl.state(), ActionState::Eating { .. }));

        // Test level up animation
        ctrl.on_level_up(&idx, &mut world);
        assert_eq!(ctrl.state(), &ActionState::LevelUp);

        // Test startup & shutdown
        ctrl.request_state(ActionState::StartUp, true, &idx, &mut world);
        assert_eq!(ctrl.state(), &ActionState::StartUp);
        ctrl.trigger_shutdown(&idx, &mut world);
        assert!(matches!(ctrl.state(), ActionState::Shutdown { .. }));
    }
}
