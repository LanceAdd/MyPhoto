# MyPhoto

> 📖 [English version → README_EN.md](README_EN.md)

基于 Tauri 2 + Vue 3 的桌面照片管理工具，面向摄影工作流中的“浏览-筛选-选片-导出”。

## 近期更新（2026-03）

- 修复导出命令参数校验问题（`photo_ids`），补全导出进度弹窗与结果汇总。
- 导出自动重命名支持自定义前缀 + 后半部分生成器：`seq` / `date_seq` / `timestamp_seq`。
- 缩略图预热升级为右下角弹窗，支持最小化/展开，展示统一总进度（首批 + 后台阶段）。
- 预热流程统一：单次读取图片可同时生成网格与选片缓存，减少重复 IO/解码。
- 首次启动按设备性能自动初始化预热数量与并发线程，设置页可手动覆盖。
- 文件监听链路支持新增/修改/删除增量同步，路径级缓存失效，降低无效重建。
- 网格工具栏整合排序、全选/取消全选、星级筛选、颜色筛选（空结果时工具可继续操作）。
- 文件树右键菜单增强：文件/文件夹操作、刷新树、重扫工作区、批量重命名全部图片。
- 网格右键“复制到/移动到”改为系统目录选择器；“在文件管理器中显示”定位行为修复。

## 功能特性

- **多工作区与扫描**：按文件夹打开多个工作区，后台扫描并写入索引。
- **网格模式**：虚拟滚动、`fit/flow` 布局、缩略图尺寸调节、网格内排序与高级筛选。
- **选片模式**：大图预览 + 底部胶片轨，快速翻片，预览缩放/旋转/重置。
- **文件树管理**：按目录与文件浏览，右键重命名/新建/删除/刷新/重扫，支持批量重命名全库图片。
- **照片右键菜单**：系统打开、资源管理器定位、复制路径、打星、上色、导出、复制/移动、删除。
- **导出系统**：格式转换（JPEG/PNG/WebP/原格式）、质量、最长边限制、命名规则与冲突策略。
- **元数据**：EXIF 展示（机身/镜头/曝光参数）+ 星级、颜色、备注本地持久化。
- **预热与缓存**：统一预热进度、后台持续预热、缓存信息查看与一键重建。
- **快捷键体系**：默认键位覆盖浏览/选片/标记/缩放，支持设置面板自定义。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面容器 | [Tauri 2](https://tauri.app) (Rust) |
| 前端 | [Vue 3](https://vuejs.org) + TypeScript + Vite |
| UI | [Naive UI](https://naiveui.com) |
| 状态管理 | [Pinia](https://pinia.vuejs.org) |
| 数据库 | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) |
| 图像处理 | [image](https://github.com/image-rs/image) |
| EXIF 读取 | [kamadak-exif](https://github.com/kamadak/exif-rs) |
| 文件监听 | [notify](https://github.com/notify-rs/notify) |

## 开发与构建

前置依赖：

- [Node.js](https://nodejs.org) >= 18
- [Rust](https://rustup.rs) stable
- [Tauri prerequisites](https://tauri.app/start/prerequisites/)（Windows 需 WebView2）

开发启动：

```bash
npm install
npm run tauri dev
```

前端构建：

```bash
npm run build
```

桌面打包：

```bash
npm run tauri build
```

## 默认快捷键

| 操作 | 快捷键 |
|---|---|
| 打开工作区 | `Ctrl+O` |
| 关闭工作区标签 | `Ctrl+W` |
| 打开设置 | `Ctrl+,` |
| 快捷键帮助 | `?` |
| 网格/选片切换 | `Tab` |
| 方向导航 | `↑` `↓` `←` `→` |
| 进入大图/预览 | `Enter` |
| 退出大图 | `Esc` |
| 星级 1-5 | `1` `2` `3` `4` `5` |
| 颜色标记 | `6` `7` `8` `9` |
| 清除标记 | `0` |
| 删除 | `Delete` |
| 缩放 | `+` `-` |
| 缩放重置 | `Ctrl+0` |

> 所有快捷键都可在设置页重绑定。

## License

MIT
