# 逆向证据基座 — Git Tower 18.0 (arm64)

来源：`/Users/webkubor/Downloads/Tower.app`（macOS 通用二进制，已 `lipo -thin arm64`）
工具：REA (`rea-agents` 6.4.0) + `strings`/`nm`/`otool` 兜底
采集时间：2026-10-14

## 文件

| 文件 | 内容 | 体量 |
|---|---|---|
| `ui-documents.txt` | 403 个自有 Interface Builder 档案：每个的类清单 + outlets + actions | 161 KB |
| `ui-all-classes.txt` | 1211 个去重界面类名（Swift mangled + AppKit 类） | 34 KB |
| `ui-graph.json` | 合并后的对象图：19924 nodes / 9099 edges（contains/outlet/action） | 24 MB |
| `fngit-types.txt` | 996 个 FNGit*/GTGit* 类型名 —— 领域层 + 引擎层 | 30 KB |
| `tower-strings.txt` | arm64 切片全量可打印字符串（87327 行） | 1.9 MB |
| `assets.json` + `assets2.json` | Assets.car 元数据，1698 条（设计 token + SF Symbols） | — |
| `otool-L.txt` | 链接库全表（自有 framework + 系统框架） | — |
| `../rea-out/ib-{aa..ag}.json` | 7 个批次的原始 REA 证据（可下钻） | 24 MB |
| `binary-layout.json` | REA 能力受限（pwntools profile 仅支持 Linux x64），已用 otool 兜底 | 444 B |

## 目标 bundle 结构（来自目录实勘）

```
Tower.app/Contents/
  MacOS/Tower            46.3 MB  主二进制（arm64 46.3MB / x86_64）
  MacOS/TowerAskpass      792 KB  凭据弹窗辅助进程
  Frameworks/
    FNGit.framework          领域包装层（FNGit* ~45 类型）
    GTGit 引擎（内嵌于 FNGit，996 类型中 951 个为 GTGit*）
    FNAppKit.framework       自有 AppKit 组件
    FNFoundation.framework   自有 Foundation 扩展
    FNMarkdown.framework     Markdown 渲染
    FNLicensing.framework    授权
    GitHubAPI / GitLabAPI / BitbucketAPI / AzureDevOpsAPI /
    StashAPI / BeanstalkAPI / CustomServiceAPI   服务集成（各一个 framework）
    Sparkle.framework        自动更新（与本项目无关）
  Resources/
    *.nib                    400 个 Tower 自有视图档案
    *.storyboardc            3 个 storyboard
    git/                     自带 git 2.51.0（完整二进制）
    git-flow/                git-flow-tower（自有 fork，shell 脚本）
    git-lfs/                 git-lfs
    Assets.car               1698 条资产
```

## 关键结论（均可追到上表证据）

1. **底层不是 shell git，是 libgit2 绑定。** 引擎层命名为 `GTGit*`，与领域层 `FNGit*` 之间有一整套
   `FNGitXxxToGTGitXxxTransformer` 枚举转换器（MergeMode / ResetMode / FileStatus / ObjectType /
   SubmoduleStatus / SubmoduleIgnoreMode）。→ 我们的 Rust 侧应对位 `git2` crate，复刻同样的分层。
2. **引擎层统一三段式：`Action` → `ActionValidator` → `Operation`。**
   例：`GTGitAddRemoteAction` / `GTGitAddRemoteActionValidator` / `GTGitAddRemoteOperation`。
   → 直接映射为 Tauri 的 `command` + 前端预校验 + Rust 执行。
3. **git 子命令面**（按二进制内出现频次）：branch 423、remote 143、merge 84、pull 51、status 48、
   worktree 37、submodule 37、config 32、rebase 31、diff 25、push 23、fetch 23、tag 20、stash 17、
   checkout 17、show 12、notes 10、log 10、reset 9、clone 9、revert 8、blame 8、describe 5、
   cherry-pick 5、restore 4、lfs 4、apply 4、rev-parse 3、gc 2、credential 1。
4. **deep-link 协议**：`gittower`、`github-mac`、`x-github-client`、`sourcetree`（Info.plist CFBundleURLTypes）。
   → 值得复刻：从 GitHub Desktop / Sourcetree / `x-github-client` 唤起并 clone。
5. **服务集成**：GitHub / GitLab / Bitbucket / Azure DevOps / Stash / Beanstalk / Custom（自定义 HTTP 服务），
   另有 **Graphite**（stack-based PR 工具）的 submit/get/next 专用符号。
6. **设计 token**：`accent_primary_color`、`BlueHintBackgroundColor`、`RedHintBackgroundColor`、
   `GreenHintBackgroundColor`、`YellowHintBackgroundColor`、`SecondaryContentBackgroundColor`、
   `TertiaryContentBackgroundColor` 等；资产含 light/dark（NSAppearanceNameAqua / DarkAqua）与 tintable。
7. **SF Symbols 词汇表**（24 个 symbol_* 条目 × 多态）：commit / branch / merge / merge_warning /
   fetch / pull / create_branch / force_push / quick_open / apply_stash / save_stash / stashes /
   git_flow / git_config / user_profile / finder / branch_worktree / archived_branch_* /
   graphite_get / graphite_submit / missing_parent 等。

## REA 声明的局限（照录，不外推）

- 无 Hopper / Ghidra / IDA → **没有原生伪代码、没有函数级调用图**。结论只能到"类型名/符号名/字符串常量/序列化档案"这一层。
  - 已尝试补齐：装好 Ghidra 12.1.4 + JDK 21（REA `doctor` 报 ghidra healthy），但 REA 要求
    `Ghidra/Features/Decompiler/os/mac_arm_64/decompile`，官方发行只打包 `linux_x86_64` 与 `win_x86_64`。
    本机从源码编译失败（`loadimage_bfd.cc` 需要 `<bfd.h>`，发行包内不含 binutils 源码，需另行构建）。
    → **本机拿不到原生伪代码，这是既成事实**，已停止该路径。若将来确实需要：
    装 Hopper Disassembler（REA 支持，macOS 原生）、或在 Linux x64 上跑 REA + Ghidra。
- Interface Builder 档案是私有序列化对象图：解码器只报告可识别的 keyed-archive 对象与连接，
  **未识别的类与字段仍然未知**；运行时可见性、动态 UI 构建、像素几何**不从档案推断**。
- target-action selector 只是 IB 观察，**不证明二进制里存在匹配实现**。
- `strings` 命中只证明字符串存在，**不证明该代码路径可达**。
- `analyze` / `decompile` / `function` / 调用图需要分析引擎：Ghidra 12.1.x + JDK 21（本机已装 JDK 21，
  Ghidra 下载中）。即使装上，通用二进制需按切片分析，且结论仍受上述限制约束。

## 复刻红线

抄机制，不抄资产：不复制对方的代码、图标、nib 资源、文案。逆向结论用于**自己实现同等能力**，
视觉走我们自己的 token 与 SF Symbols 词汇。