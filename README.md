# Better DSH

DeepSeek Harness 的轻量桌面客户端，基于 [Tauri 2](https://tauri.app/) + Rust + TypeScript 构建。

A lightweight, local-first desktop client for [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) (dsh), built with Tauri 2 + Rust + TypeScript.

## 功能 / Features

- **一键启停** — 在应用内直接启动 / 停止 `dsh web` 服务（默认端口 3080）
- **状态监控** — 实时显示服务运行状态、PID、端口、dsh 版本与启动器路径
- **内置浏览器** — 一键在应用内打开 Harness Web UI，无需手动访问浏览器
- **系统托盘** — 关闭窗口最小化到托盘；托盘菜单支持显示/隐藏、打开 Harness、控制面板与退出
- **退出联动** — 可选：退出应用时自动停止 dsh 服务（可取消勾选）
- **轻量** — 单 exe 约 8.6 MB（对比 Electron 壳同类项目 100 MB+），内存占用更低

> 与现有 Electron 方案（如 anywhere-labs/deepseek-harness-desktop）相比，Better DSH 采用 Tauri 2，体积与资源占用显著更小。

## 安装 / Install

### 前置依赖 / Prerequisites

- Windows 10/11（当前仅支持 Windows）
- [Node.js](https://nodejs.org/) 18+
- 全局安装 dsh：

```bash
npm install -g @deepseek-ai/dsh
```

### 下载安装包 / Download

从 [Releases](../../releases) 下载 `Better DSH_x.x.x_x64_en-US.msi` 安装即可。

## 开发 / Development

```bash
# 安装前端依赖
pnpm install

# 开发模式（热更新，需先安装 Rust 工具链）
pnpm tauri dev

# 构建 release 安装包（MSI）
pnpm tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 技术栈 / Tech Stack

- [Tauri 2](https://tauri.app/) — 桌面应用框架
- Rust — 后端服务管理（进程启停、端口检测、托盘）
- Vite + TypeScript + 原生 HTML/CSS — 前端控制面板

## 项目结构 / Structure

```text
better-dsh/
├── src/                 # 前端（控制面板 UI）
│   ├── main.ts
│   └── styles.css
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs       # Tauri 主逻辑（托盘、命令、窗口事件）
│   │   └── dsh.rs       # dsh 服务管理（启动/停止/状态/端口检测）
│   ├── icons/
│   ├── tauri.conf.json
│   └── Cargo.toml
├── index.html
└── package.json
```

## 路线图 / Roadmap

- [ ] NSIS 安装包支持
- [ ] 自更新（tauri-plugin-updater）
- [ ] 自定义端口 / 数据目录设置
- [ ] 启动日志查看页
- [ ] macOS / Linux 支持
- [ ] 应用图标与品牌视觉

## License

[MIT](./LICENSE)
