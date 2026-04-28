use femtovg::{Canvas, Color, Paint, Path, Renderer};

use crate::ui::{
    geometry::Rect,
    text::{draw_text as draw_text_helper, Fonts},
    theme::Theme,
};

pub fn draw_rect<T: Renderer>(canvas: &mut Canvas<T>, rect: Rect, color: Color) {
    let mut path = Path::new();
    path.rect(rect.x, rect.y, rect.width, rect.height);
    canvas.fill_path(&path, &Paint::color(color));
}

pub fn draw_rounded_rect<T: Renderer>(
    canvas: &mut Canvas<T>,
    rect: Rect,
    radius: f32,
    color: Color,
) {
    let mut path = Path::new();
    path.rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);
    canvas.fill_path(&path, &Paint::color(color));
}

pub fn draw_text<T: Renderer>(
    canvas: &mut Canvas<T>,
    fonts: &Fonts,
    x: f32,
    y: f32,
    size: f32,
    color: Color,
    text: &str,
) -> Result<(), String> {
    draw_text_helper(canvas, fonts, x, y, size, color, text)
}

#[allow(dead_code)]
pub fn draw_progress_bar<T: Renderer>(
    canvas: &mut Canvas<T>,
    rect: Rect,
    progress: f32,
    theme: &Theme,
) {
    let progress = progress.clamp(0.0, 1.0);
    let radius = (rect.height * 0.5).min(theme.radius_small);

    draw_rounded_rect(canvas, rect, radius, theme.border);

    let fill_rect = Rect::new(rect.x, rect.y, rect.width * progress, rect.height);
    draw_rounded_rect(canvas, fill_rect, radius, theme.accent);
}
