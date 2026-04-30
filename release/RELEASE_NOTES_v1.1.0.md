# OpenClaw Installer v1.1.0

## 中文

### 亮点

- 默认主模型升级为 `anthropic/claude-sonnet-4-6`（Claude Sonnet 4.6，最新主流编程模型）
- 预置模型目录新增 Claude Opus 4.7、Claude Haiku 4.5（带日期 ID）、GPT-5.3 Codex 等
- 顶栏运行/停止状态药丸按颜色区分，运行时心跳脉冲；侧边导航在前置阶段未完成时禁用
- 欢迎页首次打开自动跑环境检查，给出"就绪/未通过"明确状态
- 安装向导：取消切步全屏遮罩、合并模型主选下拉与文本框为单一组件、长提示折叠
- 执行页：安装步骤之间自动推进，无需手动点"下一步"；状态文案完整本地化
- 维护中心：状态卡片、安全得分本地化；"结束 OpenClaw"和"删除 OpenClaw"改为危险样式
- 通用：全局过渡、悬停/聚焦反馈、`prefers-reduced-motion` 兜底，整体更流畅

### 升级建议

- 直接覆盖安装；旧 `.openclaw` 配置不会被动到（仍隔离在 `%LOCALAPPDATA%\OpenClawInstaller\openclaw`）
- 若已切换至 GPT-5.2 等模型且想保留，可在维护中心重新选择，本次不会强制改写已有配置

### 下载建议

- 大陆用户优先下载：`OpenClawInstaller-v1.1.0-windows.zip`
- 解压后双击：`INSTALL_NOW.cmd`

---

## English

### Highlights

- Default primary model upgraded to `anthropic/claude-sonnet-4-6` (Claude Sonnet 4.6 — latest mainstream coding model)
- Preset catalog adds Claude Opus 4.7, Claude Haiku 4.5 (dated ID), GPT-5.3 Codex
- Status pill is colored by running/stopped with a pulse animation; sidebar nav is gated by stage prerequisites
- Welcome page auto-runs pre-flight on first visit and shows a clear ready/blocked banner
- Wizard: removed the full-screen busy mask on step changes, merged primary model dropdown + text input into a single picker, collapsed verbose hints
- Execute page: install steps auto-advance — no manual "Next step" clicks; step state labels fully localized
- Maintenance center: status card and security score localized; "End OpenClaw" and "Delete OpenClaw" use a danger style
- Global polish: smoother transitions, hover/focus feedback, `prefers-reduced-motion` fallback

### Upgrade notes

- Drop-in install; existing `.openclaw` state stays isolated under `%LOCALAPPDATA%\OpenClawInstaller\openclaw`
- Existing config is not rewritten — switch models from the maintenance center if you want to keep your previous choice

### Download recommendation

- Mainland users: `OpenClawInstaller-v1.1.0-windows.zip`
- After extract, run: `INSTALL_NOW.cmd`
