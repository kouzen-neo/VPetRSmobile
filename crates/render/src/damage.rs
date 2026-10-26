//! Damage rectangle math for partial buffer clears and `wl_surface.damage_buffer`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl DamageRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = self
            .x
            .saturating_add(self.w)
            .max(other.x.saturating_add(other.w));
        let y2 = self
            .y
            .saturating_add(self.h)
            .max(other.y.saturating_add(other.h));
        Self {
            x: x1,
            y: y1,
            w: x2.saturating_sub(x1),
            h: y2.saturating_sub(y1),
        }
    }

    #[must_use]
    pub fn clamp_to(self, buf_w: u32, buf_h: u32) -> Option<Self> {
        if self.is_empty() || buf_w == 0 || buf_h == 0 {
            return None;
        }
        let max_w = i32::try_from(buf_w).unwrap_or(i32::MAX);
        let max_h = i32::try_from(buf_h).unwrap_or(i32::MAX);
        let x1 = self.x.clamp(0, max_w);
        let y1 = self.y.clamp(0, max_h);
        let x2 = self.x.saturating_add(self.w).clamp(0, max_w);
        let y2 = self.y.saturating_add(self.h).clamp(0, max_h);
        let w = x2 - x1;
        let h = y2 - y1;
        if w <= 0 || h <= 0 {
            None
        } else {
            Some(Self { x: x1, y: y1, w, h })
        }
    }

    #[must_use]
    pub const fn scale(self, factor: i32) -> Self {
        let s = if factor < 1 { 1 } else { factor };
        Self {
            x: self.x.saturating_mul(s),
            y: self.y.saturating_mul(s),
            w: self.w.saturating_mul(s),
            h: self.h.saturating_mul(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DamageRect;

    #[test]
    fn union_handles_empty_and_disjoint_rects() {
        let empty = DamageRect::default();
        let a = DamageRect::new(10, 20, 30, 40);
        let b = DamageRect::new(50, 5, 20, 25);
        assert_eq!(empty.union(a), a);
        assert_eq!(a.union(empty), a);
        assert_eq!(a.union(b), DamageRect::new(10, 5, 60, 55));
    }

    #[test]
    fn clamp_clips_negative_and_overflow_bounds() {
        let r = DamageRect::new(-15, 1000, 50, 120);
        assert_eq!(
            r.clamp_to(1920, 1080),
            Some(DamageRect::new(0, 1000, 35, 80))
        );
        assert_eq!(DamageRect::new(-50, -50, 20, 20).clamp_to(1920, 1080), None);
    }
}
