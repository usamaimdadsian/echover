use crate::ui::action::LibraryFilter;
use crate::ui::data::DisplayBook;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use crate::ui::widgets::{book_card::book_card, filter_chip::filter_chip, search_bar::search_bar};

const SEARCH_HEIGHT: f32 = 48.0;
const FILTER_HEIGHT: f32 = 32.0;

pub fn layout(
    rect: Rect,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let mut cursor_y = rect.y;

    draw.text(
        rect.x,
        cursor_y,
        theme.text_title,
        theme.text,
        "Library",
        font,
    );
    cursor_y += font.line_height(theme.text_title) + theme.spacing_medium;

    let search = Rect::new(rect.x, cursor_y, rect.width, SEARCH_HEIGHT);
    search_bar(
        search,
        "Search by title, author, or narrator   (Ctrl+F)",
        &state.search_query,
        state.search_focused,
        theme,
        font,
        interaction,
        draw,
        hits,
    );
    cursor_y += SEARCH_HEIGHT + theme.spacing_large;

    let filters: &[(LibraryFilter, &str)] = &[
        (LibraryFilter::All, "All"),
        (LibraryFilter::InProgress, "In progress"),
        (LibraryFilter::NotStarted, "Not started"),
        (LibraryFilter::Finished, "Finished"),
    ];
    let mut x = rect.x;
    let chip_padding = 28.0;
    for &(filter, label) in filters {
        let label_w = font.measure(label, theme.text_button);
        let chip_w = label_w + chip_padding;
        let chip = Rect::new(x, cursor_y, chip_w, FILTER_HEIGHT);
        let active = state.library_filter == filter;
        filter_chip(
            chip,
            label,
            filter,
            active,
            theme,
            font,
            interaction,
            draw,
            hits,
        );
        x += chip_w + theme.spacing_small;
    }
    cursor_y += FILTER_HEIGHT + theme.spacing_large;

    let grid_h = (rect.y + rect.height - cursor_y).max(0.0);
    let books: Vec<&DisplayBook> = state
        .library
        .books
        .iter()
        .filter(|b| matches_filter(b, state.library_filter))
        .filter(|b| matches_query(b, &state.search_query))
        .collect();

    if books.is_empty() {
        let empty = if state.library.books.is_empty() {
            "Your library is empty. Set ECHOVER_LIBRARY_PATH and restart."
        } else {
            "No books match your filters."
        };
        draw.text(
            rect.x,
            cursor_y,
            theme.text_body,
            theme.muted_text,
            empty,
            font,
        );
    } else {
        book_grid(
            rect.x,
            cursor_y,
            rect.width,
            grid_h,
            4,
            &books,
            theme,
            font,
            interaction,
            draw,
            hits,
        );
    }
}

fn matches_filter(book: &DisplayBook, filter: LibraryFilter) -> bool {
    match filter {
        LibraryFilter::All => true,
        LibraryFilter::InProgress => !book.completed && book.progress > 0.0 && book.progress < 1.0,
        LibraryFilter::NotStarted => !book.completed && book.progress == 0.0,
        LibraryFilter::Finished => book.completed || book.progress >= 1.0,
    }
}

fn matches_query(book: &DisplayBook, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let needle = query.to_lowercase();
    book.title.to_lowercase().contains(&needle)
        || book.author.to_lowercase().contains(&needle)
        || book.narrator.to_lowercase().contains(&needle)
}

#[allow(clippy::too_many_arguments)]
fn book_grid(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    cols: usize,
    books: &[&DisplayBook],
    theme: &Theme,
    font: &Font,
    interaction: &Interaction,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    if width <= 0.0 || height <= 0.0 || cols == 0 {
        return;
    }
    let gap = theme.spacing_medium;
    let card_w = ((width - gap * (cols as f32 - 1.0)) / cols as f32).max(0.0);
    let card_h = 260.0_f32.min((height - gap) * 0.5).max(160.0);

    for (idx, book) in books.iter().enumerate() {
        let r = idx / cols;
        let c = idx % cols;
        let cx = x + c as f32 * (card_w + gap);
        let cy = y + r as f32 * (card_h + gap);
        if cy + card_h > y + height {
            break;
        }
        book_card(
            Rect::new(cx, cy, card_w, card_h),
            theme,
            font,
            interaction,
            draw,
            hits,
            book,
        );
    }
}
