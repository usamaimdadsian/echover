# Echover

A small, custom-rendered audiobook player for the desktop, written in Rust on
top of **Vulkan via `ash`**, **`winit`**, **`fontdue`**, **`tiny-skia`**, and
**`rodio`**, backed by **`rusqlite`** for the library and playback state.

The UI is hand-drawn — every panel, button and icon is a quad submitted to a
single graphics pipeline that branches in the fragment shader between an SDF
rounded-rect and a sampled glyph. There's no widget toolkit; the whole
interface fits in a couple hundred lines of layout code per page. The visual
direction comes from `design/design.html`.

## Quick start

```bash
cargo run            # debug build + launch
cargo run --release  # release build, smoother frame pacing
cargo check          # fast type-check
cargo test           # unit + integration tests
cargo clippy --all-targets
cargo fmt
```

By default Echover scans `./library/` (gitignored) for audiobooks on first
launch. To point at a different folder:

```bash
ECHOVER_LIBRARY_PATH=/path/to/audiobooks cargo run
```

You can also click **Add audiobook** in the sidebar (or the matching pill on
the Settings page) at any time — that opens a native folder picker, scans
the chosen directory, and re-hydrates the library in place without a
restart. Re-scanning a folder you've already imported is idempotent: the
audiobook + file rows dedupe by path.

The SQLite database lives at `data/echover.sqlite3` (gitignored). Delete it
to force a fresh schema + mock seed on the next run.

## Features

- **Vulkan** rendering via `ash` — single in-flight frame, FIFO swapchain,
  one render pass, one pipeline, persistently mapped vertex buffer.
- **Atlas-baked glyphs and icons** — fontdue rasterizes the UI font at
  every theme size; tiny-skia renders 15 vector icons (sidebar nav,
  transport, search, heart, etc.) into the same R8 atlas at startup.
- **Pages**: Home (Continue Listening + Recently played), Library (search
  + filter chips + grid), Player (cover, transport, live progress),
  Bookmarks, Settings (folders + playback controls), Book Detail (chapter
  list).
- **Hover / pressed visuals** — every clickable surface tints by 8 % / 18 %
  toward `theme.text`. Clicks fire on release if the cursor stays on the
  same hit region.
- **Keyboard**: Space play/pause, ←/→ seek 15/30 s, B add bookmark,
  Ctrl/Cmd+F focus search, Esc clear search or exit.
- **Smart-resume rewind** (5 s) when restoring a saved position on
  startup.
- **Persistence** — playback position is written on play/pause, every seek,
  and on exit; close and reopen lands you back where you stopped.
- **Bookmarks** — the `B` key (or the heart on the Player page) records
  the current position; the Bookmarks page shows them across all books and
  jumping to one seeks playback there.
- **Chapters** — when an audiobook is split across multiple audio files,
  each file is a chapter row on Book Detail; clicking one loads + plays
  that file.

## Architecture (10-second tour)

- **`src/window/`** — the *only* module that touches `ash`. Owns the
  Vulkan instance, surface, device, swapchain, render pass, framebuffers,
  pipeline, descriptor set + atlas image, and the per-frame
  `record → submit → present` cycle. Everything above sees a tiny
  renderer interface and a `DrawList` of `DrawCommand`s.
- **`src/ui/`** — custom UI layer.
  - `state::AppState` is the single source of truth (page, library
    snapshot, search, hover/press, playback position).
  - `action::UiAction` is the unified mouse + keyboard event vocabulary.
  - `hit::HitRegion { id, rect, action }` is registered during draw;
    clicks resolve against this list.
  - `data::Library` is a read-only snapshot of the DB hydrated at startup
    (and rebuilt after a folder scan).
  - `font` + `icons` build the R8 atlas; `primitives::DrawList` issues
    `rounded_rect`, `text`, `icon`, `progress_bar`.
  - `pages/` and `widgets/` are pure layout + draw — no toolkit.
- **`src/domain/`** — plain data types (`Audiobook`, `AudiobookFile`,
  `Bookmark`, `LibraryFolder`, `PlaybackState`). No I/O.
- **`src/persistence/db.rs`** — SQLite via `rusqlite` (bundled). Schema
  init, ingest, mock seed, load helpers used at startup and after each
  scan.
- **`src/library/scanner.rs`** — `walkdir`-based recursive folder scan
  that groups audio files by their immediate parent directory.
- **`src/playback/`** — `engine::PlaybackEngine` trait + a `rodio`
  backend behind it. Keeping playback behind the trait means the backend
  is swappable.

## Project layout

```
echover/
├── src/
│   ├── main.rs
│   ├── app.rs              # bootstrap: open DB, hydrate Library, run event loop
│   ├── window/
│   │   ├── event_loop.rs   # winit ApplicationHandler + dispatch_action
│   │   └── renderer.rs     # ash Vulkan renderer
│   ├── ui/
│   │   ├── shell.rs        # sidebar + page switch
│   │   ├── pages/          # home, library, player, bookmarks, settings, book_detail
│   │   ├── widgets/        # book_card, filter_chip, search_bar, player_controls
│   │   ├── icons.rs        # 15 vector icons drawn with tiny-skia
│   │   ├── font.rs         # fontdue glyph atlas + icon packing
│   │   ├── primitives.rs   # DrawList / DrawCommand / DrawKind
│   │   ├── data.rs         # Library snapshot (read-only view of the DB)
│   │   ├── state.rs        # AppState
│   │   ├── action.rs       # UiAction enum
│   │   ├── hit.rs          # HitRegion + Interaction (hover/press)
│   │   ├── geometry.rs     # Rect helpers
│   │   └── theme.rs        # cream/terracotta tokens + text scales
│   ├── domain/             # plain data types
│   ├── persistence/db.rs   # SQLite schema + queries
│   ├── library/scanner.rs  # filesystem scanner
│   └── playback/           # engine trait + rodio backend
├── shaders/
│   ├── ui.vert / ui.vert.spv
│   └── ui.frag / ui.frag.spv
├── design/design.html      # visual reference
├── Cargo.toml
└── data/                   # SQLite DB lives here (gitignored)
```

## Runtime requirements

- A Vulkan-capable GPU + the system Vulkan loader (`libvulkan.so.1` on
  Linux, MoltenVK on macOS).
- A UI font on disk. Echover looks for `assets/fonts/ui.ttf` first, then
  falls back to common system locations (DejaVu Sans, Liberation Sans,
  Noto Sans, Helvetica). Drop a `.ttf` at `assets/fonts/ui.ttf` to
  override.
- For the **Add audiobook** folder picker on Linux: GTK 3 (used by `rfd`
  with default features). On macOS and Windows the picker is native.

## Status

Phases 1–8 from the original plan are wired end-to-end:

| Phase | Scope                                | Status |
| ----- | ------------------------------------ | ------ |
| 1     | window + Vulkan renderer foundation  | ✅     |
| 2     | theme tokens, primitives, hit regions| ✅     |
| 3     | sidebar shell + page routing         | ✅     |
| 4     | pages with mock data                 | ✅     |
| 5     | hover/press, keyboard, search/filter | ✅     |
| 6     | SQLite + library scanning            | ✅     |
| 7     | playback MVP (rodio + persist)       | ✅     |
| 8     | bookmarks, chapters, recently played | ✅     |

Outstanding polish: scrolling for tall pages, real cover art, multi-file
auto-advance, configurable rewind from Settings.
