use std::path::Path;

use femtovg::{Align, Baseline, Canvas, Color, FontId, Paint, Renderer};

pub struct Fonts {
    pub ui: FontId,
    pub heading: FontId,
}

pub fn load_fonts<T: Renderer>(canvas: &mut Canvas<T>) -> Result<Fonts, String> {
    let ui_font = load_first_font(
        canvas,
        &[
            "assets/fonts/DMSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ],
    )
    .or_else(|| {
        canvas
            .add_font_dir("/usr/share/fonts")
            .ok()
            .and_then(|ids| ids.into_iter().next())
    })
    .ok_or_else(|| "failed to load a UI font".to_owned())?;

    let heading_font = load_first_font(
        canvas,
        &[
            "assets/fonts/DMSerifDisplay-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
            "/usr/share/fonts/TTF/DejaVuSerif.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
        ],
    )
    .unwrap_or(ui_font);

    Ok(Fonts {
        ui: ui_font,
        heading: heading_font,
    })
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
    let mut paint = Paint::color(color);
    paint.set_font(&[fonts.ui]);
    paint.set_font_size(size);
    paint.set_text_align(Align::Left);
    paint.set_text_baseline(Baseline::Top);

    canvas
        .fill_text(x, y, text, &paint)
        .map(|_| ())
        .map_err(|error| format!("failed to draw text: {error:?}"))
}

pub fn draw_heading_text<T: Renderer>(
    canvas: &mut Canvas<T>,
    fonts: &Fonts,
    x: f32,
    y: f32,
    size: f32,
    color: Color,
    text: &str,
) -> Result<(), String> {
    let mut paint = Paint::color(color);
    paint.set_font(&[fonts.heading]);
    paint.set_font_size(size);
    paint.set_text_align(Align::Left);
    paint.set_text_baseline(Baseline::Top);

    canvas
        .fill_text(x, y, text, &paint)
        .map(|_| ())
        .map_err(|error| format!("failed to draw heading text: {error:?}"))
}

fn load_first_font<T: Renderer>(canvas: &mut Canvas<T>, candidates: &[&str]) -> Option<FontId> {
    for path in candidates {
        if Path::new(path).exists() {
            if let Ok(id) = canvas.add_font(path) {
                return Some(id);
            }
        }
    }

    None
}
