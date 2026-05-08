use crate::ui::action::UiAction;
use crate::ui::data::DisplayBook;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::theme::{Color, Theme};

const COVER_INSET: f32 = 12.0;
const META_BLOCK: f32 = 60.0;

#[allow(clippy::too_many_arguments)]
pub fn book_card(
    rect: Rect,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
    book: &DisplayBook,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    let action = UiAction::SelectBook(book.id);
    let panel = interaction.shade(action, theme.panel, theme);
    draw.rounded_rect(rect, theme.radius_large, panel);

    let cover_h = (rect.height - COVER_INSET * 2.0 - META_BLOCK).max(0.0);
    let cover = Rect::new(
        rect.x + COVER_INSET,
        rect.y + COVER_INSET,
        rect.width - COVER_INSET * 2.0,
        cover_h,
    );
    draw.rounded_rect(cover, theme.radius_medium, cover_color(theme, book.id));

    let title_y = cover.y + cover.height + theme.spacing_small;
    draw.text(
        cover.x,
        title_y,
        theme.text_body,
        theme.text,
        &book.title,
        font,
    );
    let author_y = title_y + font.line_height(theme.text_body);
    draw.text(
        cover.x,
        author_y,
        theme.text_caption,
        theme.muted_text,
        &book.author,
        font,
    );

    if book.progress > 0.0 && book.progress < 1.0 {
        let bar = Rect::new(
            cover.x,
            author_y + font.line_height(theme.text_caption) + 4.0,
            cover.width,
            4.0,
        );
        draw.progress_bar(bar, book.progress, theme);
    }

    if book.completed {
        // Small "Finished" pill in the top-right corner of the cover area.
        let pad_x = 8.0;
        let pad_y = 4.0;
        let label = "Finished";
        let label_w = font.measure(label, theme.text_eyebrow);
        let pill_w = label_w + pad_x * 2.0;
        let pill_h = font.line_height(theme.text_eyebrow) + pad_y * 2.0;
        let pill = Rect::new(
            cover.x + cover.width - pill_w - 8.0,
            cover.y + 8.0,
            pill_w,
            pill_h,
        );
        draw.rounded_rect(pill, pill_h * 0.5, theme.accent);
        draw.text(
            pill.x + pad_x,
            pill.y + pad_y,
            theme.text_eyebrow,
            theme.panel,
            label,
            font,
        );
    }

    hits.push(HitRegion { rect, action });
}

pub fn cover_color(theme: &Theme, id: u64) -> Color {
    let palette = [
        theme.accent.with_alpha(0.55),
        theme.muted_text.with_alpha(0.45),
        theme.text.with_alpha(0.65),
        Color::rgb_u8(0x6E, 0x8F, 0x82),
        Color::rgb_u8(0xB8, 0x86, 0x6B),
        Color::rgb_u8(0x4A, 0x4E, 0x69),
    ];
    palette[(id as usize) % palette.len()]
}
