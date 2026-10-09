//! Screen context and focus regions.
//!
//! Capture and cropping happen in the macOS host (ScreenCaptureKit); this crate validates what the
//! host reports before the rest of the runtime relies on it — a region that doesn't fit its display,
//! or an image whose size doesn't match the crop at the display's scale, is rejected.

pub mod describe;
pub mod guide;

use wisp_common::geometry::Rect;
use wisp_ipc::types::focus::{FocusKind, FocusRegion};

/// Slack for floating-point rounding when comparing point geometry.
const EPS: f64 = 1.0;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FocusError {
    #[error("selection is empty")]
    EmptySelection,
    #[error("{0} lies outside the display")]
    OutsideDisplay(&'static str),
    #[error("crop does not contain the selection")]
    CropMissesSelection,
    #[error("display scale {0} is not plausible")]
    BadScale(f64),
    #[error("image is {got:?} px but the crop at this scale is {expected:?} px")]
    ImageSizeMismatch {
        got: (u32, u32),
        expected: (u32, u32),
    },
    #[error("image URL is not a Wisp focus URL")]
    BadImageUrl,
    #[error("a {0:?} focus needs a path")]
    MissingPath(FocusKind),
}

fn within(inner: &Rect, outer: &Rect) -> bool {
    inner.x >= outer.x - EPS
        && inner.y >= outer.y - EPS
        && inner.max_x() <= outer.max_x() + EPS
        && inner.max_y() <= outer.max_y() + EPS
}

/// Expected pixel size of `crop` on a display with `scale`.
pub fn pixel_size(crop: &Rect, scale: f64) -> (u32, u32) {
    let px = |v: f64| (v * scale).round().max(0.0) as u32;
    (px(crop.width), px(crop.height))
}

pub fn validate(region: &FocusRegion) -> Result<(), FocusError> {
    let d = &region.display;
    if !(0.5..=4.0).contains(&d.scale) {
        return Err(FocusError::BadScale(d.scale));
    }
    if region.selection.is_empty() {
        return Err(FocusError::EmptySelection);
    }
    if !within(&region.selection, &d.frame) {
        return Err(FocusError::OutsideDisplay("selection"));
    }
    if !within(&region.crop, &d.frame) {
        return Err(FocusError::OutsideDisplay("crop"));
    }
    if !within(&region.selection, &region.crop) {
        return Err(FocusError::CropMissesSelection);
    }
    if matches!(region.kind, FocusKind::Lasso | FocusKind::Point) && region.path.is_empty() {
        return Err(FocusError::MissingPath(region.kind));
    }
    for p in &region.path {
        if !within(&Rect::new(p.x, p.y, 0.0, 0.0), &d.frame) {
            return Err(FocusError::OutsideDisplay("path"));
        }
    }
    if let Some(img) = &region.image {
        let ok_url = img
            .url
            .strip_prefix("wisp://focus/")
            .and_then(|f| f.strip_suffix(".png"))
            .is_some_and(wisp_common::ids::is_valid_id);
        if !ok_url {
            return Err(FocusError::BadImageUrl);
        }
        let (ew, eh) = pixel_size(&region.crop, d.scale);
        let close = |a: u32, b: u32| a.abs_diff(b) <= 2;
        if !close(img.width_px, ew) || !close(img.height_px, eh) {
            return Err(FocusError::ImageSizeMismatch {
                got: (img.width_px, img.height_px),
                expected: (ew, eh),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_common::geometry::Point;
    use wisp_ipc::types::focus::{DisplayInfo, FocusImage};

    fn region() -> FocusRegion {
        let id = wisp_common::ids::new_id();
        FocusRegion {
            id: id.clone(),
            kind: FocusKind::Rectangle,
            display: DisplayInfo {
                id: 2,
                name: "Studio Display".into(),
                frame: Rect::new(1512.0, -200.0, 2560.0, 1440.0),
                scale: 2.0,
            },
            selection: Rect::new(1600.0, 0.0, 300.0, 200.0),
            crop: Rect::new(1576.0, -24.0, 348.0, 248.0),
            path: vec![],
            cursor: Point {
                x: 1900.0,
                y: 200.0,
            },
            image: Some(FocusImage {
                url: format!("wisp://focus/{id}.png"),
                width_px: 696,
                height_px: 496,
            }),
            application: None,
            window: None,
            accessibility: None,
            text: None,
            created_at_ms: 0,
        }
    }

    #[test]
    fn accepts_retina_region_on_secondary_display() {
        assert_eq!(validate(&region()), Ok(()));
    }

    #[test]
    fn rejects_inconsistent_geometry() {
        let mut r = region();
        r.selection.x = 4000.0;
        assert_eq!(validate(&r), Err(FocusError::OutsideDisplay("selection")));

        let mut r = region();
        r.image.as_mut().expect("img").width_px = 348; // 1x pixels on a 2x display
        assert!(matches!(
            validate(&r),
            Err(FocusError::ImageSizeMismatch { .. })
        ));

        let mut r = region();
        r.crop = Rect::new(1700.0, 0.0, 10.0, 10.0);
        assert_eq!(validate(&r), Err(FocusError::CropMissesSelection));

        let mut r = region();
        r.image.as_mut().expect("img").url = "file:///etc/passwd".into();
        assert_eq!(validate(&r), Err(FocusError::BadImageUrl));

        let mut r = region();
        r.kind = FocusKind::Lasso;
        assert_eq!(validate(&r), Err(FocusError::MissingPath(FocusKind::Lasso)));

        let mut r = region();
        r.display.scale = 0.0;
        assert!(matches!(validate(&r), Err(FocusError::BadScale(_))));
    }
}
