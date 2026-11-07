# Building Instructions

```
cargo build
```

## GUI tests

The interface is tested with [egui_kittest](https://docs.rs/egui_kittest) in `src/app/gui_kittest.rs`. The tests build `VirtualBookApp` in an off-screen harness. They do not open a desktop window. Clicks go through the accessibility tree, and two tests compare a rendered frame with the images in `tests/snapshots/`.

Run every GUI test:

```
cargo test --lib gui_kittest
```

Run one test:

```
cargo test --lib -- --exact app::gui_kittest::playlist_numpad_appends_the_typed_number
```

The playlist tests install an idle player, so they do not need a MIDI output. The app is forced to English, at zoom 1.5.

The snapshot tests (`snapshot_startup_choose_output` and `snapshot_empty_playlist`) render with wgpu, so a GPU or a software Vulkan adapter has to be available. After a real visual change, write new reference images with:

```
UPDATE_SNAPSHOTS=true cargo test --lib gui_kittest
```

`UPDATE_SNAPSHOTS=true` replaces only the images that failed. `UPDATE_SNAPSHOTS=force` rewrites every snapshot. A mismatch leaves `tests/snapshots/<name>.new.png` and `tests/snapshots/<name>.diff.png` beside the committed `<name>.png`. Those two extra files are gitignored.
