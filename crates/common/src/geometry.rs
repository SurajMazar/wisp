//! Geometry in macOS global display space.
//!
//! All coordinates crossing IPC are in **points** in a top-left-origin global space
//! (the CoreGraphics convention: origin at the top-left of the main display, y grows down).
//! Conversion to pixels uses each display's backing scale factor.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.width
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.max_x() && p.y >= self.y && p.y < self.max_y()
    }

    /// Intersection of two rects, or `None` if they don't overlap.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.max_x().min(other.max_x());
        let y1 = self.max_y().min(other.max_y());
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    /// Grow by `margin` on every side.
    pub fn inset_by(&self, margin: f64) -> Rect {
        Rect::new(
            self.x - margin,
            self.y - margin,
            self.width + 2.0 * margin,
            self.height + 2.0 * margin,
        )
    }

    /// Smallest rect enclosing all points (e.g. a lasso path). `None` for an empty slice.
    pub fn bounding(points: &[Point]) -> Option<Rect> {
        let first = points.first()?;
        let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
        for p in &points[1..] {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_and_containment() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        assert_eq!(a.intersection(&b), Some(Rect::new(50.0, 50.0, 50.0, 50.0)));
        assert_eq!(a.intersection(&Rect::new(200.0, 0.0, 10.0, 10.0)), None);
        assert!(a.contains(Point { x: 0.0, y: 0.0 }));
        assert!(!a.contains(Point { x: 100.0, y: 50.0 }));
    }

    #[test]
    fn bounding_box_of_lasso() {
        let pts = [
            Point { x: 10.0, y: 40.0 },
            Point { x: -5.0, y: 12.0 },
            Point { x: 30.0, y: 2.0 },
        ];
        assert_eq!(Rect::bounding(&pts), Some(Rect::new(-5.0, 2.0, 35.0, 38.0)));
        assert_eq!(Rect::bounding(&[]), None);
    }
}
