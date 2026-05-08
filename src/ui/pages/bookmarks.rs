use crate::ui::action::UiAction;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use crate::ui::widgets::book_card::cover_color;

const ROW_HEIGHT: f32 = 76.0;
const ROW_GAP: f32 = 12.0;
const COVER_SIZE: f32 = 56.0;
const TIME_CHIP_W: f32 = 96.0;
const TIME_CHIP_H: f32 = 28.0;

pub fn layout(
    rect: Rect,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    draw.text(
        rect.x,
        rect.y,
        theme.text_title,
        theme.text,
        "Bookmarks",
        font,
    );
    let subhead_y = rect.y + font.line_height(theme.text_title);
    let bookmarks = &state.library.bookmarks;
    let count = bookmarks.len();
    let subhead = if count == 0 {
        "No bookmarks yet — press B while playing to save the current spot.".to_string()
    } else {
        format!("{count} saved across your library")
    };
    draw.text(
        rect.x,
        subhead_y,
        theme.text_subtitle,
        theme.muted_text,
        &subhead,
        font,
    );

    if count == 0 {
        return;
    }

    let list_y = subhead_y + font.line_height(theme.text_subtitle) + theme.spacing_large;
    let list_h = (rect.y + rect.height - list_y).max(0.0);
    let max_rows = ((list_h + ROW_GAP) / (ROW_HEIGHT + ROW_GAP)).floor() as usize;
    let rows = max_rows.min(count);

    for (i, bookmark) in bookmarks.iter().take(rows).enumerate() {
        let row = Rect::new(
            rect.x,
            list_y + i as f32 * (ROW_HEIGHT + ROW_GAP),
            rect.width,
            ROW_HEIGHT,
        );
        let action = UiAction::JumpToBookmark(bookmark.book_id, bookmark.position_ms);
        let row_bg = interaction.shade(action, theme.panel, theme);
        draw.rounded_rect(row, theme.radius_medium, row_bg);

        let cover = Rect::new(
            row.x + theme.spacing_medium,
            row.y + (ROW_HEIGHT - COVER_SIZE) * 0.5,
            COVER_SIZE,
            COVER_SIZE,
        );
        draw.rounded_rect(cover, theme.radius_small, cover_color(theme, bookmark.book_id));

        let text_x = cover.x + COVER_SIZE + theme.spacing_medium;
        draw.text(
            text_x,
            row.y + 14.0,
            theme.text_body,
            theme.text,
            &bookmark.note,
            font,
        );
        draw.text(
            text_x,
            row.y + 14.0 + font.line_height(theme.text_body) + 2.0,
            theme.text_caption,
            theme.muted_text,
            &bookmark.book_title,
            font,
        );

        let chip = Rect::new(
            row.x + row.width - theme.spacing_medium - TIME_CHIP_W,
            row.y + (ROW_HEIGHT - TIME_CHIP_H) * 0.5,
            TIME_CHIP_W,
            TIME_CHIP_H,
        );
        draw.rounded_rect(chip, TIME_CHIP_H * 0.5, theme.accent.with_alpha(0.18));
        let time_w = font.measure(&bookmark.timestamp, theme.text_caption);
        let time_x = chip.x + (chip.width - time_w) * 0.5;
        let time_y = chip.y + (chip.height - font.line_height(theme.text_caption)) * 0.5;
        draw.text(
            time_x,
            time_y,
            theme.text_caption,
            theme.accent,
            &bookmark.timestamp,
            font,
        );

        hits.push(HitRegion { rect: row, action });
    }
}
