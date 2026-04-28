use std::ffi::CString;

use femtovg::{renderer::OpenGl, Align, Baseline, Canvas, Color, Paint};
use glutin::display::{GetGlDisplay, GlDisplay};

use crate::ui::{
    action::UiAction,
    geometry::Rect,
    hit::{hit_test, HitRegion},
    primitives::{draw_rect, draw_rounded_rect, draw_text},
    state::{AppPage, AppState},
    text::{load_fonts, Fonts},
    theme::Theme,
};

pub struct Renderer {
    canvas: Canvas<OpenGl>,
    theme: Theme,
    fonts: Fonts,
    hit_regions: Vec<HitRegion>,
}

impl Renderer {
    pub fn new<D>(
        gl_display: &D,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Result<Self, String>
    where
        D: GetGlDisplay,
    {
        let display = gl_display.display();
        let renderer = unsafe {
            OpenGl::new_from_function(|symbol| {
                let symbol = CString::new(symbol).expect("OpenGL function name contains null byte");
                display.get_proc_address(symbol.as_c_str()) as *const _
            })
        }
        .map_err(|error| format!("failed to initialize OpenGL renderer: {error:?}"))?;

        let mut canvas = Canvas::new(renderer)
            .map_err(|error| format!("failed to create femtovg canvas: {error:?}"))?;
        canvas.set_size(width, height, scale_factor);

        let fonts = load_fonts(&mut canvas)?;

        Ok(Self {
            canvas,
            theme: Theme::default(),
            fonts,
            hit_regions: Vec::new(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        self.canvas.set_size(width, height, scale_factor);
    }

    pub fn handle_click(&self, mouse_position: (f32, f32)) -> Option<UiAction> {
        hit_test(&self.hit_regions, mouse_position)
    }

    pub fn action_at(&self, mouse_position: (f32, f32)) -> Option<UiAction> {
        hit_test(&self.hit_regions, mouse_position)
    }

    pub fn render(&mut self, state: &AppState, width: u32, height: u32) -> Result<(), String> {
        self.canvas
            .clear_rect(0, 0, width, height, self.theme.background);
        self.hit_regions.clear();

        let root = Rect::new(0.0, 0.0, width as f32, height as f32);
        let shell = root.inset(self.theme.spacing_large);
        draw_rounded_rect(
            &mut self.canvas,
            shell,
            self.theme.radius_large,
            self.theme.panel,
        );

        let sidebar_width = 240.0;
        let (sidebar_rect, main_rect) = shell.split_horizontal(sidebar_width);

        let sidebar_panel = sidebar_rect.inset(self.theme.spacing_small);
        let main_panel = main_rect.inset(self.theme.spacing_small);

        draw_rounded_rect(
            &mut self.canvas,
            sidebar_panel,
            self.theme.radius_large,
            Color::white(),
        );
        draw_rounded_rect(
            &mut self.canvas,
            main_panel,
            self.theme.radius_large,
            Color::white(),
        );

        draw_text(
            &mut self.canvas,
            &self.fonts,
            sidebar_panel.x + self.theme.spacing_medium,
            sidebar_panel.y + self.theme.spacing_medium,
            13.0,
            self.theme.muted_text,
            "Echover",
        )?;

        let nav_x = sidebar_panel.x + self.theme.spacing_small;
        let nav_y = sidebar_panel.y + 76.0;
        let nav_width = sidebar_panel.width - self.theme.spacing_small * 2.0;
        let nav_height = 44.0;
        let nav_gap = 10.0;

        let nav_items = [
            ("Home", AppPage::Home, UiAction::NavigateHome),
            ("Library", AppPage::Library, UiAction::NavigateLibrary),
            ("Player", AppPage::Player, UiAction::NavigatePlayer),
            ("Settings", AppPage::Settings, UiAction::NavigateSettings),
        ];

        for (index, (label, page, action)) in nav_items.into_iter().enumerate() {
            let rect = Rect::new(
                nav_x,
                nav_y + index as f32 * (nav_height + nav_gap),
                nav_width,
                nav_height,
            );
            let is_active = state.current_page == page;
            let is_hovered = state.interaction.hovered_action == Some(action);

            let bg = if is_active {
                self.theme.accent
            } else if is_hovered {
                Color::rgba(0xC2, 0x69, 0x4A, 46)
            } else {
                Color::rgba(0, 0, 0, 0)
            };
            let fg = if is_active {
                Color::white()
            } else if is_hovered {
                self.theme.accent
            } else {
                self.theme.text
            };

            draw_rounded_rect(&mut self.canvas, rect, self.theme.radius_medium, bg);
            draw_text(
                &mut self.canvas,
                &self.fonts,
                rect.x + 16.0,
                rect.y + 13.0,
                16.0,
                fg,
                label,
            )?;

            self.hit_regions.push(HitRegion { rect, action });
        }

        draw_rect(
            &mut self.canvas,
            Rect::new(main_panel.x, main_panel.y, main_panel.width, 1.0),
            self.theme.border,
        );

        let page_label = match state.current_page {
            AppPage::Home => "Home Page",
            AppPage::Library => "Library Page",
            AppPage::Player => "Player Page",
            AppPage::Settings => "Settings Page",
        };

        let mut heading_paint = Paint::color(self.theme.text);
        heading_paint.set_font(&[self.fonts.heading]);
        heading_paint.set_font_size(42.0);
        heading_paint.set_text_align(Align::Left);
        heading_paint.set_text_baseline(Baseline::Top);
        self.canvas
            .fill_text(
                main_panel.x + self.theme.spacing_large,
                main_panel.y + self.theme.spacing_large,
                page_label,
                &heading_paint,
            )
            .map_err(|error| format!("failed to draw main heading: {error:?}"))?;

        draw_text(
            &mut self.canvas,
            &self.fonts,
            main_panel.x + self.theme.spacing_large,
            main_panel.y + self.theme.spacing_large + 58.0,
            16.0,
            self.theme.muted_text,
            "Phase 3 shell placeholder content",
        )?;

        self.canvas.flush();
        Ok(())
    }
}
