/**
 * 简体中文目录 —— 面板分片。
 *
 * 类型标注为 `PanesEn`（即 en/panes.ts 的形状）：这里漏掉的键是编译错误，
 * 不会悄悄退回英文渲染。
 *
 * 与英文目录的取舍
 *  - `_plural` 在中文里与单数同形。这两个键必须都存在，否则类型对不齐；
 *    但它们不是"漏翻"。
 *  - `commit.filesChanged` 之类用「{count} 个文件」而不是「文件数：{count}」，
 *    中文读起来更像一句话。
 *  - `workingCopy.noun*` 单独成键，是因为英文的 "Stage all modified" 靠语序
 *    成句，中文没有对应说法，只能换成「暂存所有已修改的文件」这种带「所有」
 *    的结构。
 *  - HEAD、origin、ref 保持原文；文件路径、哈希、作者名是数据，不翻译。
 *
 * 术语：branch 分支 · commit 提交 · working copy 工作区 · stage 暂存 ·
 *      checkout 签出 · remote 远程 · fetch 抓取 · pull 拉取 · push 推送 ·
 *      diff 差异 · blame 溯源 · upstream 上游 · ancestor 祖先
 */

import type { PanesEn } from "../en/panes";

const panesZh: PanesEn = {
  commit: {
    // --- 提交详情 ---------------------------------------------------------
    loadingCommit: "正在加载提交…",
    noCommitSelected: "未选择提交",
    pickCommitHint: "在历史中选一个提交，即可查看详情。",
    loadFailed: "无法加载提交",
    copySummary: "复制标题行",
    copyHash: "复制完整哈希",
    browseFiles: "浏览文件",
    browseTreeAt: "浏览 {ref} 处的目录树",
    browseTreeHere: "浏览本次提交的目录树",
    parent: "父提交",
    parents: "{count} 个父提交",
    loadingFiles: "正在加载文件…",
    noChangedFiles: "没有变更文件",
    diffUnavailableTitle: "此构建尚不能显示提交差异",
    diffUnavailableCaption:
      "此构建未包含「读取某次提交下单个文件差异」的命令，请在上方的变更列表中选择文件。",
    noFileSelected: "未选择文件",
    noFileSelectedCaption: "选择一个文件，即可查看它的差异。",

    // --- 变更列表 ---------------------------------------------------------
    statusAdded: "新增",
    statusCopied: "已复制",
    statusDeleted: "已删除",
    statusModified: "已修改",
    statusRenamed: "已重命名",
    statusUnmerged: "未合并",
    statusUntracked: "未跟踪",
    statusIgnored: "已忽略",
    filesChanged: "{count} 个文件",
    filesChanged_plural: "{count} 个文件",
    binary: "二进制",

    // --- 签名徽标 ---------------------------------------------------------
    signatureUnsigned: "未签名",
    signaturePresent: "已签名（未验证）",
    signatureValid: "签名已验证",
    signatureUntrusted: "由不受信任的密钥签名",
    signatureInvalid: "签名无效",
    signatureExpired: "签名已过期",

    // --- 提交框 -----------------------------------------------------------
    submit: "提交",
    amend: "修订",
    amendTitle: "修订提交",
    stagedCount: "暂存 {count} 个",
    unstagedCount: "未暂存 {count} 个",
    runningHook: "正在运行钩子",
    hooksOn: "钩子已启用",
    subjectPlaceholder: "提交标题",
    amendPlaceholder: "修订上一个提交",
    subjectLabel: "提交标题",
    bodyLabel: "提交正文",
    bodyPlaceholder: "描述这次改动（可选）",
    addDescription: "添加描述",
    unchangedMessage: "与上一次尝试相比，提交信息没有变化。",
    signOff: "附加签名行",
    reasonBusy: "已有一个提交正在执行。",
    reasonNoCommitToAmend: "没有可修订的提交。",
    reasonAmendNeedsChange: "改动提交信息，或暂存一些改动，才能修订。",
    reasonEnterSubject: "请填写提交标题。",
    reasonStageToCommit: "请先暂存一些改动。",
  },

  history: {
    emptyTitle: "没有提交",
    emptyCaption: "这个仓库还没有历史记录。",
    loading: "正在读取历史…",
    searchLabel: "搜索提交",
    searchPlaceholder: "搜索提交信息、作者或哈希",
    typeLabel: "提交类型",
    typeAll: "全部提交",
    typeMerges: "合并提交",
    typeNonMerges: "非合并提交",
    clear: "清除",
    viewSettings: "显示设置",
    sectionView: "显示",
    showGraph: "显示提交图",
    firstParentOnly: "仅第一父提交",
    sectionDensity: "密度",
    densityCompact: "紧凑",
    densityRegular: "标准",
    sectionGrouping: "按日期分组",
    groupingOff: "不分组",
    groupingDay: "按天",
    groupingWeek: "按周",
    groupingMonth: "按月",
    sectionSort: "排序",
    sortDate: "按时间",
    sortTopo: "拓扑序",
  },

  workingCopy: {
    emptyTitle: "没有本地改动",
    emptyCaption: "工作区与最近一次提交一致。",
    loading: "正在读取工作区…",
    refreshTitle: "刷新工作区",
    countStaged: "暂存 {count} 个",
    countModified: "已修改 {count} 个",
    countUntracked: "未跟踪 {count} 个",
    conflictsOnly: "只看冲突（{count}）",

    sectionConflicts: "冲突",
    sectionStaged: "暂存区",
    sectionModified: "已修改",
    sectionUntracked: "未跟踪",
    emptyConflicts: "没有冲突",
    emptyStaged: "暂存区没有改动",
    emptyModified: "没有未暂存的改动",
    emptyUntracked: "没有未跟踪的文件",
    noFiles: "没有文件",

    stageAll: "全部暂存",
    unstageAll: "全部取消暂存",
    bulkStageAll: "暂存所有{noun}",
    bulkUnstageAll: "取消暂存所有{noun}",
    nounStaged: "已暂存的改动",
    nounModified: "已修改的文件",
    nounUntracked: "未跟踪的文件",

    stage: "暂存",
    unstage: "取消暂存",
    discard: "放弃",
    discardChanges: "放弃改动",
    assumeUnchanged: "标记为未改动",
    clearAssumeUnchanged: "取消「标记为未改动」，让此文件重新显示",
    selectFile: "选择 {path}",

    mergeStatusLabel: "合并状态",
    merging: "正在合并 {label} —— {total} 个冲突中还剩 {remaining} 个。",
    incomingChanges: "传入的改动",
    incomingTitle: "传入的改动",
    conflictsResolved: "冲突已全部解决。提交以完成合并。",
    cleanMerge: "{label} 可以直接合并（{commits} 个提交）。",
    cleanMerge_plural: "{label} 可以直接合并（{commits} 个提交）。",
    commitMerge: "完成合并",
    abortMerge: "中止",

    openMergeTool: "打开这个文件的合并工具",
    resolve: "解决",
    blame: "溯源",
    blameThisFile: "溯源此文件",
    openInEditor: "在默认编辑器中打开",
    conflictNote: "此文件存在冲突，解决之前无法按变更块暂存。",
    noFileSelected: "未选择文件",
    noFileSelectedCaption: "从列表中选一个文件，即可查看它的改动。",
  },

  branch: {
    emptyTitle: "没有分支",
    emptyCaption: "创建一个分支开始工作。",
    emptyAction: "新建分支…",
    newBranch: "新建分支",
    loading: "正在读取分支…",
    scopeLabel: "分支范围",
    filterLocal: "本地",
    filterRemote: "远程",
    filterAll: "全部",
    searchLabel: "搜索分支",
    searchPlaceholder: "筛选分支",
    tracking: "跟踪 {ref}",
    merged: "已合并",
    copyName: "复制分支名",
    noSelectionTitle: "未选择分支",
    noSelectionCaption: "选一个分支，即可看到它的操作。",
    factTip: "顶端",
    factUpstream: "上游",
    factLastAuthor: "最近作者",
    factTracking: "跟踪状态",
    notTracking: "未设置上游",
    aheadBy: "领先 {count} 个提交",
    behindBy: "落后 {count} 个提交",
    checkout: "签出",
    checkedOut: "已签出",
    mergeInto: "合并到 {name}",
    mergeIntoCurrent: "合并到当前分支",
    rebaseOnto: "变基到",
    push: "推送",
    fetchIntoLocal: "抓取到本地",
    showInHistory: "在历史中显示",
    compare: "比较…",
    rename: "重命名",
    delete: "删除",
    reasonCheckedOut: "不能删除已签出的分支。",
    reasonProtected: "此分支受保护。",
  },

  compare: {
    baseSide: "基准",
    compareSide: "比较",
    chooseRef: "选择一个引用",
    swapTitle: "交换基准与比较对象",
    noCommonAncestor: "没有共同祖先",
    mergeBase: "合并基点 {base}",
    onlyIn: "仅在 {ref} 中",
    targetFallback: "比较对象",
    baseFallback: "基准",
    emptyTitle: "选择两个引用",
    emptyCaption: "选一个基准和一个比较对象，即可开始比较。",
    loading: "正在比较…",
  },

  blame: {
    showDiff: "显示差异",
    open: "打开",
    loading: "正在溯源…",
    emptyTitle: "无可溯源的内容",
    emptyCaption: "此文件没有已提交的历史。",
    annotationLabel: "溯源标注",
    showCommit: "查看提交",
    copyHash: "复制哈希",
  },
};

export default panesZh;
