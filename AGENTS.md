# AGENTS.md — AI 助手工作守则

## 这是什么

Meridian —— macOS 上的 Git 客户端。Tauri 2 + Vue 3 前端，`git2`（libgit2）引擎。
MIT、无付费档、无遥测、无账号。产品设计来自对 Git Tower 18 的逆向研究
（证据与结论见 `docs/00-evidence-summary.md`）。

## 关键文件

- `docs/`：逆向研究证据与设计决策 —— **改交互前先读**，设计不是拍脑袋来的。
- `design/`：设计资产。
- `CLAUDE.md`：Claude 项目上下文（更具体的以它为准，与本文不冲突）。
- `CHANGELOG.md` / `CHANGELOG.zh-CN.md`：双语更新日志，同步改。

## 常用命令

```bash
pnpm dev           # 前端开发
pnpm tauri dev     # 桌面端联调
pnpm typecheck     # 类型检查
pnpm test          # 测试
```

## 约定

- Git 操作一律走 git2 绑定，不 shell out 到系统 git（行为一致性）。
- 无遥测是承诺：不引入任何上报。
- UI 遵循 docs 里的逆向研究结论，偏离要有记录的设计理由。
