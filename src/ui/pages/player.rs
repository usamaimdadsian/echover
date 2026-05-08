use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use crate::ui::widgets::player_controls::{bookmark_button, transport};

pub fn layout(
    rect: Rect,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let card = rect;
    draw.rounded_rect(card, theme.radius_large, theme.panel);

    let Some(book) = state.library.current_listening() else {
        draw.text(
            card.x + theme.spacing_large,
            card.y + theme.spacing_large,
            theme.text_subtitle,
            theme.muted_text,
            "No audiobook to play yet.",
            font,
        );
        return;
    };

    // Live position overrides the saved snapshot when the engine has this
    // book loaded. Falls back to whatever was on disk at startup.
    let total_ms = book.total_duration_ms;
    let position_ms = if state.loaded_audiobook_id == Some(book.id) {
        state.playback_position_ms
    } else {
        // Approximate from the snapshot's progress so the bar isn't blank.
        ((book.progress as f64) * total_ms as f64) as i64
    };
    let live_progress = if total_ms > 0 {
        (position_ms as f32 / total_ms as f32).clamp(0.0, 1.0)
    } else {
        book.progress
    };
    let live_remaining = if total_ms > 0 {
        format_remaining_ms((total_ms - position_ms).max(0))
    } else {
        book.remaining_text.clone()
    };

    let cover_size = (card.height * 0.40).min(card.width * 0.42).max(160.0);
    let cover = Rect::new(
        card.x + (card.width - cover_size) * 0.5,
        card.y + theme.spacing_large * 1.5,
        cover_size,
        cover_size,
    );
    draw.rounded_rect(cover, theme.radius_large, theme.accent.with_alpha(0.55));

    let mut text_y = cover.y + cover_size + theme.spacing_large;
    centered_text(
        card,
        text_y,
        theme.text_title,
        theme.text,
        &book.title,
        font,
        draw,
    );
    text_y += font.line_height(theme.text_title) + 4.0;
    centered_text(
        card,
        text_y,
        theme.text_subtitle,
        theme.muted_text,
        &author_line(&book.author, &book.narrator),
        font,
        draw,
    );
    text_y += font.line_height(theme.text_subtitle) + theme.spacing_small;
    centered_text(
        card,
        text_y,
        theme.text_caption,
        theme.accent,
        &book.current_chapter,
        font,
        draw,
    );

    let progress_y = card.y + card.height - 150.0;
    let progress = Rect::new(
        card.x + theme.spacing_large * 2.0,
        progress_y,
        card.width - theme.spacing_large * 4.0,
        8.0,
    );

    let label_y = progress_y - font.line_height(theme.text_caption) - 6.0;
    draw.text(
        progress.x,
        label_y,
        theme.text_caption,
        theme.muted_text,
        elapsed_label(live_progress, &book.duration_text).as_str(),
        font,
    );
    let remain_w = font.measure(&live_remaining, theme.text_caption);
    draw.text(
        progress.x + progress.width - remain_w,
        label_y,
        theme.text_caption,
        theme.muted_text,
        &live_remaining,
        font,
    );
    draw.progress_bar(progress, live_progress, theme);

    let controls_top = progress_y + 24.0;
    let cluster = transport(
        card.x + card.width * 0.5,
        controls_top,
        state.is_playing,
        theme,
        font,
        interaction,
        draw,
        hits,
    );

    bookmark_button(
        card.x + card.width - theme.spacing_large * 2.0,
        cluster.y + cluster.height * 0.5,
        theme,
        font,
        interaction,
        draw,
        hits,
    );
}

fn centered_text(
    card: Rect,
    y: f32,
    size: u8,
    color: crate::ui::theme::Color,
    text: &str,
    font: &Font,
    draw: &mut DrawList,
) {
    let w = font.measure(text, size);
    let x = card.x + (card.width - w) * 0.5;
    draw.text(x, y, size, color, text, font);
}

fn author_line(author: &str, narrator: &str) -> String {
    format!("{author}  ·  Narrated by {narrator}")
}

fn elapsed_label(progress: f32, duration_text: &str) -> String {
    let pct = (progress * 100.0).round() as i32;
    format!("{pct}%  of  {duration_text}")
}

fn format_remaining_ms(remaining_ms: i64) -> String {
    if remaining_ms <= 0 {
        return "Finished".to_owned();
    }
    let total_minutes = remaining_ms / 60_000;
    let h = total_minutes / 60;
    let m = total_minutes % 60;
    if h > 0 {
        format!("{h}h {m:02}m left")
    } else {
        format!("{m}m left")
    }
}
