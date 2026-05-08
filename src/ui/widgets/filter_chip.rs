use crate::ui::action::{LibraryFilter, UiAction};
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn filter_chip(
    rect: Rect,
    label: &str,
    filter: LibraryFilter,
    active: bool,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let action = UiAction::SetFilter(filter);
    let base = if active {
        theme.accent.with_alpha(0.18)
    } else {
        theme.panel
    };
    let bg = interaction.shade(action, base, theme);
    draw.rounded_rect(rect, rect.height * 0.5, bg);

    let label_color = if active { theme.accent } else { theme.muted_text };
    let text_w = font.measure(label, theme.text_button);
    let text_x = rect.x + (rect.width - text_w) * 0.5;
    let text_y = rect.y + (rect.height - font.line_height(theme.text_button)) * 0.5;
    draw.text(text_x, text_y, theme.text_button, label_color, label, font);

    hits.push(HitRegion { rect, action });
}
