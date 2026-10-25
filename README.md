# Hien

A fast notepad for Windows, written in Rust with egui. Notes are plain `.txt` files in `%APPDATA%\Hien\notes` and save on their own 400 ms after you stop typing.

[Download](https://github.com/misutesu-desu/hien/releases/latest) · [Website](https://misutesu-desu.github.io/hien/) · [Türkçe](README.tr.md)

## Features

- Instant search across notes. The first line of a note is its name.
- Find and replace, one at a time or all at once, with match case and whole word options.
- Insert word: puts another word before or after every match. With `cat` and `tabby`, every `cat` becomes `tabby cat`. A match that already has the word is skipped, and the space between the two is optional.
- Bulk changes undo with one button.
- Deleted notes go to a `trash` folder and can be restored from the status bar.
- English and Turkish interface, picked from the system language and changeable in Settings.
- Dark, light and system theme.
- Closing the window keeps Hien running in the background, so the next file opens at once. Settings has Quit Hien (Ctrl+Q), an option to turn this off, and Start with Windows, which starts Hien hidden when you sign in.
- Opens `.txt` files. Set Hien as the default app for `.txt` and double-clicking a file opens it in Hien. The file is saved back in place with its line endings, and Close file takes it off the list without touching it. Files must be UTF-8. If Hien is already open, the file opens in that window.

## Shortcuts

| Key | Action |
|---|---|
| Ctrl+N | New note |
| Ctrl+F | Find |
| Ctrl+H | Replace |
| Ctrl+E | Insert word |
| F3, Shift+F3 | Next, previous match |
| Enter | Runs the action of the focused panel row |
| Ctrl+K | Search notes |
| Alt+Up, Alt+Down | Previous, next note |
| Ctrl+S | Save now |
| Ctrl+Q | Quit Hien |
| Esc | Close the panel |
| Middle click | Scroll by moving the mouse, click again to stop. Hold the wheel down to scroll only while held. |

Text selected in the editor fills the find box when the panel opens.

## Building

```bash
cargo build --release
```

The binary is `target/release/hien.exe`. Run the tests with `cargo test`.
