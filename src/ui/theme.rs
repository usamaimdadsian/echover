#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    #[allow(dead_code)]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn with_alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

// Tokens extracted from `design/design.html` (cream/terracotta palette).
pub struct Theme {
    pub background: Color,
    pub panel: Color,
    pub text: Color,
    pub muted_text: Color,
    pub accent: Color,
    pub border: Color,
    pub track: Color,
    pub radius_small: f32,
    pub radius_medium: f32,
    pub radius_large: f32,
    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
    // Text scales (px). Must all appear in `ui::font::TEXT_SIZES` so the atlas
    // is pre-rasterized at startup.
    pub text_eyebrow: u8,
    pub text_caption: u8,
    pub text_subtitle: u8,
    pub text_body: u8,
    pub text_button: u8,
    pub text_section: u8,
    pub text_title: u8,
    pub text_display: u8,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::rgb_u8(0xED, 0xE8, 0xE1),
            panel: Color::rgb_u8(0xFE, 0xFC, 0xF8),
            text: Color::rgb_u8(0x1C, 0x19, 0x17),
            muted_text: Color::rgb_u8(0xA8, 0xA2, 0x9E),
            accent: Color::rgb_u8(0xC2, 0x69, 0x4A),
            border: Color::rgb_u8(0xE0, 0xD9, 0xD0),
            track: Color::rgb_u8(0xEA, 0xE5, 0xDE),
            radius_small: 8.0,
            radius_medium: 10.0,
            radius_large: 16.0,
            spacing_small: 8.0,
            spacing_medium: 16.0,
            spacing_large: 24.0,
            text_eyebrow: 11,
            text_caption: 12,
            text_subtitle: 13,
            text_body: 14,
            text_button: 14,
            text_section: 16,
            text_title: 22,
            text_display: 28,
        }
    }
}
