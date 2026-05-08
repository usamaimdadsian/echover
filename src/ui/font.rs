use std::collections::HashMap;

use crate::ui::icons::{rasterize as rasterize_icon, IconId, ALL_ICONS, ICON_SIZES};

// Font sizes the theme uses. Pre-rasterized at startup so the atlas is fixed
// after init — no dynamic growth, no per-frame uploads. Keep this list in sync
// with `Theme::text_*` constants below.
pub const TEXT_SIZES: &[u8] = &[11, 12, 13, 14, 16, 18, 22, 28];

// Non-ASCII glyphs the design uses (middle dot for separators, transport
// symbols, ellipsis, heart for bookmarks). Rasterized at every TEXT_SIZE.
const EXTRA_GLYPHS: &[char] = &['·', '–', '—', '…', '▶', '❚', '♥', '⏵', '⏸', '←', '→'];

const ATLAS_SIZE: u32 = 1024;
const ATLAS_GUTTER: u32 = 1;

#[derive(Clone, Copy)]
pub struct GlyphInfo {
    pub atlas_x: u16,
    pub atlas_y: u16,
    pub atlas_w: u16,
    pub atlas_h: u16,
    // Pen-relative offset to the quad's top-left in screen pixels.
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
}

#[derive(Clone, Copy)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}

pub struct Atlas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub struct Font {
    pub atlas: Atlas,
    glyphs: HashMap<(char, u8), GlyphInfo>,
    line_metrics: HashMap<u8, LineMetrics>,
    icons: HashMap<(IconId, u8), GlyphInfo>,
}

impl Font {
    pub fn load_default() -> Result<Self, String> {
        let bytes = read_font_bytes()?;
        let face = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|err| format!("failed to parse font: {err}"))?;
        Self::from_face(face)
    }

    fn from_face(face: fontdue::Font) -> Result<Self, String> {
        let mut atlas = Atlas {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            pixels: vec![0u8; (ATLAS_SIZE * ATLAS_SIZE) as usize],
        };
        let mut glyphs: HashMap<(char, u8), GlyphInfo> = HashMap::new();
        let mut line_metrics: HashMap<u8, LineMetrics> = HashMap::new();

        let mut pen_x: u32 = ATLAS_GUTTER;
        let mut row_y: u32 = ATLAS_GUTTER;
        let mut row_max_h: u32 = 0;

        for &size in TEXT_SIZES {
            if let Some(lm) = face.horizontal_line_metrics(size as f32) {
                line_metrics.insert(
                    size,
                    LineMetrics {
                        ascent: lm.ascent,
                        descent: lm.descent,
                        line_height: lm.new_line_size,
                    },
                );
            }
            let ascii_chars = (0x20u8..=0x7E).map(|c| c as char);
            for ch in ascii_chars.chain(EXTRA_GLYPHS.iter().copied()) {
                let (metrics, bitmap) = face.rasterize(ch, size as f32);
                let w = metrics.width as u32;
                let h = metrics.height as u32;
                if w == 0 || h == 0 {
                    glyphs.insert(
                        (ch, size),
                        GlyphInfo {
                            atlas_x: 0,
                            atlas_y: 0,
                            atlas_w: 0,
                            atlas_h: 0,
                            bearing_x: 0.0,
                            bearing_y: 0.0,
                            advance: metrics.advance_width,
                        },
                    );
                    continue;
                }

                if pen_x + w + ATLAS_GUTTER >= atlas.width {
                    pen_x = ATLAS_GUTTER;
                    row_y += row_max_h + ATLAS_GUTTER;
                    row_max_h = 0;
                }
                if row_y + h + ATLAS_GUTTER >= atlas.height {
                    return Err(format!("glyph atlas overflow at {ch:?}@{size}px"));
                }

                for y in 0..h {
                    let src = (y * w) as usize;
                    let dst = ((row_y + y) * atlas.width + pen_x) as usize;
                    atlas.pixels[dst..dst + w as usize]
                        .copy_from_slice(&bitmap[src..src + w as usize]);
                }

                glyphs.insert(
                    (ch, size),
                    GlyphInfo {
                        atlas_x: pen_x as u16,
                        atlas_y: row_y as u16,
                        atlas_w: w as u16,
                        atlas_h: h as u16,
                        bearing_x: metrics.xmin as f32,
                        // fontdue's `ymin` is the offset from baseline to the
                        // bitmap's bottom (positive = below baseline). We want
                        // the offset from baseline to the bitmap's top in
                        // screen-down coords, which is `-ymin - height`.
                        bearing_y: -(metrics.ymin as f32) - h as f32,
                        advance: metrics.advance_width,
                    },
                );
                pen_x += w + ATLAS_GUTTER;
                if h > row_max_h {
                    row_max_h = h;
                }
            }
        }

        // Bake icons into the same atlas. Treats each (icon, size) like a
        // glyph entry — same packer, same R8 bitmap path.
        let mut icons: HashMap<(IconId, u8), GlyphInfo> = HashMap::new();
        for &size in ICON_SIZES {
            for &id in ALL_ICONS {
                let bitmap = rasterize_icon(id, size);
                let w = size as u32;
                let h = size as u32;
                if pen_x + w + ATLAS_GUTTER >= atlas.width {
                    pen_x = ATLAS_GUTTER;
                    row_y += row_max_h + ATLAS_GUTTER;
                    row_max_h = 0;
                }
                if row_y + h + ATLAS_GUTTER >= atlas.height {
                    return Err(format!("icon atlas overflow at {id:?}@{size}px"));
                }
                for y in 0..h {
                    let src = (y * w) as usize;
                    let dst = ((row_y + y) * atlas.width + pen_x) as usize;
                    atlas.pixels[dst..dst + w as usize]
                        .copy_from_slice(&bitmap[src..src + w as usize]);
                }
                icons.insert(
                    (id, size),
                    GlyphInfo {
                        atlas_x: pen_x as u16,
                        atlas_y: row_y as u16,
                        atlas_w: w as u16,
                        atlas_h: h as u16,
                        bearing_x: 0.0,
                        bearing_y: 0.0,
                        advance: w as f32,
                    },
                );
                pen_x += w + ATLAS_GUTTER;
                if h > row_max_h {
                    row_max_h = h;
                }
            }
        }

        tracing::info!(
            glyphs = glyphs.len(),
            icons = icons.len(),
            atlas_used_rows = row_y + row_max_h,
            "font atlas ready"
        );

        Ok(Self {
            atlas,
            glyphs,
            line_metrics,
            icons,
        })
    }

    /// Empty font for unit tests: a 1×1 atlas with no glyphs. `draw.text(...)`
    /// renders nothing but still walks the layout pipeline correctly.
    #[cfg(test)]
    pub fn empty_for_tests() -> Self {
        Self {
            atlas: Atlas {
                width: 1,
                height: 1,
                pixels: vec![0u8; 1],
            },
            glyphs: HashMap::new(),
            line_metrics: HashMap::new(),
            icons: HashMap::new(),
        }
    }

    pub fn glyph(&self, ch: char, size: u8) -> Option<&GlyphInfo> {
        self.glyphs.get(&(ch, size))
    }

    /// Look up an icon entry. Falls back to the largest baked size that's
    /// `<= size`, or the smallest baked size if `size` is below them all.
    pub fn icon(&self, id: IconId, size: u8) -> Option<&GlyphInfo> {
        if let Some(info) = self.icons.get(&(id, size)) {
            return Some(info);
        }
        let mut best: Option<&GlyphInfo> = None;
        let mut best_diff = i32::MAX;
        for &candidate in ICON_SIZES {
            if let Some(info) = self.icons.get(&(id, candidate)) {
                let diff = (candidate as i32 - size as i32).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best = Some(info);
                }
            }
        }
        best
    }

    pub fn measure(&self, text: &str, size: u8) -> f32 {
        let mut total = 0.0;
        for ch in text.chars() {
            if let Some(g) = self.glyph(ch, size) {
                total += g.advance;
            }
        }
        total
    }

    pub fn ascent(&self, size: u8) -> f32 {
        self.line_metrics
            .get(&size)
            .map(|m| m.ascent)
            .unwrap_or_else(|| size as f32 * 0.8)
    }

    pub fn line_height(&self, size: u8) -> f32 {
        self.line_metrics
            .get(&size)
            .map(|m| m.line_height)
            .unwrap_or_else(|| size as f32 * 1.25)
    }
}

fn read_font_bytes() -> Result<Vec<u8>, String> {
    // Project-local font wins. Drop a .ttf at assets/fonts/ui.ttf to override
    // the system fallbacks below.
    let candidates = [
        "assets/fonts/ui.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            tracing::info!(font = path, "loaded UI font");
            return Ok(bytes);
        }
    }
    Err("no UI font found; drop a .ttf at assets/fonts/ui.ttf".to_owned())
}
