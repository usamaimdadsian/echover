//! Vector icons baked into the glyph atlas at startup. Each `IconId` is
//! hand-drawn against a 24×24 viewbox using `tiny-skia`, then rasterized at
//! the few pixel sizes the UI actually uses. Once baked, an icon costs the
//! same as one glyph: a UV lookup and a single quad in the draw list.

use tiny_skia::{
    LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

/// Pre-rasterized icon sizes. Match what nav rows / transport / chips use.
pub const ICON_SIZES: &[u8] = &[16, 18, 20, 24, 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconId {
    Home,
    Library,
    Player,
    Bookmarks,
    Settings,
    Play,
    Pause,
    Rewind,
    Forward,
    Search,
    Heart,
    ArrowLeft,
    FolderPlus,
    Plus,
    Minus,
}

pub const ALL_ICONS: &[IconId] = &[
    IconId::Home,
    IconId::Library,
    IconId::Player,
    IconId::Bookmarks,
    IconId::Settings,
    IconId::Play,
    IconId::Pause,
    IconId::Rewind,
    IconId::Forward,
    IconId::Search,
    IconId::Heart,
    IconId::ArrowLeft,
    IconId::FolderPlus,
    IconId::Plus,
    IconId::Minus,
];

/// Rasterize `id` to an R8 (alpha-only) bitmap at `size_px × size_px`. The
/// returned vec is row-major, `size_px * size_px` bytes long. The caller
/// uploads it into the atlas the same way it uploads font glyph bitmaps.
pub fn rasterize(id: IconId, size_px: u8) -> Vec<u8> {
    let size = size_px as u32;
    let mut pixmap = Pixmap::new(size, size).expect("non-zero icon size");

    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;

    let scale = size as f32 / 24.0;
    let xform = Transform::from_scale(scale, scale);
    // Stroke width tuned by hand: stays visible at 16px, doesn't blob at 32px.
    let stroke_width = (1.6_f32).max(2.0 / scale.max(0.6));
    let stroke = Stroke {
        width: stroke_width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };

    match id {
        IconId::Home => {
            // House silhouette + interior door cut-out (Lucide-style).
            let mut pb = PathBuilder::new();
            pb.move_to(3.0, 11.0);
            pb.line_to(12.0, 3.0);
            pb.line_to(21.0, 11.0);
            pb.line_to(21.0, 21.0);
            pb.line_to(3.0, 21.0);
            pb.close();
            let outline = pb.finish().unwrap();
            pixmap.stroke_path(&outline, &paint, &stroke, xform, None);

            let mut pb = PathBuilder::new();
            pb.move_to(9.5, 21.0);
            pb.line_to(9.5, 13.5);
            pb.line_to(14.5, 13.5);
            pb.line_to(14.5, 21.0);
            let door = pb.finish().unwrap();
            pixmap.stroke_path(&door, &paint, &stroke, xform, None);
        }
        IconId::Library => {
            // Three vertical book spines.
            for x in [4.0_f32, 9.5, 15.0] {
                let mut pb = PathBuilder::new();
                pb.move_to(x, 4.0);
                pb.line_to(x + 4.0, 4.0);
                pb.line_to(x + 4.0, 20.0);
                pb.line_to(x, 20.0);
                pb.close();
                let path = pb.finish().unwrap();
                pixmap.stroke_path(&path, &paint, &stroke, xform, None);
            }
        }
        IconId::Player => {
            // Headphones: arch + two earcups.
            let mut pb = PathBuilder::new();
            pb.move_to(4.0, 14.0);
            pb.cubic_to(4.0, 6.0, 20.0, 6.0, 20.0, 14.0);
            let arch = pb.finish().unwrap();
            pixmap.stroke_path(&arch, &paint, &stroke, xform, None);

            for cx in [5.5_f32, 18.5] {
                let mut pb = PathBuilder::new();
                pb.move_to(cx - 2.0, 14.0);
                pb.line_to(cx + 2.0, 14.0);
                pb.line_to(cx + 2.0, 19.5);
                pb.line_to(cx - 2.0, 19.5);
                pb.close();
                let cup = pb.finish().unwrap();
                pixmap.fill_path(
                    &cup,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    xform,
                    None,
                );
            }
        }
        IconId::Bookmarks => {
            // Pennant ribbon.
            let mut pb = PathBuilder::new();
            pb.move_to(6.0, 3.0);
            pb.line_to(18.0, 3.0);
            pb.line_to(18.0, 21.0);
            pb.line_to(12.0, 16.0);
            pb.line_to(6.0, 21.0);
            pb.close();
            let path = pb.finish().unwrap();
            pixmap.stroke_path(&path, &paint, &stroke, xform, None);
        }
        IconId::Settings => {
            // Three horizontal sliders with a knob each — simpler than a gear
            // and still reads as "settings" at small sizes.
            for (y, kx) in [(6.0_f32, 16.0), (12.0, 8.0), (18.0, 17.0)] {
                let mut pb = PathBuilder::new();
                pb.move_to(3.0, y);
                pb.line_to(21.0, y);
                let line = pb.finish().unwrap();
                pixmap.stroke_path(&line, &paint, &stroke, xform, None);
                let mut pb = PathBuilder::new();
                pb.push_circle(kx, y, 2.2);
                let knob = pb.finish().unwrap();
                pixmap.fill_path(
                    &knob,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    xform,
                    None,
                );
            }
        }
        IconId::Play => {
            // Filled triangle.
            let mut pb = PathBuilder::new();
            pb.move_to(7.0, 4.5);
            pb.line_to(20.0, 12.0);
            pb.line_to(7.0, 19.5);
            pb.close();
            let path = pb.finish().unwrap();
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, xform, None);
        }
        IconId::Pause => {
            for x in [7.0_f32, 14.0] {
                let mut pb = PathBuilder::new();
                pb.move_to(x, 5.0);
                pb.line_to(x + 3.0, 5.0);
                pb.line_to(x + 3.0, 19.0);
                pb.line_to(x, 19.0);
                pb.close();
                let bar = pb.finish().unwrap();
                pixmap.fill_path(
                    &bar,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    xform,
                    None,
                );
            }
        }
        IconId::Rewind => {
            // Two left-pointing triangles.
            for ox in [12.5_f32, 5.5] {
                let mut pb = PathBuilder::new();
                pb.move_to(ox + 6.0, 6.0);
                pb.line_to(ox, 12.0);
                pb.line_to(ox + 6.0, 18.0);
                pb.close();
                let path = pb.finish().unwrap();
                pixmap.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    xform,
                    None,
                );
            }
        }
        IconId::Forward => {
            for ox in [5.5_f32, 12.5] {
                let mut pb = PathBuilder::new();
                pb.move_to(ox, 6.0);
                pb.line_to(ox + 6.0, 12.0);
                pb.line_to(ox, 18.0);
                pb.close();
                let path = pb.finish().unwrap();
                pixmap.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    xform,
                    None,
                );
            }
        }
        IconId::Search => {
            // Lens + handle.
            let mut pb = PathBuilder::new();
            pb.push_circle(11.0, 11.0, 6.5);
            let lens = pb.finish().unwrap();
            pixmap.stroke_path(&lens, &paint, &stroke, xform, None);

            let mut pb = PathBuilder::new();
            pb.move_to(16.0, 16.0);
            pb.line_to(20.5, 20.5);
            let handle = pb.finish().unwrap();
            pixmap.stroke_path(&handle, &paint, &stroke, xform, None);
        }
        IconId::Heart => {
            // Two arcs joined at the bottom point. Cubic curves keep it round.
            let mut pb = PathBuilder::new();
            pb.move_to(12.0, 20.5);
            pb.cubic_to(2.0, 13.0, 4.5, 4.0, 12.0, 8.5);
            pb.cubic_to(19.5, 4.0, 22.0, 13.0, 12.0, 20.5);
            pb.close();
            let path = pb.finish().unwrap();
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, xform, None);
        }
        IconId::ArrowLeft => {
            let mut pb = PathBuilder::new();
            pb.move_to(20.0, 12.0);
            pb.line_to(4.0, 12.0);
            let shaft = pb.finish().unwrap();
            pixmap.stroke_path(&shaft, &paint, &stroke, xform, None);

            let mut pb = PathBuilder::new();
            pb.move_to(10.0, 6.0);
            pb.line_to(4.0, 12.0);
            pb.line_to(10.0, 18.0);
            let head = pb.finish().unwrap();
            pixmap.stroke_path(&head, &paint, &stroke, xform, None);
        }
        IconId::FolderPlus => {
            // Folder outline + plus glyph.
            let mut pb = PathBuilder::new();
            pb.move_to(3.0, 7.0);
            pb.line_to(10.0, 7.0);
            pb.line_to(12.0, 9.0);
            pb.line_to(21.0, 9.0);
            pb.line_to(21.0, 19.0);
            pb.line_to(3.0, 19.0);
            pb.close();
            let folder = pb.finish().unwrap();
            pixmap.stroke_path(&folder, &paint, &stroke, xform, None);

            let mut pb = PathBuilder::new();
            pb.move_to(12.0, 11.5);
            pb.line_to(12.0, 16.5);
            let v = pb.finish().unwrap();
            pixmap.stroke_path(&v, &paint, &stroke, xform, None);

            let mut pb = PathBuilder::new();
            pb.move_to(9.5, 14.0);
            pb.line_to(14.5, 14.0);
            let h = pb.finish().unwrap();
            pixmap.stroke_path(&h, &paint, &stroke, xform, None);
        }
        IconId::Plus => {
            let mut pb = PathBuilder::new();
            pb.move_to(12.0, 5.0);
            pb.line_to(12.0, 19.0);
            let v = pb.finish().unwrap();
            pixmap.stroke_path(&v, &paint, &stroke, xform, None);
            let mut pb = PathBuilder::new();
            pb.move_to(5.0, 12.0);
            pb.line_to(19.0, 12.0);
            let h = pb.finish().unwrap();
            pixmap.stroke_path(&h, &paint, &stroke, xform, None);
        }
        IconId::Minus => {
            let mut pb = PathBuilder::new();
            pb.move_to(5.0, 12.0);
            pb.line_to(19.0, 12.0);
            let path = pb.finish().unwrap();
            pixmap.stroke_path(&path, &paint, &stroke, xform, None);
        }
    }

    // tiny-skia gives us premultiplied RGBA. The icon is white, so the alpha
    // channel already encodes the silhouette — that's all the atlas needs.
    pixmap.data().chunks(4).map(|px| px[3]).collect()
}
