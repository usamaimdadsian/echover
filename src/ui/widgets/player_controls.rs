use crate::ui::action::UiAction;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::icons::IconId;
use crate::ui::primitives::DrawList;
use crate::ui::theme::Theme;

const SECONDARY_SIZE: f32 = 44.0;
const PRIMARY_SIZE: f32 = 64.0;
const GAP: f32 = 24.0;

#[allow(clippy::too_many_arguments)]
pub fn transport(
    center_x: f32,
    top_y: f32,
    is_playing: bool,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) -> Rect {
    let total_w = SECONDARY_SIZE + GAP + PRIMARY_SIZE + GAP + SECONDARY_SIZE;
    let left = center_x - total_w * 0.5;

    let rewind = Rect::new(
        left,
        top_y + (PRIMARY_SIZE - SECONDARY_SIZE) * 0.5,
        SECONDARY_SIZE,
        SECONDARY_SIZE,
    );
    icon_secondary(
        rewind,
        IconId::Rewind,
        "15",
        UiAction::SeekBackward,
        theme,
        font,
        interaction,
        draw,
        hits,
    );

    let play_rect = Rect::new(left + SECONDARY_SIZE + GAP, top_y, PRIMARY_SIZE, PRIMARY_SIZE);
    icon_primary(
        play_rect,
        if is_playing { IconId::Pause } else { IconId::Play },
        UiAction::PlayPause,
        theme,
        font,
        interaction,
        draw,
        hits,
    );

    let forward = Rect::new(
        left + SECONDARY_SIZE + GAP + PRIMARY_SIZE + GAP,
        top_y + (PRIMARY_SIZE - SECONDARY_SIZE) * 0.5,
        SECONDARY_SIZE,
        SECONDARY_SIZE,
    );
    icon_secondary(
        forward,
        IconId::Forward,
        "30",
        UiAction::SeekForward,
        theme,
        font,
        interaction,
        draw,
        hits,
    );

    Rect::new(left, top_y, total_w, PRIMARY_SIZE)
}

#[allow(clippy::too_many_arguments)]
fn icon_primary(
    rect: Rect,
    icon: IconId,
    action: UiAction,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let bg = interaction.shade(action, theme.accent, theme);
    draw.rounded_rect(rect, rect.height * 0.5, bg);
    let icon_size = 28.0;
    let icon_rect = Rect::new(
        rect.x + (rect.width - icon_size) * 0.5,
        rect.y + (rect.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    draw.icon(icon_rect, theme.panel, font, icon);
    hits.push(HitRegion { rect, action });
}

#[allow(clippy::too_many_arguments)]
fn icon_secondary(
    rect: Rect,
    icon: IconId,
    seconds_label: &str,
    action: UiAction,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let bg = interaction.shade(action, theme.track, theme);
    draw.rounded_rect(rect, rect.height * 0.5, bg);

    // Icon top-half, "15" / "30" caption below — matches the design spec for
    // skip-by-time buttons.
    let icon_size = 16.0;
    let icon_rect = Rect::new(
        rect.x + (rect.width - icon_size) * 0.5,
        rect.y + 6.0,
        icon_size,
        icon_size,
    );
    draw.icon(icon_rect, theme.text, font, icon);

    let label_w = font.measure(seconds_label, theme.text_eyebrow);
    draw.text(
        rect.x + (rect.width - label_w) * 0.5,
        rect.y + rect.height - font.line_height(theme.text_eyebrow) - 4.0,
        theme.text_eyebrow,
        theme.text,
        seconds_label,
        font,
    );

    hits.push(HitRegion { rect, action });
}

pub fn bookmark_button(
    center_x: f32,
    center_y: f32,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) -> Rect {
    let size = 36.0;
    let rect = Rect::new(center_x - size * 0.5, center_y - size * 0.5, size, size);
    let action = UiAction::AddBookmark;
    let bg = interaction.shade(action, theme.track, theme);
    draw.rounded_rect(rect, size * 0.5, bg);
    let icon_size = 18.0;
    let icon_rect = Rect::new(
        rect.x + (rect.width - icon_size) * 0.5,
        rect.y + (rect.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    draw.icon(icon_rect, theme.accent, font, IconId::Heart);
    hits.push(HitRegion { rect, action });
    rect
}
