use crate::ui::action::UiAction;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::icons::IconId;
use crate::ui::pages;
use crate::ui::primitives::DrawList;
use crate::ui::state::{AppPage, AppState};
use crate::ui::theme::Theme;

const SIDEBAR_WIDTH: f32 = 220.0;
const NAV_ROW_HEIGHT: f32 = 40.0;
const NAV_ROW_GAP: f32 = 4.0;
const LOGO_SIZE: f32 = 32.0;
const NAV_ICON_SIZE: f32 = 18.0;

#[allow(clippy::too_many_arguments)]
pub fn layout(
    width: f32,
    height: f32,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let outer = Rect::new(0.0, 0.0, width, height).inset(theme.spacing_large);
    let (sidebar, main_area) = outer.split_horizontal(SIDEBAR_WIDTH);

    layout_sidebar(
        sidebar,
        theme,
        state.current_page,
        interaction,
        font,
        draw,
        hits,
    );

    let main_panel = Rect::new(
        main_area.x + theme.spacing_medium,
        main_area.y,
        (main_area.width - theme.spacing_medium).max(0.0),
        main_area.height,
    );
    layout_page(main_panel, theme, state, interaction, font, draw, hits);
}

#[allow(clippy::too_many_arguments)]
fn layout_sidebar(
    sidebar: Rect,
    theme: &Theme,
    current_page: AppPage,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    draw.rounded_rect(sidebar, theme.radius_large, theme.panel);

    let logo = Rect::new(
        sidebar.x + theme.spacing_large,
        sidebar.y + theme.spacing_large,
        LOGO_SIZE,
        LOGO_SIZE,
    );
    draw.rounded_rect(logo, theme.radius_small, theme.accent);

    let nav_x = sidebar.x + theme.spacing_medium;
    let nav_width = sidebar.width - theme.spacing_medium * 2.0;
    let mut nav_y = logo.y + LOGO_SIZE + theme.spacing_large + theme.spacing_small;

    for &(page, action, label, icon) in NAV_ITEMS {
        let row = Rect::new(nav_x, nav_y, nav_width, NAV_ROW_HEIGHT);
        let is_active = current_page == page
            || (matches!(current_page, AppPage::BookDetail(_)) && page == AppPage::Library);

        let base = if is_active {
            theme.accent.with_alpha(0.18)
        } else {
            theme.panel.with_alpha(0.0)
        };
        let bg = interaction.shade(action, base, theme);
        if bg.a > 0.001 {
            draw.rounded_rect(row, theme.radius_small, bg);
        }

        let label_color = if is_active {
            theme.accent
        } else {
            theme.muted_text
        };

        let icon_rect = Rect::new(
            row.x + theme.spacing_medium,
            row.y + (row.height - NAV_ICON_SIZE) * 0.5,
            NAV_ICON_SIZE,
            NAV_ICON_SIZE,
        );
        draw.icon(icon_rect, label_color, font, icon);

        let text_x = icon_rect.x + icon_rect.width + theme.spacing_small + 2.0;
        let text_top = row.y + (row.height - font.line_height(theme.text_body)) * 0.5;
        draw.text(text_x, text_top, theme.text_body, label_color, label, font);

        hits.push(HitRegion { rect: row, action });
        nav_y += NAV_ROW_HEIGHT + NAV_ROW_GAP;
    }

    // "Add audiobook folder" lives at the bottom of the sidebar so it's
    // always one click away regardless of which page is active.
    let add_row = Rect::new(
        nav_x,
        sidebar.y + sidebar.height - NAV_ROW_HEIGHT - theme.spacing_medium,
        nav_width,
        NAV_ROW_HEIGHT,
    );
    let add_action = UiAction::AddLibraryFolder;
    let add_bg = interaction.shade(add_action, theme.accent, theme);
    draw.rounded_rect(add_row, theme.radius_small, add_bg);

    let icon_rect = Rect::new(
        add_row.x + theme.spacing_medium,
        add_row.y + (add_row.height - NAV_ICON_SIZE) * 0.5,
        NAV_ICON_SIZE,
        NAV_ICON_SIZE,
    );
    draw.icon(icon_rect, theme.panel, font, IconId::FolderPlus);
    let text_x = icon_rect.x + icon_rect.width + theme.spacing_small + 2.0;
    let text_top = add_row.y + (add_row.height - font.line_height(theme.text_body)) * 0.5;
    draw.text(
        text_x,
        text_top,
        theme.text_body,
        theme.panel,
        "Add audiobook",
        font,
    );
    hits.push(HitRegion {
        rect: add_row,
        action: add_action,
    });
}

fn layout_page(
    rect: Rect,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    match state.current_page {
        AppPage::Home => pages::home::layout(rect, theme, state, interaction, font, draw, hits),
        AppPage::Library => {
            pages::library::layout(rect, theme, state, interaction, font, draw, hits)
        }
        AppPage::Player => {
            pages::player::layout(rect, theme, state, interaction, font, draw, hits)
        }
        AppPage::Bookmarks => {
            pages::bookmarks::layout(rect, theme, state, interaction, font, draw, hits)
        }
        AppPage::Settings => {
            pages::settings::layout(rect, theme, state, interaction, font, draw, hits)
        }
        AppPage::BookDetail(id) => pages::book_detail::layout(
            rect,
            id,
            theme,
            &state.library,
            interaction,
            font,
            draw,
            hits,
        ),
    }
}

const NAV_ITEMS: &[(AppPage, UiAction, &str, IconId)] = &[
    (AppPage::Home, UiAction::NavigateHome, "Home", IconId::Home),
    (
        AppPage::Library,
        UiAction::NavigateLibrary,
        "Library",
        IconId::Library,
    ),
    (
        AppPage::Player,
        UiAction::NavigatePlayer,
        "Player",
        IconId::Player,
    ),
    (
        AppPage::Bookmarks,
        UiAction::NavigateBookmarks,
        "Bookmarks",
        IconId::Bookmarks,
    ),
    (
        AppPage::Settings,
        UiAction::NavigateSettings,
        "Settings",
        IconId::Settings,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn render_page(page: AppPage) -> (usize, usize) {
        let theme = Theme::default();
        let state = AppState {
            current_page: page,
            library: crate::ui::data::Library::sample_for_tests(),
            ..AppState::default()
        };
        let font = Font::empty_for_tests();
        let interaction = Interaction::default();
        let mut draw = DrawList::default();
        let mut hits = Vec::new();
        layout(
            1200.0,
            800.0,
            &theme,
            &state,
            &interaction,
            &font,
            &mut draw,
            &mut hits,
        );
        (draw.commands.len(), hits.len())
    }

    #[test]
    fn every_page_emits_draw_commands_and_nav_hits() {
        for page in [
            AppPage::Home,
            AppPage::Library,
            AppPage::Player,
            AppPage::Bookmarks,
            AppPage::Settings,
            AppPage::BookDetail(1),
        ] {
            let (rects, hits) = render_page(page);
            assert!(rects > 5, "{page:?} produced only {rects} rects");
            // 5 nav rows + 1 "Add folder" row = at least 6 hits per page.
            assert!(hits >= 6, "{page:?} produced only {hits} hit regions");
        }
    }

    #[test]
    fn active_nav_item_matches_current_page() {
        let mut seen = std::collections::HashSet::new();
        for &(page, _, _, _) in NAV_ITEMS {
            assert!(seen.insert(page), "duplicate nav entry for {page:?}");
        }
        assert_eq!(seen.len(), 5);
    }
}
