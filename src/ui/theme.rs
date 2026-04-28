use femtovg::Color;

pub struct Theme {
    pub background: Color,
    pub panel: Color,
    pub text: Color,
    pub muted_text: Color,
    pub accent: Color,
    pub border: Color,
    #[allow(dead_code)]
    pub radius_small: f32,
    pub radius_medium: f32,
    pub radius_large: f32,
    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::rgb(0xED, 0xE8, 0xE1),
            panel: Color::rgb(0xF5, 0xF0, 0xEA),
            text: Color::rgb(0x1C, 0x19, 0x17),
            muted_text: Color::rgb(0xA8, 0xA2, 0x9E),
            accent: Color::rgb(0xC2, 0x69, 0x4A),
            border: Color::rgb(0xE0, 0xD9, 0xD0),
            radius_small: 8.0,
            radius_medium: 12.0,
            radius_large: 16.0,
            spacing_small: 8.0,
            spacing_medium: 16.0,
            spacing_large: 24.0,
        }
    }
}
