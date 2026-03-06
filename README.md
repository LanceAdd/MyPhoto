# MyPhoto

> 📖 [English version → README_EN.md](README_EN.md)

一款基于 Tauri 2 + Vue 3 构建的桌面照片管理软件，专为摄影师的选片与整理工作流设计。

## 功能特性

- **多工作区** — 以文件夹为单位打开多个工作区，标签页切换
- **网格模式** — 虚拟滚动网格，固定正方形等比缩略图（灰色letterbox），可调缩略图大小
- **选片模式** — 大图预览 + 底部横向胶片轨，键盘左右翻片
- **文件树** — 左侧目录树，支持子文件夹筛选，右键菜单（重命名/新建/在资源管理器中打开）
- **右键菜单** — 网格中右键照片，快速打星、上色、导出、复制/移动、删除
- **元数据面板** — EXIF 信息展示（相机/镜头/快门/光圈/ISO/焦距）+ 可编辑打星、颜色标签、备注
- **星级评分** — 1-5 星，键盘 `1-5` 快速打星
- **颜色标签** — 红/橙/黄/绿/蓝/紫，键盘 `6-9` 快速上色
- **大图模式** — 全屏查看，滚轮缩放，拖拽平移，EXIF HUD
- **照片导出** — 格式转换（JPEG/PNG/WebP）、质量、最长边限制、命名规则、冲突处理
- **SQLite 持久化** — 元数据（星级/颜色/备注/快捷键）本地存储
- **实时文件监听** — 打开工作区即监听文件变动，自动同步缺失/新增
- **快捷键自定义** — 设置页面重新绑定任意操作，`?` 键查看所有快捷键

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面容器 | [Tauri 2](https://tauri.app) (Rust) |
| 前端框架 | [Vue 3](https://vuejs.org) + TypeScript + Vite |
| UI 组件库 | [Naive UI](https://naiveui.com) (Dark theme) |
| 状态管理 | [Pinia](https://pinia.vuejs.org) |
| 数据库 | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) |
| 图像处理 | [image](https://github.com/image-rs/image) crate |
| EXIF 读取 | [kamadak-exif](https://github.com/kamadak/exif-rs) |
| 文件监听 | [notify](https://github.com/notify-rs/notify) crate |

## 开发环境

**前置依赖：**
- [Node.js](https://nodejs.org) >= 18
- [Rust](https://rustup.rs) (stable)
- [Tauri 前置依赖](https://tauri.app/start/prerequisites/)（Windows 需要 WebView2，通常已内置）

**启动开发服务器：**

```bash
npm install
npm run tauri dev
```

**构建发布版本：**

```bash
npm run tauri build
```

## 快捷键

| 操作 | 快捷键 |
|---|---|
| 打开工作区 | `Ctrl+O` |
| 关闭标签页 | `Ctrl+W` |
| 打星 1-5 | `1` `2` `3` `4` `5` |
| 颜色标签 | `6` `7` `8` `9` |
| 清除标记 | `0` |
| 网格 上/下/左/右 | `↑` `↓` `←` `→` |
| 选片模式 上一张/下一张 | `←` `→` |
| 返回网格 | `Tab` |
| 进入大图 | `Enter` |
| 退出大图 | `Esc` |
| 快捷键帮助 | `?` |
| 设置 | `Ctrl+,` |

> 所有快捷键均可在设置页面自定义。

## License

MIT

