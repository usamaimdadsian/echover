use crate::ui::action::UiAction;
use crate::ui::data::Library;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use crate::ui::widgets::book_card::book_card;

const HERO_HEIGHT: f32 = 240.0;

pub fn layout(
    rect: Rect,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let library = &state.library;
    let Some(book) = library.current_listening() else {
        empty_state(rect, theme, font, draw);
        return;
    };

    let hero = Rect::new(rect.x, rect.y, rect.width, HERO_HEIGHT);
    draw.rounded_rect(hero, theme.radius_large, theme.panel);

    let cover_size = HERO_HEIGHT - theme.spacing_large * 2.0;
    let cover = Rect::new(
        hero.x + theme.spacing_large,
        hero.y + theme.spacing_large,
        cover_size * 0.75,
        cover_size,
    );
    draw.rounded_rect(cover, theme.radius_medium, theme.accent.with_alpha(0.55));

    let text_x = cover.x + cover.width + theme.spacing_large;
    let text_w = (hero.x + hero.width - theme.spacing_large) - text_x;

    let eyebrow_y = hero.y + theme.spacing_large;
    draw.text(
        text_x,
        eyebrow_y,
        theme.text_eyebrow,
        theme.accent,
        "CONTINUE LISTENING",
        font,
    );

    let title_y = eyebrow_y + font.line_height(theme.text_eyebrow) + 4.0;
    draw.text(
        text_x,
        title_y,
        theme.text_title,
        theme.text,
        &book.title,
        font,
    );

    let subtitle_y = title_y + font.line_height(theme.text_title);
    let subtitle = format!("{}  ·  {}", book.author, book.current_chapter);
    draw.text(
        text_x,
        subtitle_y,
        theme.text_subtitle,
        theme.muted_text,
        &subtitle,
        font,
    );

    let remain_y = hero.y + HERO_HEIGHT - 122.0;
    draw.text(
        text_x,
        remain_y,
        theme.text_caption,
        theme.muted_text,
        &book.remaining_text,
        font,
    );
    let progress = Rect::new(text_x, hero.y + HERO_HEIGHT - 96.0, text_w, 8.0);
    draw.progress_bar(progress, book.progress, theme);

    let action = UiAction::ContinueListening;
    let btn = Rect::new(text_x, hero.y + HERO_HEIGHT - 64.0, 168.0, 40.0);
    let bg = interaction.shade(action, theme.accent, theme);
    draw.rounded_rect(btn, 20.0, bg);
    let label = "Continue";
    let label_w = font.measure(label, theme.text_button);
    let label_x = btn.x + (btn.width - label_w) * 0.5;
    let label_y = btn.y + (btn.height - font.line_height(theme.text_button)) * 0.5;
    draw.text(
        label_x,
        label_y,
        theme.text_button,
        theme.panel,
        label,
        font,
    );
    hits.push(HitRegion { rect: btn, action });

    let heading_y = hero.y + hero.height + theme.spacing_large;
    draw.text(
        rect.x,
        heading_y,
        theme.text_section,
        theme.text,
        "Recently played",
        font,
    );
    let row_y = heading_y + font.line_height(theme.text_section) + theme.spacing_medium;
    let avail_h = (rect.y + rect.height - row_y).max(0.0);
    let row_h = avail_h.min(240.0);
    book_strip(
        rect.x,
        row_y,
        rect.width,
        row_h,
        4,
        library,
        theme,
        font,
        interaction,
        draw,
        hits,
    );
}

fn empty_state(rect: Rect, theme: &Theme, font: &Font, draw: &mut DrawList) {
    draw.text(
        rect.x,
        rect.y,
        theme.text_title,
        theme.text,
        "Welcome to Echover",
        font,
    );
    let y = rect.y + font.line_height(theme.text_title) + 4.0;
    draw.text(
        rect.x,
        y,
        theme.text_subtitle,
        theme.muted_text,
        "Set ECHOVER_LIBRARY_PATH to a folder of audiobooks and restart to begin.",
        font,
    );
}

#[allow(clippy::too_many_arguments)]
fn book_strip(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    count: usize,
    library: &Library,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    if width <= 0.0 || height <= 0.0 || count == 0 {
        return;
    }
    let gap = theme.spacing_medium;
    let card_w = ((width - gap * (count as f32 - 1.0)) / count as f32).max(0.0);
    for (i, book) in library.books.iter().take(count).enumerate() {
        let cx = x + i as f32 * (card_w + gap);
        book_card(
            Rect::new(cx, y, card_w, height),
            theme,
            font,
            interaction,
            draw,
            hits,
            book,
        );
    }
}
