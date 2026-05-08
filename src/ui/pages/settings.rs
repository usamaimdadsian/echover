use crate::ui::action::UiAction;
use crate::ui::data::DisplayFolder;
use crate::ui::font::Font;
use crate::ui::geometry::Rect;
use crate::ui::hit::{HitRegion, Interaction};
use crate::ui::icons::IconId;
use crate::ui::primitives::DrawList;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;

const SECTION_GAP: f32 = 32.0;
const ROW_HEIGHT: f32 = 64.0;

pub fn layout(
    rect: Rect,
    theme: &Theme,
    state: &AppState,
    interaction: &Interaction,
    font: &Font,
    draw: &mut DrawList,
    hits: &mut Vec<HitRegion>,
) {
    let mut y = rect.y;

    draw.text(rect.x, y, theme.text_title, theme.text, "Settings", font);
    y += font.line_height(theme.text_title);
    draw.text(
        rect.x,
        y,
        theme.text_subtitle,
        theme.muted_text,
        "Configure where your audiobooks live and how they play back.",
        font,
    );
    y += font.line_height(theme.text_subtitle) + SECTION_GAP;

    // Header row: section title on the left, "Add folder" button on the right.
    section_heading(rect.x, y, "Library folders", theme, font, draw);
    let add_action = UiAction::AddLibraryFolder;
    let add_label = "Add folder";
    let icon_size = 16.0;
    let label_w = font.measure(add_label, theme.text_button);
    let pad = 14.0;
    let add_w = icon_size + 8.0 + label_w + pad * 2.0;
    let add_h = 32.0;
    let add_btn = Rect::new(rect.x + rect.width - add_w, y - 4.0, add_w, add_h);
    let add_bg = interaction.shade(add_action, theme.accent, theme);
    draw.rounded_rect(add_btn, add_h * 0.5, add_bg);
    let icon_x = add_btn.x + pad;
    let icon_y = add_btn.y + (add_btn.height - icon_size) * 0.5;
    draw.icon(
        Rect::new(icon_x, icon_y, icon_size, icon_size),
        theme.panel,
        font,
        IconId::FolderPlus,
    );
    draw.text(
        icon_x + icon_size + 8.0,
        add_btn.y + (add_btn.height - font.line_height(theme.text_button)) * 0.5,
        theme.text_button,
        theme.panel,
        add_label,
        font,
    );
    hits.push(HitRegion {
        rect: add_btn,
        action: add_action,
    });

    y += font.line_height(theme.text_section) + theme.spacing_small;

    let folders = &state.library.folders;
    if folders.is_empty() {
        draw.text(
            rect.x,
            y,
            theme.text_body,
            theme.muted_text,
            "No folders registered. Set ECHOVER_LIBRARY_PATH and restart.",
            font,
        );
        y += font.line_height(theme.text_body) + SECTION_GAP;
    } else {
        let folders_panel = Rect::new(rect.x, y, rect.width, ROW_HEIGHT * folders.len() as f32);
        draw.rounded_rect(folders_panel, theme.radius_medium, theme.panel);
        for (i, folder) in folders.iter().enumerate() {
            folder_row(
                folders_panel.x,
                folders_panel.y + i as f32 * ROW_HEIGHT,
                folders_panel.width,
                folder,
                theme,
                font,
                draw,
            );
        }
        y += folders_panel.height + SECTION_GAP;
    }

    section_heading(rect.x, y, "Playback", theme, font, draw);
    y += font.line_height(theme.text_section) + theme.spacing_small;
    let playback_panel = Rect::new(rect.x, y, rect.width, ROW_HEIGHT * 3.0);
    draw.rounded_rect(playback_panel, theme.radius_medium, theme.panel);

    setting_row(
        playback_panel.x,
        playback_panel.y,
        playback_panel.width,
        "Default playback speed",
        ValueControl::Slider("1.0×", 0.5),
        theme,
        font,
        draw,
    );
    setting_row(
        playback_panel.x,
        playback_panel.y + ROW_HEIGHT,
        playback_panel.width,
        "Smart resume rewind",
        ValueControl::Stepper("5 seconds"),
        theme,
        font,
        draw,
    );
    setting_row(
        playback_panel.x,
        playback_panel.y + ROW_HEIGHT * 2.0,
        playback_panel.width,
        "Forward seek",
        ValueControl::Stepper("30 seconds"),
        theme,
        font,
        draw,
    );
}

fn section_heading(x: f32, y: f32, label: &str, theme: &Theme, font: &Font, draw: &mut DrawList) {
    draw.text(x, y, theme.text_section, theme.text, label, font);
}

fn folder_row(
    x: f32,
    y: f32,
    width: f32,
    folder: &DisplayFolder,
    theme: &Theme,
    font: &Font,
    draw: &mut DrawList,
) {
    let row = Rect::new(x, y, width, ROW_HEIGHT);
    let icon_size = 22.0;
    draw.icon(
        Rect::new(
            row.x + theme.spacing_medium,
            row.y + (ROW_HEIGHT - icon_size) * 0.5,
            icon_size,
            icon_size,
        ),
        theme.accent,
        font,
        IconId::FolderPlus,
    );

    let text_x = row.x + theme.spacing_medium + 48.0;
    let label_y = row.y + (ROW_HEIGHT - font.line_height(theme.text_body) * 2.0) * 0.5;
    draw.text(
        text_x,
        label_y,
        theme.text_body,
        theme.text,
        &folder.label,
        font,
    );
    let detail = format!("{}  ·  {} books", folder.path, folder.book_count);
    draw.text(
        text_x,
        label_y + font.line_height(theme.text_body),
        theme.text_caption,
        theme.muted_text,
        &detail,
        font,
    );

    let remove = Rect::new(
        row.x + row.width - theme.spacing_medium - 80.0,
        row.y + (ROW_HEIGHT - 32.0) * 0.5,
        80.0,
        32.0,
    );
    draw.rounded_rect(remove, theme.radius_small, theme.background);
    let label = "Remove";
    let lw = font.measure(label, theme.text_caption);
    draw.text(
        remove.x + (remove.width - lw) * 0.5,
        remove.y + (remove.height - font.line_height(theme.text_caption)) * 0.5,
        theme.text_caption,
        theme.text,
        label,
        font,
    );
}

enum ValueControl {
    Slider(&'static str, f32),
    Stepper(&'static str),
}

#[allow(clippy::too_many_arguments)]
fn setting_row(
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    control: ValueControl,
    theme: &Theme,
    font: &Font,
    draw: &mut DrawList,
) {
    let row = Rect::new(x, y, width, ROW_HEIGHT);
    let label_y = row.y + (ROW_HEIGHT - font.line_height(theme.text_body)) * 0.5;
    draw.text(
        row.x + theme.spacing_medium,
        label_y,
        theme.text_body,
        theme.text,
        label,
        font,
    );

    match control {
        ValueControl::Slider(value_label, fill) => {
            let track = Rect::new(
                row.x + row.width * 0.45,
                row.y + (ROW_HEIGHT - 6.0) * 0.5,
                row.width * 0.40,
                6.0,
            );
            draw.progress_bar(track, fill, theme);
            let knob_x = track.x + track.width * fill - 8.0;
            draw.rounded_rect(
                Rect::new(knob_x, track.y - 7.0, 16.0, 20.0),
                8.0,
                theme.accent,
            );
            let value_w = font.measure(value_label, theme.text_body);
            draw.text(
                row.x + row.width - theme.spacing_medium - value_w,
                label_y,
                theme.text_body,
                theme.muted_text,
                value_label,
                font,
            );
        }
        ValueControl::Stepper(value_label) => {
            let stepper = Rect::new(
                row.x + row.width - theme.spacing_medium - 180.0,
                row.y + (ROW_HEIGHT - 36.0) * 0.5,
                180.0,
                36.0,
            );
            draw.rounded_rect(stepper, theme.radius_small, theme.background);

            let icon_size = 16.0;
            draw.icon(
                Rect::new(
                    stepper.x + 12.0,
                    stepper.y + (stepper.height - icon_size) * 0.5,
                    icon_size,
                    icon_size,
                ),
                theme.text,
                font,
                IconId::Minus,
            );
            draw.icon(
                Rect::new(
                    stepper.x + stepper.width - 12.0 - icon_size,
                    stepper.y + (stepper.height - icon_size) * 0.5,
                    icon_size,
                    icon_size,
                ),
                theme.text,
                font,
                IconId::Plus,
            );

            let vw = font.measure(value_label, theme.text_body);
            draw.text(
                stepper.x + (stepper.width - vw) * 0.5,
                stepper.y + (stepper.height - font.line_height(theme.text_body)) * 0.5,
                theme.text_body,
                theme.text,
                value_label,
                font,
            );
        }
    }
}
