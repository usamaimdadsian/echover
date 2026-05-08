use crate::ui::{
    action::UiAction,
    geometry::Rect,
    theme::{Color, Theme},
};

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Idle,
    Hover,
    Pressed,
}

/// Hover + pressed action snapshot, threaded through layout so widgets can
/// shade their backgrounds without each one reaching into `WindowState`.
#[derive(Default, Clone, Copy)]
pub struct Interaction {
    pub hover: Option<UiAction>,
    pub pressed: Option<UiAction>,
}

impl Interaction {
    pub fn state_for(&self, action: UiAction) -> ButtonState {
        if self.pressed == Some(action) {
            ButtonState::Pressed
        } else if self.hover == Some(action) {
            ButtonState::Hover
        } else {
            ButtonState::Idle
        }
    }

    pub fn shade(&self, action: UiAction, base: Color, theme: &Theme) -> Color {
        match self.state_for(action) {
            ButtonState::Idle => base,
            ButtonState::Hover => mix(base, theme.text, 0.08),
            ButtonState::Pressed => mix(base, theme.text, 0.18),
        }
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color {
        r: a.r * inv + b.r * t,
        g: a.g * inv + b.g * t,
        b: a.b * inv + b.b * t,
        a: a.a,
    }
}
