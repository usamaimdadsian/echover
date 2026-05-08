use crate::ui::action::UiAction;
use crate::ui::data::{DisplayChapter, Library};
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::icons::IconId;
use crate::ui::primitives::DrawList;
use crate::ui::theme::Theme;
use crate::ui::widgets::book_card::cover_color;

const HERO_HEIGHT: f32 = 260.0;
const COVER_W: f32 = 180.0;
const CHAPTER_ROW_HEIGHT: f32 = 56.0;
const CHAPTER_ROW_GAP: f32 = 8.0;

#[allow(clippy::too_many_arguments)]
pub fn layout(
    rect: Rect,
    book_id: u64,
    theme: &Theme,
    library: &Library,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let Some(book) = library.find_book(book_id) else {
        draw.text(
            rect.x,
            rect.y,
            theme.text_subtitle,
            theme.muted_text,
            "Book not found.",
            font,
        );
        return;
    };

    let hero = Rect::new(rect.x, rect.y, rect.width, HERO_HEIGHT);
    draw.rounded_rect(hero, theme.radius_large, theme.panel);

    let cover = Rect::new(
        hero.x + theme.spacing_large,
        hero.y + theme.spacing_large,
        COVER_W,
        HERO_HEIGHT - theme.spacing_large * 2.0,
    );
    draw.rounded_rect(cover, theme.radius_medium, cover_color(theme, book.id));

    let text_x = cover.x + cover.width + theme.spacing_large;
    let text_w = (hero.x + hero.width - theme.spacing_large) - text_x;

    let eyebrow_y = cover.y;
    draw.text(
        text_x,
        eyebrow_y,
        theme.text_eyebrow,
        theme.accent,
        "AUDIOBOOK",
        font,
    );
    let title_y = eyebrow_y + font.line_height(theme.text_eyebrow) + 4.0;
    draw.text(
        text_x,
        title_y,
        theme.text_display,
        theme.text,
        &book.title,
        font,
    );

    let author_y = title_y + font.line_height(theme.text_display);
    let author_line = format!("{}  ·  Narrated by {}", book.author, book.narrator);
    draw.text(
        text_x,
        author_y,
        theme.text_subtitle,
        theme.muted_text,
        &author_line,
        font,
    );

    let progress_y = hero.y + HERO_HEIGHT - 96.0;
    let progress = Rect::new(text_x, progress_y, text_w, 8.0);
    draw.progress_bar(progress, book.progress, theme);

    let remain_y = progress_y - font.line_height(theme.text_caption) - 4.0;
    draw.text(
        text_x,
        remain_y,
        theme.text_caption,
        theme.muted_text,
        &book.remaining_text,
        font,
    );

    let btn_y = hero.y + HERO_HEIGHT - 56.0;
    let play = Rect::new(text_x, btn_y, 168.0, 40.0);
    let play_action = UiAction::ContinueListening;
    draw.rounded_rect(play, 20.0, interaction.shade(play_action, theme.accent, theme));
    let play_label = if book.progress > 0.0 && book.progress < 1.0 {
        "Resume"
    } else {
        "Play"
    };
    centered_label(play, play_label, theme.text_button, theme.panel, font, draw);
    hits.push(HitRegion {
        rect: play,
        action: play_action,
    });

    let bookmark = Rect::new(play.x + play.width + theme.spacing_small, btn_y, 140.0, 40.0);
    let bookmark_action = UiAction::AddBookmark;
    draw.rounded_rect(
        bookmark,
        20.0,
        interaction.shade(bookmark_action, theme.track, theme),
    );
    centered_label(
        bookmark,
        "Bookmark",
        theme.text_button,
        theme.text,
        font,
        draw,
    );
    hits.push(HitRegion {
        rect: bookmark,
        action: bookmark_action,
    });

    let back = Rect::new(
        hero.x + hero.width - theme.spacing_large - 96.0,
        hero.y + theme.spacing_large,
        96.0,
        32.0,
    );
    let back_action = UiAction::NavigateLibrary;
    draw.rounded_rect(
        back,
        theme.radius_small,
        interaction.shade(back_action, theme.background, theme),
    );
    let icon_size = 14.0;
    let icon_x = back.x + 10.0;
    let icon_y = back.y + (back.height - icon_size) * 0.5;
    draw.icon(
        Rect::new(icon_x, icon_y, icon_size, icon_size),
        theme.text,
        font,
        IconId::ArrowLeft,
    );
    let label = "Library";
    let lw = font.measure(label, theme.text_caption);
    let lx = (icon_x + icon_size + 8.0).max(back.x + (back.width - lw) * 0.5);
    draw.text(
        lx,
        back.y + (back.height - font.line_height(theme.text_caption)) * 0.5,
        theme.text_caption,
        theme.text,
        label,
        font,
    );
    hits.push(HitRegion {
        rect: back,
        action: back_action,
    });

    let list_y = hero.y + hero.height + theme.spacing_large;
    draw.text(
        rect.x,
        list_y,
        theme.text_section,
        theme.text,
        "Chapters",
        font,
    );
    let row_y0 = list_y + font.line_height(theme.text_section) + theme.spacing_medium;
    let avail = (rect.y + rect.height - row_y0).max(0.0);

    if book.chapters.is_empty() {
        draw.text(
            rect.x,
            row_y0,
            theme.text_body,
            theme.muted_text,
            "Chapter list not available for this book yet.",
            font,
        );
        return;
    }

    let max_rows =
        ((avail + CHAPTER_ROW_GAP) / (CHAPTER_ROW_HEIGHT + CHAPTER_ROW_GAP)).floor() as usize;
    let rows = max_rows.min(book.chapters.len());

    for (i, chapter) in book.chapters.iter().take(rows).enumerate() {
        let row = Rect::new(
            rect.x,
            row_y0 + i as f32 * (CHAPTER_ROW_HEIGHT + CHAPTER_ROW_GAP),
            rect.width,
            CHAPTER_ROW_HEIGHT,
        );
        chapter_row(row, chapter, book.id, theme, interaction, font, draw, hits);
    }
}

#[allow(clippy::too_many_arguments)]
fn chapter_row(
    row: Rect,
    chapter: &DisplayChapter,
    book_id: u64,
    theme: &Theme,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let action = UiAction::SelectChapter(book_id, chapter.index);
    let bg = interaction.shade(action, theme.panel, theme);
    draw.rounded_rect(row, theme.radius_medium, bg);

    let index_label = format!("{:02}", chapter.index);
    let index_w = font.measure(&index_label, theme.text_section);
    draw.text(
        row.x + theme.spacing_medium + (32.0 - index_w) * 0.5,
        row.y + (row.height - font.line_height(theme.text_section)) * 0.5,
        theme.text_section,
        theme.muted_text,
        &index_label,
        font,
    );

    let title_x = row.x + theme.spacing_medium + 56.0;
    let title_y = row.y + (row.height - font.line_height(theme.text_body)) * 0.5;
    draw.text(
        title_x,
        title_y,
        theme.text_body,
        theme.text,
        &chapter.title,
        font,
    );

    let dur_w = font.measure(&chapter.duration_text, theme.text_caption);
    draw.text(
        row.x + row.width - theme.spacing_medium - dur_w,
        row.y + (row.height - font.line_height(theme.text_caption)) * 0.5,
        theme.text_caption,
        theme.muted_text,
        &chapter.duration_text,
        font,
    );

    hits.push(HitRegion { rect: row, action });
}

fn centered_label(
    rect: Rect,
    label: &str,
    size: u8,
    color: crate::ui::theme::Color,
    font: &Font,
    draw: &mut DrawList,
) {
    let w = font.measure(label, size);
    let x = rect.x + (rect.width - w) * 0.5;
    let y = rect.y + (rect.height - font.line_height(size)) * 0.5;
    draw.text(x, y, size, color, label, font);
}
