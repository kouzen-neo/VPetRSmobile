#![allow(dead_code)]
//! Priority animation state machine, `A_Start -> B_Loop -> C_End` segment player,
//! `500x500` logical hit-region classifier, auto-walk, and headless interaction controller.

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

/// High-level behavioral state ordered by priority:
/// `Raised (70) > Touch (60) > Working (50) > Saying (40) > Moving (30) > Idle (20) > Sleeping (10)`
/// (Note: explicit `Sleeping` holds until woken, while `Touch` / `Raised` can temporarily preempt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionState {
    Idle,
    Sleeping {
        waking: bool,
    },
    Moving {
        dir_x: i32,
        speed_px_per_sec: i32,
        loops_remaining: u32,
    },
    /// Idle fidget from the `IDEL` graph (boring, squat, yawning, meow, ...).
    Fidget {
        name: String,
    },
    Saying,
    Working {
        graph: String,
    },
    Touch {
        kind: TouchKind,
    },
    Raised {
        dropping: bool,
    },
    SideHide {
        left: bool,
        rising: bool,
    },
}

impl ActionState {
    #[must_use]
    pub const fn priority(&self) -> u8 {
        match self {
            Self::Sleeping { .. } => 10,
            Self::Idle => 20,
            Self::Fidget { .. } => 25,
            Self::Moving { .. } => 30,
            Self::Saying => 40,
            Self::Working { .. } => 50,
            Self::Touch { .. } => 60,
            Self::SideHide { .. } => 65,
            Self::Raised { .. } => 70,
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
        }
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
        self.request_state(ActionState::Raised { dropping: false }, true, index, world);
    }

    pub fn on_drag_end(
        &mut self,
        output_name: &str,
        pos: (i32, i32),
        index: &AnimationIndex,
        world: &mut World,
    ) {
        world.output_positions.insert(output_name.to_owned(), pos);
        if pos.0 <= 20 {
            self.request_state(
                ActionState::SideHide {
                    left: true,
                    rising: false,
                },
                true,
                index,
                world,
            );
        } else {
            self.request_state(ActionState::Raised { dropping: true }, true, index, world);
        }
    }

    pub fn on_click_zone(
        &mut self,
        zone: HitZone,
        texts: &vpet_content::TextDatabase,
        index: &AnimationIndex,
        world: &mut World,
    ) {
        if let ActionState::SideHide {
            left,
            rising: false,
        } = self.state
        {
            self.request_state(
                ActionState::SideHide { left, rising: true },
                true,
                index,
                world,
            );
            self.show_bubble("Peekaboo!", 3_000);
            return;
        }

        let kind = match zone {
            HitZone::Head | HitZone::Pinch => {
                let _ = world.pet_head();
                TouchKind::Head
            }
            HitZone::Body => {
                let _ = world.pet_body();
                TouchKind::Body
            }
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

        // Trigger occasional auto-walk or idle fidget from Default Idle.
        if matches!(self.state, ActionState::Idle) && !world.pet.sleeping {
            self.idle_elapsed_ms = self.idle_elapsed_ms.saturating_add(dt_ms);
            if self.auto_walk_enabled && self.idle_elapsed_ms >= 8_000 {
                self.idle_elapsed_ms = 0;
                let roll = world.rng.next_u64();
                let fidgets = idle_fidget_names(index);
                // ~60% fidget, ~40% walk (walk only when not dragged into a corner).
                if !fidgets.is_empty() && roll % 5 < 3 {
                    let name = fidgets[(roll / 5) as usize % fidgets.len()].clone();
                    self.request_state(ActionState::Fidget { name }, false, index, world);
                } else {
                    let dir_x = if roll & 1 == 0 { -1 } else { 1 };
                    self.request_state(
                        ActionState::Moving {
                            dir_x,
                            speed_px_per_sec: 48,
                            loops_remaining: 3,
                        },
                        false,
                        index,
                        world,
                    );
                }
            }
        }

        // Apply horizontal movement during `Moving` and bounce/flip cleanly at screen edges.
        if let ActionState::Moving {
            dir_x,
            speed_px_per_sec,
            loops_remaining,
        } = self.state
        {
            let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
            self.walk_subpx += f64::from(dir_x * speed_px_per_sec) * (f64::from(dt_ms) / 1000.0);
            let step_px = self.walk_subpx.trunc() as i32;
            if step_px != 0 {
                self.walk_subpx -= f64::from(step_px);
                let next_x = (pos.0 + step_px).clamp(0, max_x);
                let traveled = (next_x - pos.0).unsigned_abs();
                world.statistics.pixels_walked += u64::from(traveled);
                pos.0 = next_x;
                world.output_positions.insert(output_name.to_owned(), *pos);

                if (next_x == 0 && dir_x < 0) || (next_x == max_x && dir_x > 0) {
                    let is_left = next_x == 0;
                    let roll = world.rng.next_u64();
                    if roll.is_multiple_of(3) {
                        self.request_state(
                            ActionState::SideHide {
                                left: is_left,
                                rising: false,
                            },
                            true,
                            index,
                            world,
                        );
                        return;
                    }
                    let flipped = -dir_x;
                    self.request_state(
                        ActionState::Moving {
                            dir_x: flipped,
                            speed_px_per_sec,
                            loops_remaining,
                        },
                        true,
                        index,
                        world,
                    );
                    return;
                }
            }
        }

        self.ensure_clip(index, world);
        let fallback_durs = [125_u32];
        let durs = frame_durations_ms
            .filter(|s| !s.is_empty())
            .unwrap_or(&fallback_durs);

        let Some(clip) = self.active_clip.as_mut() else {
            return;
        };

        if clip.frame_index >= durs.len() {
            clip.frame_index = 0;
        }
        clip.frame_elapsed_ms = clip.frame_elapsed_ms.saturating_add(dt_ms);

        let mut clip_finished = false;
        while clip.frame_elapsed_ms >= durs[clip.frame_index].max(20) {
            let dur = durs[clip.frame_index].max(20);
            clip.frame_elapsed_ms -= dur;
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
            ActionState::Idle => {
                resolve_chain_start(index, GraphType::Default, None, mode, seed, None, true)
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
            ActionState::Saying => {
                resolve_chain_start(index, GraphType::Say, None, mode, seed, Some(1), false)
            }
            ActionState::Fidget { name } => resolve_chain_start(
                index,
                GraphType::Idel,
                Some(name.as_str()),
                mode,
                seed,
                Some(3),
                false,
            ),
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
            ActionState::Raised { dropping: false } => resolve_chain_start(
                index,
                GraphType::RaisedDynamic,
                None,
                mode,
                seed,
                None,
                true,
            ),
            ActionState::Raised { dropping: true } => {
                resolve_c_end_or_single(index, GraphType::RaisedStatic, None, mode, seed)
            }
            ActionState::SideHide {
                left,
                rising: false,
            } => {
                let gt = if *left {
                    GraphType::SideHideLeftMain
                } else {
                    GraphType::SideHideRightMain
                };
                resolve_chain_start(index, gt, None, mode, seed, None, true)
            }
            ActionState::SideHide { left, rising: true } => {
                let gt = if *left {
                    GraphType::SideHideLeftRise
                } else {
                    GraphType::SideHideRightRise
                };
                resolve_chain_start(index, gt, None, mode, seed, Some(1), false)
                    .or_else(|| resolve_c_end_or_single(index, gt, None, mode, seed))
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

        // If a drop/wake transition had no dedicated CEnd clip, return immediately to base state.
        if self.active_clip.is_none()
            && matches!(
                self.state,
                ActionState::Raised { dropping: true }
                    | ActionState::Sleeping { waking: true }
                    | ActionState::Touch { .. }
                    | ActionState::Saying
                    | ActionState::Fidget { .. }
                    | ActionState::SideHide { rising: true, .. }
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
            ActionState::Touch { .. } | ActionState::Saying => Some(1),
            ActionState::Fidget { .. } => Some(3),
            ActionState::SideHide { rising: true, .. } => Some(1),
            ActionState::SideHide { rising: false, .. } => None,
            ActionState::Moving {
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
            SegmentPhase::CEnd | SegmentPhase::Single { .. } => {
                self.finish_transient_state(index, world);
            }
        }
    }

    fn finish_transient_state(&mut self, index: &AnimationIndex, world: &mut World) {
        self.state = if world.pet.sleeping {
            ActionState::Sleeping { waking: false }
        } else {
            ActionState::Idle
        };
        self.active_clip = None;
        self.enter_state_clip(index, world);
    }
}

/// Distinct `IDEL` fidget names available in `index`, in stable order.
fn idle_fidget_names(index: &AnimationIndex) -> Vec<String> {
    let mut names: Vec<String> = index
        .entries()
        .iter()
        .filter(|e| e.graph_type == GraphType::Idel && !e.name.is_empty())
        .map(|e| e.name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
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
        assert_eq!(ctrl.state(), &ActionState::Raised { dropping: false });
        ctrl.on_drag_end("eDP-1", (320, 640), &idx, &mut world);
        assert_eq!(world.output_positions.get("eDP-1"), Some(&(320, 640)));

        // Auto-walk hitting left screen edge (x = 0) flips direction to +1 cleanly.
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
        assert_eq!(pos.0, 0);
        assert!(matches!(ctrl.state(), ActionState::Moving { dir_x: 1, .. }));
    }

    #[test]
    fn idle_timer_triggers_walk_or_fidget_on_real_vup() {
        let vup_dir =
            Path::new("/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup");
        if !vup_dir.exists() {
            return;
        }
        let idx = AnimationIndex::scan_pet_dir("vup", vup_dir);
        let mut world = World::new("test", "Loris", 42);
        let mut ctrl = InteractionController::new(250, true);
        let mut pos = (500, 0);
        ctrl.ensure_clip(&idx, &mut world);

        let mut saw_fidget = false;
        let mut saw_walk = false;
        for _ in 0..2_000 {
            ctrl.advance(
                100,
                None,
                &mut pos,
                AdvanceViewport {
                    bounds: (1080, 2400),
                    sprite_logical: (250, 250),
                    output_name: "screen",
                },
                &idx,
                &mut world,
            );
            match ctrl.state() {
                ActionState::Fidget { .. } => saw_fidget = true,
                ActionState::Moving { .. } => saw_walk = true,
                _ => {}
            }
        }
        assert!(saw_fidget, "idle never played an IDEL fidget");
        assert!(saw_walk, "idle never auto-walked");
    }

    #[test]
    fn test_side_hide_indexed() {
        let vup_dir =
            Path::new("/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup");
        if !vup_dir.exists() {
            return;
        }
        let idx = AnimationIndex::scan_pet_dir("vup", vup_dir);
        let q_left = idx.query(
            GraphType::SideHideLeftMain,
            None,
            AnimatType::BLoop,
            ModeType::Nomal,
        );
        let q_left_rise = idx.query(
            GraphType::SideHideLeftRise,
            None,
            AnimatType::BLoop,
            ModeType::Nomal,
        );
        let q_right = idx.query(
            GraphType::SideHideRightMain,
            None,
            AnimatType::BLoop,
            ModeType::Nomal,
        );
        let q_right_rise = idx.query(
            GraphType::SideHideRightRise,
            None,
            AnimatType::BLoop,
            ModeType::Nomal,
        );
        eprintln!(
            "SideHide entries: left_main={}, left_rise={}, right_main={}, right_rise={}",
            q_left.len(),
            q_left_rise.len(),
            q_right.len(),
            q_right_rise.len()
        );
        assert!(!q_left.is_empty(), "SideHideLeftMain not indexed!");
        assert!(!q_left_rise.is_empty(), "SideHideLeftRise not indexed!");
        assert!(!q_right.is_empty(), "SideHideRightMain not indexed!");
        assert!(!q_right_rise.is_empty(), "SideHideRightRise not indexed!");
    }
}
