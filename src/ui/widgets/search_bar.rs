use crate::ui::action::UiAction;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::icons::IconId;
use crate::ui::primitives::DrawList;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn search_bar(
    rect: Rect,
    placeholder: &str,
    query: &str,
    focused: bool,
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let base = if focused { theme.background } else { theme.panel };
    let bg = interaction.shade(UiAction::FocusSearch, base, theme);
    draw.rounded_rect(rect, theme.radius_large, bg);

    let icon_size = 18.0;
    let icon_rect = Rect::new(
        rect.x + theme.spacing_large,
        rect.y + (rect.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    draw.icon(icon_rect, theme.muted_text, font, IconId::Search);

    let text_x = rect.x + theme.spacing_large + icon_size + theme.spacing_small;
    let text_y = rect.y + (rect.height - font.line_height(theme.text_body)) * 0.5;
    let (text, color) = if query.is_empty() {
        (placeholder, theme.muted_text)
    } else {
        (query, theme.text)
    };
    let advanced = draw.text(text_x, text_y, theme.text_body, color, text, font);

    if focused {
        // Caret after the query text (or at the input origin if empty).
        let caret_x = if query.is_empty() {
            text_x
        } else {
            text_x + advanced + 1.0
        };
        let caret = Rect::new(
            caret_x,
            text_y + 2.0,
            2.0,
            font.line_height(theme.text_body) - 4.0,
        );
        draw.rounded_rect(caret, 1.0, theme.accent);
    }

    hits.push(HitRegion {
        rect,
        action: UiAction::FocusSearch,
    });
}
