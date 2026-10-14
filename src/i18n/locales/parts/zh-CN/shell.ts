/**
 * 简体中文目录切片 —— 布局、屏幕、对话框、命令面板。
 *
 * 标注为 `ShellEn`（即 en/shell.ts 的形状）：这里漏掉的键是编译错误，不会
 * 悄悄退回英文渲染。
 *
 * 术语：branch 分支 · commit 提交 · working copy 工作区 · stage 暂存 ·
 *      checkout 签出 · remote 远程 · pull request 合并请求 · issue 议题 ·
 *      filter 筛选 · search 搜索 · theme 主题 · appearance 外观
 *
 * 引号用「」，与 zh-CN.ts 中既有的 `errors.byCode` 保持一致。
 *
 * `@` 在 vue-i18n 消息里是保留字符（链接消息标记），字面量写作 `{'@'}`；
 * 花括号写作 `{'{'}` —— refname 校验的错误文案要照抄 git 自己禁止的字符。
 */

import type { ShellEn } from "../en/shell";

const shellZh: ShellEn = {
  toolbar: {
    ariaLabel: "仓库操作",
    empty: "没有可用的仓库操作。",
    activity: "活动",
    activityOpen: "打开活动列表",
    stalled: "已停滞",
  },

  sidebar: {
    repositoryLabel: "仓库",
    filterLabel: "筛选边栏",
    filterPlaceholder: "筛选",
    switchRepository: "切换仓库",
    noRepositoryOpen: "未打开仓库",
    chooseRepository: "选择仓库",
    recentRepositories: "最近的仓库",
    noOtherRepositories: "没有其他仓库。",
    allRepositories: "全部仓库…",
    rowOptions: "「{name}」的更多选项",
    addToSection: "添加到「{name}」",
  },

  screens: {
    choose: "选择…",
    operation: {
      state: {
        merging: "合并中",
        rebasing: "变基中",
        cherryPicking: "拣选中",
        reverting: "撤销中",
        bisecting: "二分中",
        applyingPatches: "应用补丁中",
        inProgress: "操作进行中",
      },
      bannerHint: "{state}。请完成或中止该操作后继续。",
      continue: "继续",
      abort: "中止",
    },
    masterDetail: {
      resizeLabel: "调整详情面板宽度",
    },
    toast: {
      // 提示正文与操作按钮由调用方 pushToast 时决定，组件只负责给这个关闭
      // 按钮起名。
      dismissNotification: "关闭通知",
    },
    settings: {
      title: "设置",
      sectionsLabel: "设置分区",
      shellChip: "外壳",
      searchPlaceholder: "搜索设置",
      searchLabel: "搜索设置",
      loading: "正在加载设置…",
      tryAgain: "重试",
      emptyTitle: "暂时没有可配置的项",
      emptyCaption: "后端命令就位后，设置会陆续可用。",
      theme: "主题",
      appearance: "外观",
      appearanceHelp: "除非你另选主题，否则跟随系统设置。",
      appearanceAuto: "自动",
      appearanceLight: "浅色",
      appearanceDark: "深色",
      reset: "重置",
      resetTitle: "恢复默认值",
      noMatchingGroup: "没有匹配的设置。",
      noMatchingTitle: "没有匹配的设置",
      nothingMatches: "没有与「{query}」匹配的项。",
    },
    account: {
      title: "账户",
      add: "添加账户",
      loading: "正在加载账户…",
      emptyTitle: "尚未连接账户",
      emptyCaption: "连接托管服务后即可浏览远程与合并请求。",
      notConnected: "未连接",
      noneSelected: "未选择账户",
      noneSelectedCaption: "选择一个账户以查看详情。",
      signOut: "退出登录",
      connect: "连接",
      repositories: "仓库",
      organizations: "组织",
      keys: "密钥",
      noKeys: "此账户没有存储密钥。",
      copyKeyId: "复制密钥 id",
      remove: "移除账户",
      providers: {
        other: "其他服务",
      },
    },
    repository: {
      tabsLabel: "仓库视图",
      // 四个页签名称由 `router/routes.ts` 的 `WORKSPACE_TAB_LABEL_KEYS` 提供，
      // 命令面板与这里的页签栏共用同一套名称。
      noRepositoryOpen: "未打开仓库",
      noRepositoryCaption: "从仓库列表中打开一个仓库。",
    },
    repositories: {
      modeGroupLabel: "仓库操作",
      open: "打开",
      clone: "克隆",
      create: "创建",
      urlLabel: "要克隆的仓库地址",
      newFolderPlaceholder: "新仓库所在的文件夹",
      existingFolderPlaceholder: "包含仓库的文件夹",
      newFolderLabel: "新仓库所在的文件夹",
      openFolderLabel: "要打开的文件夹",
      chooseFolder: "选择文件夹",
      recentsTitle: "最近的仓库",
      filterLabel: "筛选最近的仓库",
      filterPlaceholder: "筛选",
      loadingRecents: "正在加载最近的仓库…",
      noRecents: "没有最近的仓库",
      noRecentsCaption: "打开、克隆或新建一个仓库即可开始。",
      reveal: "在访达中显示",
      removeRecent: "从最近列表中移除",
      nearbyTitle: "附近的仓库",
      scanAgain: "重新扫描",
      nothingNearby: "附近没有找到仓库",
      nothingNearbyCaption: "你已授权的文件夹里的仓库会显示在这里。",
      scanning: "正在扫描仓库…",
    },
  },

  dialogues: {
    prompt: {
      confirmLabel: "创建",
      errors: {
        valueRequired: "请输入内容。",
        nameRequired: "请输入名称。",
        nameAt: "名称不能只是「{'@'}」。",
        nameStartsWithDash: "名称不能以「-」开头。",
        nameSlash: "名称不能以「/」开头或结尾。",
        nameDot: "名称不能以「.」开头或结尾。",
        nameLock: "名称不能以「.lock」结尾。",
        nameDoubleDot: "名称不能包含「..」。",
        nameDoubleSlash: "名称不能包含「//」。",
        nameAtBrace: "名称不能包含「{'@'}{'{'}」。",
        nameInvalidChars: "名称不能包含空格、~ ^ : ? * [ \\ 或控制字符。",
      },
    },
    chooseBranch: {
      confirmLabel: "选择",
      filterLabel: "筛选分支",
      filterPlaceholder: "筛选分支",
      listLabel: "分支",
      empty: "没有分支匹配「{query}」。",
    },
  },

  palette: {
    label: "快速打开",
    queryLabel: "快速打开的搜索词",
    resultsLabel: "结果",
    placeholder: "搜索…",
    empty: "没有匹配项",
  },
};

export default shellZh;