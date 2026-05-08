use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::icons::IconId;
use crate::ui::theme::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawKind {
    /// Solid fill with rounded-rect SDF anti-aliasing in the fragment shader.
    Solid,
    /// Sample the glyph atlas at `uv_min..uv_max`; the sampled alpha modulates
    /// `color.a`. `radius` is ignored.
    Glyph,
}

#[derive(Debug, Clone, Copy)]
pub struct DrawCommand {
    pub kind: DrawKind,
    pub rect: Rect,
    pub color: Color,
    pub radius: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

#[derive(Default)]
pub struct DrawList {
    pub commands: Vec<DrawCommand>,
}

impl DrawList {
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    #[allow(dead_code)]
    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(DrawCommand {
            kind: DrawKind::Solid,
            rect,
            color,
            radius: 0.0,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
        });
    }

    pub fn rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        self.commands.push(DrawCommand {
            kind: DrawKind::Solid,
            rect,
            color,
            radius,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
        });
    }

    pub fn progress_bar(&mut self, rect: Rect, progress: f32, theme: &Theme) {
        let progress = progress.clamp(0.0, 1.0);
        let radius = (rect.height * 0.5).min(theme.radius_small);
        self.rounded_rect(rect, radius, theme.track);
        if progress > 0.0 {
            let fill = Rect::new(rect.x, rect.y, rect.width * progress, rect.height);
            self.rounded_rect(fill, radius, theme.accent);
        }
    }

    /// Lay out `text` left-to-right at `(x, top_y)` using the given size, in
    /// pixels, against `font`'s pre-rasterized atlas. Returns the advanced pen
    /// width so callers can chain runs or measure trailing space.
    pub fn text(
        &mut self,
        x: f32,
        top_y: f32,
        size: u8,
        color: Color,
        text: &str,
        font: &Font,
    ) -> f32 {
        // Snap baseline to integer pixels so glyphs stay sharp under LINEAR
        // sampling — fontdue rasterizes onto an integer grid.
        let baseline = (top_y + font.ascent(size)).round();
        let atlas_w = font.atlas.width as f32;
        let atlas_h = font.atlas.height as f32;
        let mut pen = x;
        for ch in text.chars() {
            let Some(g) = font.glyph(ch, size) else {
                continue;
            };
            if g.atlas_w > 0 && g.atlas_h > 0 {
                // Snap each glyph quad's origin too. `pen` keeps fractional
                // running width so kerning isn't lost.
                let qx = (pen + g.bearing_x).round();
                let qy = (baseline + g.bearing_y).round();
                let qw = g.atlas_w as f32;
                let qh = g.atlas_h as f32;
                let u0 = g.atlas_x as f32 / atlas_w;
                let v0 = g.atlas_y as f32 / atlas_h;
                let u1 = (g.atlas_x as f32 + qw) / atlas_w;
                let v1 = (g.atlas_y as f32 + qh) / atlas_h;
                self.commands.push(DrawCommand {
                    kind: DrawKind::Glyph,
                    rect: Rect::new(qx, qy, qw, qh),
                    color,
                    radius: 0.0,
                    uv_min: [u0, v0],
                    uv_max: [u1, v1],
                });
            }
            pen += g.advance;
        }
        pen - x
    }

    /// Draw `icon` centred inside `rect` tinted by `color`. The atlas entry
    /// is sampled for its alpha; the colour comes from `color`.
    pub fn icon(&mut self, rect: Rect, color: Color, font: &Font, icon: IconId) {
        let pick_size = rect.height.round().max(1.0) as u8;
        let Some(g) = font.icon(icon, pick_size) else {
            return;
        };
        let atlas_w = font.atlas.width as f32;
        let atlas_h = font.atlas.height as f32;
        let qw = g.atlas_w as f32;
        let qh = g.atlas_h as f32;
        let qx = (rect.x + (rect.width - qw) * 0.5).round();
        let qy = (rect.y + (rect.height - qh) * 0.5).round();
        let u0 = g.atlas_x as f32 / atlas_w;
        let v0 = g.atlas_y as f32 / atlas_h;
        let u1 = (g.atlas_x as f32 + qw) / atlas_w;
        let v1 = (g.atlas_y as f32 + qh) / atlas_h;
        self.commands.push(DrawCommand {
            kind: DrawKind::Glyph,
            rect: Rect::new(qx, qy, qw, qh),
            color,
            radius: 0.0,
            uv_min: [u0, v0],
            uv_max: [u1, v1],
        });
    }
}
