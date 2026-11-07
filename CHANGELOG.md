# Changelog

### 2026-11-07: v0.0.60
- draw the book on the GPU, and keep the previous rectangle drawing when effects are off or wgpu is unavailable
- book effects in preferences: note fire, sparkle, ripple, neon lane, arcade burst, with an intensity control
- pause and resume the current tune
- history view beside the catalog: recently played, and a ranking by stars, play count, or both
- a plus button to add a file to the playlist from the catalog and from history
- the same file can sit on the playlist more than once
- the play screen shows the name of the next file
- the roll follows playback time directly, so a late MIDI clock no longer pulls the view the wrong way
- if the saved MIDI device is gone, fall back to the first available output
- move to egui 0.37 and Rust 1.95

### 2025-11-23: v0.0.59
- local play metadata: play counts and stars, kept across sessions
- modernized interface, default colors, and the application logo
- snappier book display

### 2024-04 to 2024-10: v0.0.50 – v0.0.56
- playlist total duration, automatic return to the roll view, and clearer icons
- French and English interface strings
- scrollable panes (egui 0.27)
- touch-friendly controls and a realtime view
- playback latency setting
- paper texture on the book, and more precise hole positions
- build documentation
- faster startup

### 2024-03: v0.0.48
- fix on book reading (negative hole entries)

### 2023-10: v0.0.42
- book file support, with midi translation for control systems
- performance improvments (down to 15% of cpu usage instead of 30%)
- user experience improvments (refresh folders, .... )

### 2023-09-23: 
- add icons on buttons, simplify the gui


### 2023-09-22: 
- add book widgets
- add escape/space shortcut
- add play button when not initialized
- add title next to it
- rework the playlist list
- add tooltips
