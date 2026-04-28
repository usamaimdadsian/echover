use crate::ui::{action::UiAction, geometry::Rect};

#[derive(Debug, Clone, Copy)]
pub struct HitRegion {
    pub rect: Rect,
    pub action: UiAction,
}

pub fn hit_test(regions: &[HitRegion], point: (f32, f32)) -> Option<UiAction> {
    regions
        .iter()
        .rev()
        .find(|region| region.rect.contains(point))
        .map(|region| region.action)
}
