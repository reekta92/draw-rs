# draw-rs

Terminal drawing library and standalone application, extracted from
[clin-rs](https://github.com/reekta92/clin-rs), revision `1e5710f3`.
Copyright 2026 clin-rs contributors. Licensed GPL-3.0-only; see LICENSE.

The `.draw` v2 format preserves item IDs and transforms; v0/v1 documents
migrate in memory without rewriting on load. Saving legacy documents removes
unsupported legacy images, as clin already did.

## Standalone application

```sh
cargo install draw-rs --locked
draw sketch.draw
draw --help
```

The parent directory must already exist. Exactly one `.draw` path is required.
Missing files are created atomically; existing, malformed, oversized, unreadable,
or dangling-symlink paths never fall back to an empty drawing. A file created
before terminal initialization fails remains on disk.

Keys: `a` cursor, `d` draw, `s` shape, `t` text, `e` erase, `c` color,
`G` grid, `Ctrl+z` undo, `Ctrl+y` redo, `v` paste element, `?`/`F1` help.
Right-click opens an element menu; right/middle drag pans; scroll zooms.
`Enter` confirms editors/selectors. `Esc` cancels a transform, clears selection,
then leaves; `q` leaves. Finish or explicitly cancel an interaction before leaving.
Text editors consume printable `q`. Text copy/cut/paste uses textarea-local storage;
the binary does not access the system clipboard or load clin configuration.

## Embedding

The library does not initialize a terminal or access a system clipboard.
The host owns physical keys, toolbars, footer, document identity and clipboard
transport. `DrawAction` is semantic; there is no competing embedded keymap.
Use `handle_draw_mouse` for modal/canvas input and send `CopyText` to your host
clipboard. `insert_text` only pastes into an active editor. Text confirmation
stores the first textarea line, preserving clin's existing behavior.

```no_run
use draw::{DrawState, DrawAction, DrawHints, ThemeColors};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
# fn main() -> anyhow::Result<()> {
let mut drawing = DrawState::load(std::path::Path::new("sketch.draw"))?;
let mut clipboard = None;
draw::apply_action(&mut drawing, DrawAction::ToggleGrid, &mut clipboard)?;
let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
let canvas = Rect::new(0, 1, 80, 22);
drawing.set_canvas_area(canvas); // Also update before input after a resize.
terminal.draw(|frame| draw::draw_view(
    frame, &mut drawing, canvas, &ThemeColors::default(), &DrawHints::default(), None,
))?;
# Ok(())
# }
```

Modal routing: text cancel/confirm/input; selector direct actions; direct element
paste; context menu direct actions/fixed shortcuts; transient cancellation;
selected-item shortcuts; quit and other host-resolved actions. Consume unmatched
menu input. Mouse modals and captured drags take priority over host toolbar clicks.
Call `input_available()` and then `save()` before accepting ordinary departure.

## Persistence and compatibility

`DrawState::new()` is fileless. `load()` requires an existing regular UTF-8 file
and resolves its path; `create()` publishes a default document without replacing
a competing destination. Both input and serialized output are limited to 16 MiB.
Commits, undo and redo save eagerly and retain at most 20 history entries. A
failed write keeps live data and history; retry `save()` without reapplying the
operation. Changed fileless drawings return `Drawing has changes but no save target`.

Serialization validates IDs and transforms before publication. Atomic saves use
exclusive temporary files, preserve permissions, sync, rename, and sync the parent
directory on Unix. A failure after rename can mean data has already reached disk;
retain the state and report the error. Concurrent external edits are not detected
and may be overwritten. Forced process termination cannot recover unfinished
interaction state or unsaved changes following a failed save.

`state` and `geometry` retain the extracted names and JSON representation.
`draw_snapshot` paints committed content with existing fitting/pan/zoom behavior;
it excludes grid, selection, handles, and unfinished interactions.

## Development

CI, releases, and beta issue labeling call the reusable workflows in
[reekta92/.github](https://github.com/reekta92/.github). Releases publish the
`draw-rs` crate on crates.io and require the `CARGO_REGISTRY_TOKEN` repository secret.
Install the standalone binary with `cargo install draw-rs --locked`.
[GitHub Releases](https://github.com/reekta92/draw-rs/releases) include installation
instructions, Linux x86_64 and ARM64 CLI archives
(`draw-rs-<version>-<target>.tar.xz` / `.tar.gz`), the Cargo source archive, and
`SHA256SUMS`. Binary archives contain `draw`, `LICENSE`, and `README.md` at the
root; Linux builds require glibc 2.35 or newer. AUR maintainers can use the
versioned archives and hashes in their PKGBUILDs. The GitHub release is published
after crates.io succeeds.

Rust 1.90.0 or later, edition 2024. Dependencies use Ratatui 0.30, Crossterm 0.29
and ratatui-textarea 0.9, matching clin. No clin, graf or pinstar dependency.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
cargo +1.90.0 check --locked --all-targets
```

From a terminal, the optional unwind check is
`cargo test --bin draw terminal_guard_restores_after_panic -- --ignored`.
It verifies raw-mode and screen restoration during a caught panic.

See [the shared development process](https://github.com/reekta92/clin-rs/blob/main/docs/DEVELOPMENT.md)
for coordinated engine changes and release acceptance.
