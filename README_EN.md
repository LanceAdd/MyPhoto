# MyPhoto

> 📖 [中文版 → README.md](README.md)

A desktop photo management application built with Tauri 2 + Vue 3, designed for photographers' culling and organization workflow.

## Features

- **Multi-workspace** — Open multiple folder-based workspaces with tab switching
- **Grid Mode** — Virtual-scroll grid with fixed square letterbox thumbnails, adjustable size
- **Cull Mode** — Large preview + horizontal filmstrip rail at the bottom, keyboard navigation
- **File Tree** — Left sidebar with subfolder filtering and context menu (rename / new folder / open in explorer)
- **Context Menu** — Right-click photos in grid for quick star, color, export, copy/move, delete
- **Metadata Panel** — EXIF info (camera/lens/shutter/aperture/ISO/focal length) + editable star, color label, notes
- **Star Rating** — 1-5 stars, press `1-5` to rate quickly
- **Color Labels** — Red/Orange/Yellow/Green/Blue/Purple, press `6-9` to label
- **Lightbox** — Full-screen viewer with scroll-to-zoom, drag-to-pan, EXIF HUD
- **Photo Export** — Format conversion (JPEG/PNG/WebP), quality, max dimension, naming rules, conflict handling
- **SQLite Persistence** — Metadata (stars/colors/notes/keybindings) stored locally
- **Real-time File Watcher** — Automatically syncs new/missing files when workspace is open
- **Keybinding Customization** — Rebind any action in settings, press `?` to view all shortcuts

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop container | [Tauri 2](https://tauri.app) (Rust) |
| Frontend | [Vue 3](https://vuejs.org) + TypeScript + Vite |
| UI components | [Naive UI](https://naiveui.com) (Dark theme) |
| State management | [Pinia](https://pinia.vuejs.org) |
| Database | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) |
| Image processing | [image](https://github.com/image-rs/image) crate |
| EXIF reading | [kamadak-exif](https://github.com/kamadak/exif-rs) |
| File watching | [notify](https://github.com/notify-rs/notify) crate |

## Development Setup

**Prerequisites:**
- [Node.js](https://nodejs.org) >= 18
- [Rust](https://rustup.rs) (stable)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) (Windows: WebView2, usually pre-installed)

**Start dev server:**

```bash
npm install
npm run tauri dev
```

**Build for production:**

```bash
npm run tauri build
```

## Keyboard Shortcuts

| Action | Shortcut |
|---|---|
| Open workspace | `Ctrl+O` |
| Close tab | `Ctrl+W` |
| Rate 1-5 stars | `1` `2` `3` `4` `5` |
| Color label | `6` `7` `8` `9` |
| Clear marks | `0` |
| Grid navigate | `↑` `↓` `←` `→` |
| Cull prev/next | `←` `→` |
| Back to grid | `Tab` |
| Open lightbox | `Enter` |
| Close lightbox | `Esc` |
| Shortcuts help | `?` |
| Settings | `Ctrl+,` |

> All shortcuts can be rebound in the Settings page.

## License

MIT
