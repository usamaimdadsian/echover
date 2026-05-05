use std::ffi::CString;

use femtovg::{renderer::OpenGl, Canvas, Color};
use glutin::display::{GetGlDisplay, GlDisplay};

use crate::domain::audiobook::Audiobook;
use crate::ui::{
    action::{LibraryFilter, UiAction},
    geometry::Rect,
    hit::{hit_test, HitRegion},
    primitives::{draw_progress_bar, draw_rect, draw_rounded_rect, draw_text},
    state::{AppPage, AppState, PlaybackStatus},
    text::{draw_heading_text, load_fonts, Fonts},
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

    pub fn render(&mut self, state: &mut AppState, width: u32, height: u32) -> Result<(), String> {
        self.canvas
            .clear_rect(0, 0, width, height, self.theme.background);
        self.hit_regions.clear();

        let root = Rect::new(0.0, 0.0, width as f32, height as f32);
        let shell_width = root.width.min(1400.0);
        let shell = Rect::new((root.width - shell_width) * 0.5, 0.0, shell_width, root.height);
        draw_rect(&mut self.canvas, shell, self.theme.panel);

        let sidebar_width = if shell.width < 560.0 {
            (shell.width * 0.34).clamp(132.0, 180.0)
        } else {
            (shell.width * 0.20).clamp(200.0, 260.0)
        };
        let (sidebar_rect, main_rect) = shell.split_horizontal(sidebar_width);
        let sidebar_panel = sidebar_rect;
        let main_panel = main_rect;

        draw_rect(
            &mut self.canvas,
            sidebar_panel,
            Color::rgb(0xFE, 0xFC, 0xF8),
        );
        draw_rect(&mut self.canvas, main_panel, Color::white());
        draw_rect(
            &mut self.canvas,
            Rect::new(sidebar_panel.x + sidebar_panel.width - 1.0, sidebar_panel.y, 1.0, sidebar_panel.height),
            Color::rgb(0xED, 0xE8, 0xE1),
        );

        self.draw_sidebar(state, sidebar_panel)?;

        match state.current_page {
            AppPage::Home => self.draw_home_page(state, main_panel)?,
            AppPage::Library => self.draw_library_page(state, main_panel)?,
            AppPage::Player => self.draw_player_page(state, main_panel)?,
            AppPage::Settings => self.draw_settings_page(main_panel)?,
        }

        self.canvas.flush();
        Ok(())
    }

    fn draw_sidebar(&mut self, state: &AppState, sidebar_panel: Rect) -> Result<(), String> {
        let pad_x = if sidebar_panel.width < 170.0 { 12.0 } else { 24.0 };
        let nav_x = sidebar_panel.x + 12.0;
        let nav_width = (sidebar_panel.width - 24.0).max(0.0);
        let label_x = sidebar_panel.x + pad_x + 42.0;

        let brand_y = sidebar_panel.y + 28.0;
        let brand_rect = Rect::new(sidebar_panel.x + pad_x, brand_y, 32.0, 32.0);
        draw_rounded_rect(
            &mut self.canvas,
            brand_rect,
            10.0,
            self.theme.accent,
        );
        if sidebar_panel.width >= 190.0 {
            draw_heading_text(
                &mut self.canvas,
                &self.fonts,
                label_x,
                brand_y + 7.0,
                18.0,
                self.theme.text,
                "echover",
            )?;
        }

        draw_text(
            &mut self.canvas,
            &self.fonts,
            sidebar_panel.x + pad_x,
            brand_y + 44.0,
            11.0,
            self.theme.muted_text,
            "NAVIGATION",
        )?;

        let nav_y = brand_y + 64.0;
        let nav_height = 40.0;
        let nav_gap = 4.0;

        let nav_items = [
            ("Home", AppPage::Home, UiAction::NavigateHome),
            ("Library", AppPage::Library, UiAction::NavigateLibrary),
            ("Now Playing", AppPage::Player, UiAction::NavigatePlayer),
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
                Color::rgba(0xC2, 0x69, 0x4A, 24)
            } else if is_hovered {
                Color::rgb(0xF5, 0xF0, 0xEA)
            } else {
                Color::rgba(0, 0, 0, 0)
            };
            let fg = if is_active {
                self.theme.accent
            } else if is_hovered {
                self.theme.text
            } else {
                Color::rgb(0x78, 0x71, 0x6C)
            };

            draw_rounded_rect(&mut self.canvas, rect, 10.0, bg);
            draw_text(
                &mut self.canvas,
                &self.fonts,
                rect.x + 12.0,
                rect.y + 11.0,
                14.0,
                fg,
                label,
            )?;

            self.hit_regions.push(HitRegion { rect, action });
        }

        Ok(())
    }

    fn draw_home_page(&mut self, state: &mut AppState, main_panel: Rect) -> Result<(), String> {
        let page_pad = if main_panel.width < 760.0 { 16.0 } else { 32.0 };
        let mut y = main_panel.y + page_pad;
        let x = main_panel.x + page_pad;
        let content_width = (main_panel.width - page_pad * 2.0).max(160.0);
        let viewport_height = (main_panel.height - page_pad * 2.0).max(0.0);

        let recent_card_height = 148.0;
        let preview_card_height = 132.0;
        let card_gap = self.theme.spacing_medium;

        let recent_cols = ((content_width + card_gap) / (170.0 + card_gap))
            .floor()
            .clamp(1.0, 3.0) as usize;
        let recent_count = state.books.iter().take(3).count();
        let recent_rows = recent_count.div_ceil(recent_cols.max(1));
        let recent_grid_height = if recent_count == 0 {
            0.0
        } else {
            recent_rows as f32 * recent_card_height + (recent_rows as f32 - 1.0) * card_gap
        };

        let preview_cols = ((content_width + card_gap) / (150.0 + card_gap))
            .floor()
            .clamp(1.0, 4.0) as usize;
        let preview_count = state.books.iter().take(6).count();
        let preview_rows = preview_count.div_ceil(preview_cols.max(1));
        let preview_grid_height = if preview_count == 0 {
            0.0
        } else {
            preview_rows as f32 * preview_card_height + (preview_rows as f32 - 1.0) * card_gap
        };

        let total_content_height =
            44.0 + 24.0 + 178.0 + 26.0 + 24.0 + recent_grid_height + 26.0 + 24.0 + preview_grid_height;
        state.set_home_scroll_max((total_content_height - viewport_height).max(0.0));
        let scroll = state.home_scroll;
        y -= scroll;

        draw_heading_text(
            &mut self.canvas,
            &self.fonts,
            x,
            y,
            26.0,
            self.theme.text,
            "Home",
        )?;
        y += 44.0;

        let continue_book = state
            .continue_book()
            .or_else(|| state.selected_book())
            .ok_or_else(|| "no mock books available".to_owned())?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            x,
            y,
            14.0,
            self.theme.muted_text,
            "Continue Listening",
        )?;
        y += 24.0;

        let continue_card = Rect::new(x, y, content_width, 178.0);
        let continue_button = self.draw_continue_card(
            state,
            continue_card,
            continue_book,
            UiAction::ContinueListening,
        )?;
        self.hit_regions.push(HitRegion {
            rect: continue_button,
            action: UiAction::ContinueListening,
        });
        y += continue_card.height + 26.0;

        draw_text(
            &mut self.canvas,
            &self.fonts,
            x,
            y,
            14.0,
            self.theme.muted_text,
            "Recently Played",
        )?;
        y += 24.0;

        let recent_cards = state.books.iter().take(3).collect::<Vec<_>>();
        let recent_width = (content_width - card_gap * (recent_cols as f32 - 1.0)) / recent_cols as f32;
        for (idx, book) in recent_cards.into_iter().enumerate() {
            let row = idx / recent_cols;
            let col = idx % recent_cols;
            let rect = Rect::new(
                x + col as f32 * (recent_width + card_gap),
                y + row as f32 * (recent_card_height + card_gap),
                recent_width,
                recent_card_height,
            );
            self.draw_book_card(state, rect, book, 0.75)?;
            self.hit_regions.push(HitRegion {
                rect,
                action: UiAction::SelectBook(book.id),
            });
        }
        y += recent_grid_height + 26.0;

        draw_text(
            &mut self.canvas,
            &self.fonts,
            x,
            y,
            14.0,
            self.theme.muted_text,
            "Library Preview",
        )?;
        y += 24.0;

        let preview_gap = self.theme.spacing_small;
        let preview_width =
            (content_width - preview_gap * (preview_cols as f32 - 1.0)) / preview_cols as f32;
        for (idx, book) in state.books.iter().take(6).enumerate() {
            let row = idx / preview_cols;
            let col = idx % preview_cols;
            let rect = Rect::new(
                x + col as f32 * (preview_width + preview_gap),
                y + row as f32 * (preview_card_height + preview_gap),
                preview_width,
                preview_card_height,
            );
            self.draw_book_card(state, rect, book, 0.65)?;
            self.hit_regions.push(HitRegion {
                rect,
                action: UiAction::SelectBook(book.id),
            });
        }

        Ok(())
    }

    fn draw_library_page(&mut self, state: &mut AppState, main_panel: Rect) -> Result<(), String> {
        let page_pad = if main_panel.width < 760.0 { 16.0 } else { 32.0 };
        let mut y = main_panel.y + page_pad;
        let x = main_panel.x + page_pad;
        let content_width = (main_panel.width - page_pad * 2.0).max(180.0);
        let viewport_height = (main_panel.height - page_pad * 2.0).max(0.0);

        let filtered_count = state.filtered_books().len() as u32;
        let gap = self.theme.spacing_medium;
        let min_card = 190.0;
        let max_cols = 4_u32;
        let mut cols = ((content_width + gap) / (min_card + gap)).floor() as u32;
        cols = cols.clamp(1, max_cols);
        let rows = if filtered_count == 0 {
            0
        } else {
            filtered_count.div_ceil(cols)
        };
        let compact_tabs = content_width < 420.0;
        let tab_section_height = if compact_tabs {
            36.0 * 2.0 + self.theme.spacing_small
        } else {
            36.0
        };
        let card_height = 154.0;
        let grid_height = if rows == 0 {
            40.0
        } else {
            rows as f32 * card_height + (rows as f32 - 1.0) * gap
        };
        let total_content_height = 44.0
            + 40.0
            + self.theme.spacing_medium
            + tab_section_height
            + self.theme.spacing_medium
            + grid_height;
        state.set_library_scroll_max((total_content_height - viewport_height).max(0.0));
        let scroll = state.library_scroll;
        y -= scroll;

        draw_heading_text(
            &mut self.canvas,
            &self.fonts,
            x,
            y,
            26.0,
            self.theme.text,
            "Library",
        )?;
        y += 44.0;

        let search_rect = Rect::new(x, y, content_width.min(420.0), 40.0);
        draw_rounded_rect(
            &mut self.canvas,
            search_rect,
            self.theme.radius_medium,
            self.theme.panel,
        );
        let search_text = if state.search_query.is_empty() {
            "Search books or authors..."
        } else {
            &state.search_query
        };
        draw_text(
            &mut self.canvas,
            &self.fonts,
            search_rect.x + 14.0,
            search_rect.y + 12.0,
            14.0,
            if state.search_query.is_empty() {
                self.theme.muted_text
            } else {
                self.theme.text
            },
            search_text,
        )?;
        y += search_rect.height + self.theme.spacing_medium;

        let tabs = [
            ("All", LibraryFilter::All),
            ("In Progress", LibraryFilter::InProgress),
            ("Not Started", LibraryFilter::NotStarted),
            ("Finished", LibraryFilter::Finished),
        ];
        let tab_height = 36.0;
        let tab_gap = self.theme.spacing_small;
        let tab_width = if compact_tabs {
            (content_width - tab_gap) * 0.5
        } else {
            ((content_width - tab_gap * 3.0) * 0.25).clamp(84.0, 118.0)
        };

        for (idx, (label, filter)) in tabs.into_iter().enumerate() {
            let (row, col) = if compact_tabs {
                (idx / 2, idx % 2)
            } else {
                (0, idx)
            };
            let rect = Rect::new(
                x + col as f32 * (tab_width + tab_gap),
                y + row as f32 * (tab_height + tab_gap),
                tab_width,
                tab_height,
            );
            let action = UiAction::SetFilter(filter);
            let is_active = state.library_filter == filter;
            let is_hovered = state.interaction.hovered_action == Some(action);
            let bg = if is_active {
                self.theme.accent
            } else if is_hovered {
                Color::rgba(0xC2, 0x69, 0x4A, 35)
            } else {
                self.theme.panel
            };
            let fg = if is_active {
                Color::white()
            } else {
                self.theme.text
            };

            draw_rounded_rect(&mut self.canvas, rect, self.theme.radius_medium, bg);
            draw_text(
                &mut self.canvas,
                &self.fonts,
                rect.x + 12.0,
                rect.y + 10.0,
                13.0,
                fg,
                label,
            )?;
            self.hit_regions.push(HitRegion { rect, action });
        }
        y += tab_section_height + self.theme.spacing_medium;

        let filtered = state.filtered_books();
        let card_width = (content_width - gap * (cols as f32 - 1.0)) / cols as f32;

        for (idx, book) in filtered.iter().enumerate() {
            let row = idx as u32 / cols;
            let col = idx as u32 % cols;
            let rect = Rect::new(
                x + col as f32 * (card_width + gap),
                y + row as f32 * (card_height + gap),
                card_width,
                card_height,
            );

            self.draw_book_card(state, rect, book, 0.92)?;
            self.hit_regions.push(HitRegion {
                rect,
                action: UiAction::SelectBook(book.id),
            });
        }

        Ok(())
    }

    fn draw_player_page(&mut self, state: &AppState, main_panel: Rect) -> Result<(), String> {
        let Some(book) = state.selected_book() else {
            draw_text(
                &mut self.canvas,
                &self.fonts,
                main_panel.x + self.theme.spacing_large,
                main_panel.y + self.theme.spacing_large,
                24.0,
                self.theme.text,
                "Player",
            )?;
            return Ok(());
        };

        let horizontal_pad = if main_panel.width < 760.0 { 16.0 } else { 32.0 };
        let vertical_pad = if main_panel.height < 620.0 { 16.0 } else { 24.0 };
        let card_width = (main_panel.width - horizontal_pad * 2.0).clamp(280.0, 560.0);
        let card_height = (main_panel.height - vertical_pad * 2.0).clamp(340.0, 460.0);
        let card_x = main_panel.x + (main_panel.width - card_width) * 0.5;
        let card_y = main_panel.y + (main_panel.height - card_height) * 0.5;
        let card = Rect::new(card_x, card_y, card_width, card_height);

        draw_rounded_rect(
            &mut self.canvas,
            card,
            self.theme.radius_large,
            self.theme.panel,
        );

        draw_heading_text(
            &mut self.canvas,
            &self.fonts,
            card.x + self.theme.spacing_large,
            card.y + self.theme.spacing_large,
            24.0,
            self.theme.text,
            &book.title,
        )?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            card.x + self.theme.spacing_large,
            card.y + self.theme.spacing_large + 36.0,
            15.0,
            self.theme.muted_text,
            &book.author,
        )?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            card.x + self.theme.spacing_large,
            card.y + self.theme.spacing_large + 64.0,
            13.0,
            self.theme.muted_text,
            &book.current_chapter,
        )?;
        let (status_label, status_color) = match &state.playback_status {
            PlaybackStatus::Idle => ("Idle", self.theme.muted_text),
            PlaybackStatus::Loading => ("Loading...", self.theme.accent),
            PlaybackStatus::Playing => ("Playing", Color::rgb(0x4A, 0x9E, 0x6B)),
            PlaybackStatus::Paused => ("Paused", self.theme.muted_text),
            PlaybackStatus::Error(_) => ("Error", Color::rgb(0xB2, 0x43, 0x36)),
        };
        draw_text(
            &mut self.canvas,
            &self.fonts,
            card.x + self.theme.spacing_large,
            card.y + self.theme.spacing_large + 82.0,
            12.0,
            status_color,
            status_label,
        )?;

        if let PlaybackStatus::Error(message) = &state.playback_status {
            draw_text(
                &mut self.canvas,
                &self.fonts,
                card.x + self.theme.spacing_large,
                card.y + self.theme.spacing_large + 98.0,
                12.0,
                Color::rgb(0xB2, 0x43, 0x36),
                message,
            )?;
        }

        let progress_rect = Rect::new(
            card.x + self.theme.spacing_large,
            card.y + self.theme.spacing_large + 124.0,
            card.width - self.theme.spacing_large * 2.0,
            10.0,
        );
        draw_progress_bar(&mut self.canvas, progress_rect, book.progress, &self.theme);

        draw_text(
            &mut self.canvas,
            &self.fonts,
            progress_rect.x,
            progress_rect.y + 16.0,
            12.0,
            self.theme.muted_text,
            &format!(
                "{:.0}% • {} • bookmarks {}",
                book.progress * 100.0,
                book.duration_text,
                state.bookmark_count
            ),
        )?;

        let bookmarks_title_y = progress_rect.y + 42.0;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            card.x + self.theme.spacing_large,
            bookmarks_title_y,
            12.0,
            self.theme.muted_text,
            "Bookmarks",
        )?;

        if state.bookmarks.is_empty() {
            draw_text(
                &mut self.canvas,
                &self.fonts,
                card.x + self.theme.spacing_large,
                bookmarks_title_y + 18.0,
                12.0,
                self.theme.muted_text,
                "No bookmarks yet",
            )?;
        } else {
            for (index, bookmark) in state.bookmarks.iter().rev().take(3).enumerate() {
                let seconds = (bookmark.position_ms.max(0) / 1000) as i64;
                let line = format!("• {:02}:{:02}:{:02}", seconds / 3600, (seconds / 60) % 60, seconds % 60);
                draw_text(
                    &mut self.canvas,
                    &self.fonts,
                    card.x + self.theme.spacing_large,
                    bookmarks_title_y + 18.0 + index as f32 * 16.0,
                    12.0,
                    self.theme.text,
                    &line,
                )?;
            }
        }

        let controls_origin = (
            card.x + card.width * 0.5 - 120.0,
            card.y + card.height - 86.0,
        );
        let (back_rect, play_rect, forward_rect) =
            self.draw_player_controls(state, controls_origin)?;
        self.hit_regions.push(HitRegion {
            rect: back_rect,
            action: UiAction::SeekBackward,
        });
        self.hit_regions.push(HitRegion {
            rect: play_rect,
            action: UiAction::PlayPause,
        });
        self.hit_regions.push(HitRegion {
            rect: forward_rect,
            action: UiAction::SeekForward,
        });
        self.hit_regions.push(HitRegion {
            rect: Rect::new(
                card.x + card.width - 130.0,
                card.y + card.height - 52.0,
                112.0,
                32.0,
            ),
            action: UiAction::AddBookmark,
        });
        let bookmark_button = Rect::new(
            card.x + card.width - 130.0,
            card.y + card.height - 52.0,
            112.0,
            32.0,
        );
        let bookmark_hovered = state.interaction.hovered_action == Some(UiAction::AddBookmark);
        draw_rounded_rect(
            &mut self.canvas,
            bookmark_button,
            self.theme.radius_small,
            if bookmark_hovered {
                Color::rgba(0xC2, 0x69, 0x4A, 55)
            } else {
                Color::rgba(0xC2, 0x69, 0x4A, 35)
            },
        );
        draw_text(
            &mut self.canvas,
            &self.fonts,
            bookmark_button.x + 12.0,
            bookmark_button.y + 9.0,
            12.0,
            self.theme.text,
            "Bookmark (B)",
        )?;

        if let Some(message) = &state.transient_message {
            draw_text(
                &mut self.canvas,
                &self.fonts,
                card.x + card.width - 156.0,
                card.y + self.theme.spacing_large,
                12.0,
                Color::rgb(0x4A, 0x9E, 0x6B),
                message,
            )?;
        }

        Ok(())
    }

    fn draw_settings_page(&mut self, main_panel: Rect) -> Result<(), String> {
        draw_heading_text(
            &mut self.canvas,
            &self.fonts,
            main_panel.x + self.theme.spacing_large,
            main_panel.y + self.theme.spacing_large,
            26.0,
            self.theme.text,
            "Settings",
        )?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            main_panel.x + self.theme.spacing_large,
            main_panel.y + self.theme.spacing_large + 44.0,
            15.0,
            self.theme.muted_text,
            "Phase 4 placeholder",
        )?;
        Ok(())
    }

    fn draw_book_card(
        &mut self,
        state: &AppState,
        rect: Rect,
        book: &Audiobook,
        title_scale: f32,
    ) -> Result<Rect, String> {
        let action = UiAction::SelectBook(book.id);
        let is_hovered = state.interaction.hovered_action == Some(action);
        let is_selected = state.selected_book_id == Some(book.id);

        let bg = if is_selected {
            Color::rgba(0xC2, 0x69, 0x4A, 28)
        } else if is_hovered {
            Color::rgba(0xF5, 0xF0, 0xEA, 255)
        } else {
            Color::white()
        };
        draw_rounded_rect(&mut self.canvas, rect, self.theme.radius_medium, bg);

        let inner = rect.inset(14.0);
        draw_text(
            &mut self.canvas,
            &self.fonts,
            inner.x,
            inner.y,
            (16.0 * title_scale).max(11.0),
            self.theme.text,
            &book.title,
        )?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            inner.x,
            inner.y + 22.0,
            12.0,
            self.theme.muted_text,
            &book.author,
        )?;

        let progress_rect = Rect::new(inner.x, rect.y + rect.height - 36.0, inner.width, 8.0);
        draw_progress_bar(&mut self.canvas, progress_rect, book.progress, &self.theme);

        Ok(rect)
    }

    fn draw_continue_card(
        &mut self,
        state: &AppState,
        card: Rect,
        book: &Audiobook,
        button_action: UiAction,
    ) -> Result<Rect, String> {
        draw_rounded_rect(
            &mut self.canvas,
            card,
            self.theme.radius_large,
            Color::white(),
        );

        let accent_strip = Rect::new(card.x, card.y, 8.0, card.height);
        draw_rounded_rect(
            &mut self.canvas,
            accent_strip,
            self.theme.radius_small,
            self.theme.accent,
        );

        let left = card.inset(self.theme.spacing_large);
        draw_text(
            &mut self.canvas,
            &self.fonts,
            left.x,
            left.y,
            22.0,
            self.theme.text,
            &book.title,
        )?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            left.x,
            left.y + 34.0,
            14.0,
            self.theme.muted_text,
            &book.author,
        )?;
        draw_text(
            &mut self.canvas,
            &self.fonts,
            left.x,
            left.y + 56.0,
            13.0,
            self.theme.muted_text,
            &book.current_chapter,
        )?;

        let progress_rect = Rect::new(
            left.x,
            card.y + card.height - 50.0,
            card.width - 190.0,
            10.0,
        );
        draw_progress_bar(&mut self.canvas, progress_rect, book.progress, &self.theme);

        draw_text(
            &mut self.canvas,
            &self.fonts,
            progress_rect.x,
            progress_rect.y + 16.0,
            12.0,
            self.theme.muted_text,
            &format!("{:.0}% • {}", book.progress * 100.0, book.duration_text),
        )?;

        let button_rect = Rect::new(
            card.x + card.width - 150.0,
            card.y + card.height - 62.0,
            124.0,
            42.0,
        );
        let is_hovered = state.interaction.hovered_action == Some(button_action);
        draw_rounded_rect(
            &mut self.canvas,
            button_rect,
            self.theme.radius_medium,
            if is_hovered {
                Color::rgb(0xA6, 0x52, 0x37)
            } else {
                self.theme.accent
            },
        );
        draw_text(
            &mut self.canvas,
            &self.fonts,
            button_rect.x + 18.0,
            button_rect.y + 13.0,
            15.0,
            Color::white(),
            "Continue",
        )?;

        Ok(button_rect)
    }

    fn draw_player_controls(
        &mut self,
        state: &AppState,
        origin: (f32, f32),
    ) -> Result<(Rect, Rect, Rect), String> {
        let (x, y) = origin;
        let back_rect = Rect::new(x, y, 64.0, 44.0);
        let play_rect = Rect::new(x + 78.0, y - 6.0, 84.0, 56.0);
        let forward_rect = Rect::new(x + 178.0, y, 64.0, 44.0);

        let draw_control = |canvas: &mut Canvas<OpenGl>,
                            fonts: &Fonts,
                            theme: &Theme,
                            rect: Rect,
                            label: &str,
                            hovered: bool,
                            primary: bool|
         -> Result<(), String> {
            let bg = if primary {
                if hovered {
                    if state.is_playing {
                        Color::rgb(0x3D, 0x86, 0x59)
                    } else {
                        Color::rgb(0xA6, 0x52, 0x37)
                    }
                } else {
                    if state.is_playing {
                        Color::rgb(0x4A, 0x9E, 0x6B)
                    } else {
                        theme.accent
                    }
                }
            } else if hovered {
                Color::rgba(0xC2, 0x69, 0x4A, 35)
            } else {
                Color::white()
            };
            let fg = if primary { Color::white() } else { theme.text };
            draw_rounded_rect(canvas, rect, theme.radius_medium, bg);
            draw_text(
                canvas,
                fonts,
                rect.x + 18.0,
                rect.y + if primary { 19.0 } else { 13.0 },
                if primary { 16.0 } else { 14.0 },
                fg,
                label,
            )
        };

        draw_control(
            &mut self.canvas,
            &self.fonts,
            &self.theme,
            back_rect,
            "-15s",
            state.interaction.hovered_action == Some(UiAction::SeekBackward),
            false,
        )?;
        draw_control(
            &mut self.canvas,
            &self.fonts,
            &self.theme,
            play_rect,
            if state.is_playing { "Pause" } else { "Play" },
            state.interaction.hovered_action == Some(UiAction::PlayPause),
            true,
        )?;
        draw_control(
            &mut self.canvas,
            &self.fonts,
            &self.theme,
            forward_rect,
            "+30s",
            state.interaction.hovered_action == Some(UiAction::SeekForward),
            false,
        )?;

        Ok((back_rect, play_rect, forward_rect))
    }
}
