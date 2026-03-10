# MyPhoto

> 📖 [中文版 → README.md](README.md)

A Tauri 2 + Vue 3 desktop photo manager for browsing, filtering, culling, and exporting.

## Recent Updates (2026-03)

- Fixed export command argument validation (`photo_ids`) and added a clear export progress popup with summary.
- Added conflict auto-rename rule options: custom prefix + suffix generator (`seq` / `date_seq` / `timestamp_seq`).
- Upgraded warmup UI to a bottom-right popup with unified total progress (initial + background stages).
- Unified warmup pipeline: one image read can generate both grid and cull/preview caches in one pass.
- Added adaptive first-run warmup defaults based on device capability (initial count + worker concurrency).
- Improved incremental sync for create/modify/remove events with path-level cache invalidation.
- Moved grid controls into the grid action area: sort, select all/clear, star/color filters.
- Expanded file tree context menus: file/folder actions, refresh, rescan, and batch rename all photos.
- Replaced manual copy/move destination input with system folder picker; fixed explorer reveal behavior.

## Features

- **Workspace & Scan**: open multiple folder-based workspaces with background indexing.
- **Grid Mode**: virtualized rendering, `fit/flow` layouts, adjustable thumb size, in-grid sort/filter controls.
- **Cull Mode**: large preview + filmstrip rail, fast navigation, zoom/rotate/reset controls.
- **File Tree Management**: file/folder navigation, context actions, and workspace-wide batch rename.
- **Photo Context Actions**: open, reveal in explorer, copy path, rate/color, export, copy/move, delete.
- **Export Pipeline**: format conversion, quality, max dimension, naming rules, conflict handling.
- **Metadata**: EXIF + editable stars/colors/notes persisted locally.
- **Warmup & Cache**: unified progress popup, background warmup, cache inspection, and cache rebuild.
- **Keybindings**: configurable shortcuts for navigation, tagging, and viewing actions.

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop container | [Tauri 2](https://tauri.app) (Rust) |
| Frontend | [Vue 3](https://vuejs.org) + TypeScript + Vite |
| UI | [Naive UI](https://naiveui.com) |
| State management | [Pinia](https://pinia.vuejs.org) |
| Database | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) |
| Image processing | [image](https://github.com/image-rs/image) |
| EXIF parsing | [kamadak-exif](https://github.com/kamadak/exif-rs) |
| File watching | [notify](https://github.com/notify-rs/notify) |

## Development

Prerequisites:

- [Node.js](https://nodejs.org) >= 18
- [Rust](https://rustup.rs) stable
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) (WebView2 on Windows)

Run in dev mode:

```bash
npm install
npm run tauri dev
```

Build frontend:

```bash
npm run build
```

Build desktop package:

```bash
npm run tauri build
```

## Default Shortcuts

| Action | Shortcut |
|---|---|
| Open workspace | `Ctrl+O` |
| Close workspace tab | `Ctrl+W` |
| Open settings | `Ctrl+,` |
| Show shortcuts help | `?` |
| Toggle grid/cull | `Tab` |
| Navigation | `↑` `↓` `←` `→` |
| Enter preview/lightbox | `Enter` |
| Exit lightbox | `Esc` |
| Rate 1-5 stars | `1` `2` `3` `4` `5` |
| Color labels | `6` `7` `8` `9` |
| Clear marks | `0` |
| Delete | `Delete` |
| Zoom | `+` `-` |
| Reset zoom | `Ctrl+0` |

> All shortcuts can be rebound in Settings.

## License

MIT
