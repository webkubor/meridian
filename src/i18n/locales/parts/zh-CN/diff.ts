/**
 * 简体中文目录分片 —— 差异视图、提交列表、提交图与 App.vue 外壳。
 *
 * 类型标注为 `DiffEn`（即 en/diff.ts 的形状）：这里少一个键或写错一个插值名
 * 都是编译错误，不会悄悄退回英文。
 *
 * 复数：中文没有词形变化，所以英文的 `a | b | {count} c` 三种形态在这里写成
 * 一句带 `{count}` 的话，0 / 1 / 多种读下来都通顺；需要按数量分支的句子请改写
 * 句式，不要照抄英文的并列结构。
 *
 * 有三处与英文逐字节相同，是刻意的，需要登记进 catalog.test.ts 的
 * `UNTRANSLATED_BY_DESIGN`：diff.hunkStats 与 commitList.overflowBadge 的
 * `+12 −3`（正负号本身就是信息，规范要求两种语言用同一组字符），
 * 以及 shell.settings.git —— 工具的名字，和 “Meridian” 同理。
 *
 * 术语：diff 差异 · hunk 变更块 · file 文件 · line 行 · added 新增 ·
 * deleted 删除 · context 上下文 · word diff 按词差异 · whitespace 空白 ·
 * branch 分支 · commit 提交 · working copy 工作区 · stage 暂存 · checkout 签出 ·
 * remote 远程 · fetch 抓取 · pull 拉取 · push 推送 · stash 储藏 · tag 标签 ·
 * repository 仓库 · graph 提交图 · lane 分道 · bubble 节点 · ref 引用
 */

import type { DiffEn } from "../en/diff";

const diffZh: DiffEn = {
  diff: {
    regionLabel: "文件差异",
    loading: "正在加载差异…",
    retry: "重试",
    failedTitle: "无法加载差异",
    emptyTitle: "没有改动",
    emptyBody: "此版本与所比较的版本完全一致。",
    noHunks: "该文件没有可显示的文本改动。",

    // 工具栏
    toolbarLabel: "差异选项",
    layoutLabel: "差异布局",
    viewLabel: "布局",
    viewUnified: "统一",
    viewSplit: "并排",
    contextLabel: "上下文",
    contextAria: "上下文行数",
    contextFull: "整个文件",
    contextLines: "{count} 行",
    ignoreWhitespace: "忽略空白",
    ignoreWhitespaceShort: "空白",
    showFullFile: "显示整个文件",
    showFullFileShort: "完整文件",
    syntaxColouring: "语法着色",
    syntaxShort: "语法",
    wordDiff: "按词差异",
    wrap: "长行折行",

    // 变更块
    hunkLabel: "变更块 {header}",
    collapseHunk: "折叠变更块",
    expandHunk: "展开变更块",
    // 与英文逐字节相同：正负号本身就是信息，见文件头。
    hunkStats: "+{added} −{removed}",
    stage: "暂存",
    unstage: "取消暂存",
    discard: "放弃",
    stageThisHunk: "暂存此变更块",
    unstageThisHunk: "取消暂存此变更块",
    discardThisHunk: "放弃此变更块",
    stageHunk: "暂存变更块",
    unstageHunk: "取消暂存变更块",
    discardHunk: "放弃变更块",
    hunkBusy: "处理中…",
    controlsDisabledReason: "暂存此文件前请先解决冲突。",
    controlsDisabledReasonHunks: "暂存变更块前请先解决冲突。",

    // 折叠与截断（永不静默省略）
    expandAbove: "展开 {count} 行隐藏内容",
    expandAboveUnknown: "显示更多上下文",
    expandBelow: "显示下方更多上下文",
    hiddenBannerTitle: "此差异有部分内容未显示",
    hiddenBannerBody: "正在显示 {total} 行中的 {shown} 行。",
    showAll: "全部显示",
    backendElided: "差异引擎提前停止：有 {count} 个变更块（{lines} 行）未生成。",
    requestFull: "请求完整差异",
    hunkClipped: "此变更块被渲染上限截断。",
    renderLimitNote: "过长的差异只渲染前 {count} 行；任何隐藏都会配上这条提示。",

    // 二进制与文件权限
    binaryTitle: "二进制文件",
    binaryNone: "此文件没有文本差异。",
    binaryIdentical: "两侧逐字节相同，仅元数据不同。",
    binaryChanged: "二进制内容不同。",
    binaryOldSize: "修改前：{size}",
    binaryNewSize: "修改后：{size}",
    before: "修改前",
    after: "修改后",
    binaryOid: "{side}：blob {oid}",
    modeChanged: "文件权限从 {from} 变为 {to}。",
    modeAdded: "以 {to} 权限新建文件。",
    modeRemoved: "文件已删除（原权限 {from}）。",

    // 图像
    imageLoading: "正在加载图像数据…",
    imageFailed: "图像数据加载失败",
    imageUnavailable: "此侧没有图像",
    imageChecker: "透明区域以棋盘格底纹显示。",

    // 重命名
    renamedFrom: "由 {path} 重命名而来",

    // 无障碍（docs/04 §9：逐行标签）
    lineLabel: "{where}，{role}：{text}",
    lineNumber: "第 {number} 行",
    lineNumberUnknown: "行号未知",
    roleContext: "未改动",
    roleAdded: "新增",
    roleRemoved: "删除",
    roleNoNewline: "文件末尾",

    // 视图自身渲染的状态
    loadingAria: "正在加载差异",
    tryAgain: "重试",
    emptyPathTitle: "没有可显示的改动",
    emptyNoFileTitle: "未选择文件",
    emptyPathCaption: "此文件在比较的两侧之间没有变化。",
    emptySelectCaption: "选择一个文件即可查看它的差异。",
    gateTitle: "此差异较大",
    gateCaption: "{size}，已超过 {limit} 的自动上限。",
    gateAction: "仍要显示",
    binaryCaption: "此文件没有文本差异。",
    imageTitle: "图像差异",
    // 不是 git 的字面输出 `\ No newline at end of file`：这是行槽里的一句提示，
    // 不是引擎 stdout 的复制，所以可以本地化；变更块头（`@@ -1,4 +1,6 @@`）
    // 属于数据，保持原样。
    noNewline: "\\ 文件末尾没有换行符",
  },

  commitList: {
    listLabel: "提交历史",
    listHint: "用方向键移动选择。",
    today: "今天",
    yesterday: "昨天",
    weekOf: "{date} 所在周",
    rowLabel: "{summary} — {author}，{when}",
    // 与英文逐字节相同，见文件头。
    overflowBadge: "+{count}",
    skeletonsLabel: "正在加载提交",
    skeletonAria: "加载中",
  },

  graph: {
    // 行的拓扑形态，用于行的无障碍名称。画布单元格是 `aria-hidden` 的，
    // 提交图的形状只能从这里被读出——画出来的分道与节点本身不会。
    merge: "合并提交",
    forkPoint: "分叉点",
  },

  shell: {
    subtitle: {
      settings: "设置",
      account: "账户",
      about: "关于 Meridian",
      repositories: "仓库",
      on: "位于 {branch} 上",
    },

    sidebar: {
      workingCopy: "工作区",
      history: "历史",
      branches: "分支",
      remotes: "远程",
      tags: "标签",
      stashes: "储藏",
      emptyWorkingCopy: "无本地改动",
      emptyHistory: "尚无提交",
      emptyBranches: "尚无分支",
      emptyRemotes: "尚无远程分支",
      emptyTags: "尚无标签",
      emptyStashes: "尚无储藏",
      rowWorkingCopy: "本地改动",
      rowHistory: "全部提交",
    },

    toolbar: {
      branch: "分支",
      fetch: "抓取",
      pull: "拉取",
      push: "推送",
      stash: "储藏",
      quickOpen: "快速打开",
      quickActions: "快捷操作",
      settings: "设置",
      account: "账户",
      about: "关于 Meridian",
      newBranch: "新建分支…",
      checkoutBranch: "签出分支…",
      renameBranch: "重命名分支…",
      noRepository: "请先打开一个仓库。",
      noRemote: "此仓库没有配置远程。",
      detachedHead: "HEAD 处于游离状态 —— 请先签出分支再推送。",
      activityUnavailable: "当前版本尚未提供活动跟踪。",
    },

    // 工作区标签页。`router/routes.ts` 以 `labelKey` 读取同一批键，
    // 路由表本身不再成为标签的存放处。
    tab: {
      workingCopy: "工作区",
      history: "历史",
      branches: "分支",
      compare: "比较",
    },

    palette: {
      refreshRepository: "刷新仓库",
      openSettings: "打开设置",
      openAccounts: "打开账户",
      repositoryList: "仓库列表",
      groupBranches: "分支",
      groupRepository: "仓库",
      groupRepositories: "仓库",
      groupApplication: "应用",
      groupViews: "视图",
      headingActions: "快捷操作",
      headingOpen: "快速打开",
      placeholderActions: "执行一条命令",
      placeholderOpen: "搜索仓库、分支与视图",
      empty: "没有匹配项",
    },

    settings: {
      appearance: "外观",
      editor: "编辑器",
      editorFontSize: "编辑器字号",
      editorFontSizeHelp: "作用于差异视图与 blame 视图。",
      git: "Git",
      safety: "安全",
      confirmCheckout: "带本地改动签出前先确认",
      confirmCheckoutHelp: "工作区不干净时，每次签出前都先弹出确认。",
      general: "通用",
      services: "服务",
      advanced: "高级",
    },

    dialogue: {
      newBranchTitle: "新建分支",
      newBranchLabel: "分支名",
      newBranchHelp: "新分支从当前 HEAD 开始。",
      newBranchCheckoutOption: "签出新建的分支",
      renameBranchTitle: "重命名分支",
      renameBranchLabel: "新名字",
      deleteBranchTitle: "删除分支？",
      deleteBranchMessage: "删除 {branch}？此操作无法撤销。",
      deleteBranchConfirm: "删除分支",
      checkoutTitle: "签出分支",
      checkoutDirtyTitle: "带本地改动签出？",
      checkoutDirtyMessage: "你有 {count} 处未提交的改动，签出可能失败，也可能把它们一并带过去。",
      checkoutDirtyConfirm: "仍要签出",
      discardTitle: "放弃改动？",
      discardMessage: "这会永久放弃此文件中的改动，无法撤销。",
      discardConfirm: "放弃改动",
    },

    // 提交按钮下方的分叉动作。
    commitAndPush: "提交并推送",

    toast: {
      noBranchSelected: "请先选择一个分支。",
      linkFailed: "无法打开该链接。",
      feedbackOpened: "已在 GitHub 打开，预填了 {count} 项信息。",
      revealed: "已在文件管理器中显示。",
      copied: "已复制到剪贴板。",
      copyFailed: "无法复制到剪贴板。",
      actionFailed: "操作失败。",
      fetched: "抓取完成。",
      pulled: "拉取完成。",
      pushed: "已推送 {branch}。",
      stashed: "已储藏工作区。",
      branchCreated: "已创建分支 {branch}。",
      branchCreateFailed: "无法创建该分支。",
      branchRenamed: "已重命名为 {branch}。",
      branchRenameFailed: "无法重命名该分支。",
      merged: "已合并 {branch}。",
      rebased: "已变基到 {branch}。",
      branchDeleted: "已删除 {branch}。",
      branchDeleteFailed: "无法删除该分支。",
      checkedOut: "已签出 {branch}。",
      checkoutFailed: "无法签出该分支。",
      discarded: "已放弃该文件的改动。",
      discardFailed: "无法放弃这些改动。",
      committed: "提交完成。",
      commitFailed: "无法创建该提交。",
      committedAndPushed: "已提交并推送。",
      conflictResolved: "冲突已解决。",
      commitMessageRequired: "请先填写提交信息。",
      operationContinued: "操作已继续。",
      operationAborted: "操作已中止。",
      mergeContinued: "合并已继续。",
      mergeAborted: "合并已中止。",
    },

    unavailable: {
      addSection: "当前版本尚不支持添加{noun}。",
      nounEntries: "条目",
      nounWorkingCopy: "工作区条目",
      nounCommits: "提交",
      nounRemotes: "远程",
      nounTags: "标签",
      nounStashes: "储藏",
      historyMergeFilter: "当前版本尚不支持按合并状态筛选。",
      browseFolder: "当前版本尚不支持选择文件夹。",
      clone: "当前版本尚不支持克隆。",
      scanNearby: "当前版本尚不支持扫描附近的仓库。",
      compare: "当前版本尚不支持比较修订。",
      accounts: "当前版本尚不支持连接和管理托管账户。",
    },
    activity: {
      title: "活动",
      subtitle: "Meridian 此刻正在做的事。",
      running: "正在运行 {count} 项操作",
      idle: "当前没有正在运行的操作。",
      state: {
        running: "运行中",
        done: "已完成",
        failed: "失败",
      },
      clear: "清除已完成",
      nothingToClear: "还没有已完成的操作。",
      empty: "还没有任何操作",
      emptyCaption: "抓取、拉取、推送与克隆在运行时会显示在这里。",
      privacy:
        "这份清单来自你在本窗口里的点击。不写入磁盘，也不发送到任何地方。",
    },

  },
};

export default diffZh;