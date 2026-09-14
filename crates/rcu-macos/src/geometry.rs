#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.w, self.h]
            .iter()
            .all(|v| v.is_finite())
            && self.w > 0.0
            && self.h > 0.0
    }
    pub fn contains(self, p: [f64; 2]) -> bool {
        self.valid()
            && p[0] >= self.x
            && p[0] < self.x + self.w
            && p[1] >= self.y
            && p[1] < self.y + self.h
    }
    pub fn contains_rect(self, r: Self) -> bool {
        r.valid()
            && r.x >= self.x
            && r.y >= self.y
            && r.x + r.w <= self.x + self.w
            && r.y + r.h <= self.y + self.h
    }
    pub fn same(self, r: Self) -> bool {
        [self.x - r.x, self.y - r.y, self.w - r.w, self.h - r.h]
            .iter()
            .all(|d| d.abs() < 0.01)
    }
    pub fn map(self, pixels: [u32; 2]) -> Option<[f64; 6]> {
        if !self.valid() || pixels.contains(&0) {
            None
        } else {
            Some([
                self.w / pixels[0] as f64,
                0.0,
                0.0,
                self.h / pixels[1] as f64,
                self.x,
                self.y,
            ])
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offset_retina_mapping() {
        let r = Rect {
            x: -1920.0,
            y: -500.0,
            w: 900.0,
            h: 600.0,
        };
        assert_eq!(
            r.map([1800, 1200]),
            Some([0.5, 0.0, 0.0, 0.5, -1920.0, -500.0])
        );
        assert!(!r.contains([-1020.0, 0.0]));
    }
    #[test]
    fn crossing_display_is_not_single_display_mapping() {
        let d = Rect {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        assert!(!d.contains_rect(Rect {
            x: 1800.0,
            y: 0.0,
            w: 500.0,
            h: 600.0
        }));
    }
    #[test]
    fn same_and_disjoint_rects() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        assert!(r.same(r));
        assert!(!r.same(Rect { x: 1.0, ..r }));
        assert_eq!(r.map([0, 1]), None);
        assert!(!Rect { x: f64::NAN, ..r }.valid());
    }
}
