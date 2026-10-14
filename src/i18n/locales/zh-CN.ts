/**
 * 简体中文目录 —— 默认语言。
 *
 * 类型标注为 `Catalog`（即 en.ts 的形状）：这里漏掉的键是编译错误，不会
 * 悄悄退回英文渲染。
 *
 * 术语：branch 分支 · commit 提交 · working copy 工作区 · stage 暂存 ·
 *      checkout 签出 · remote 远程 · release 版本发布 · issue 议题
 */

import diffZh from "./parts/zh-CN/diff";
import panesZh from "./parts/zh-CN/panes";
import shellZh from "./parts/zh-CN/shell";
import diagnosticsZh from "./parts/zh-CN/diagnostics";
import type { Catalog } from "./en";

const zhCN: Catalog = {
  // Slices, kept as their own files so three parts of the UI can be
  // translated without three people editing the same object literal.
  ...diffZh,
  ...panesZh,
  ...shellZh,
  ...diagnosticsZh,

  app: {
    name: "Meridian",
    // The name's own meaning, not a translation of the English line: a meridian
    // is the line the sun crosses at noon, casting no shadow. Kept short so it
    // can sit under the wordmark.
    tagline: "正午无影之线",
  },

  common: {
    ok: "确定",
    cancel: "取消",
    close: "关闭",
    save: "保存",
    delete: "删除",
    retry: "重试",
    copy: "复制",
    copied: "已复制",
    loading: "加载中…",
    none: "无",
    search: "搜索",
    refresh: "刷新",
    learnMore: "了解更多",
    notAvailableInThisBuild: "当前版本尚未提供。",
  },

  language: {
    label: "语言",
    note: "切换后立即生效，并会记住在这台 Mac 上。",
    en: "English",
    zhCN: "简体中文",
  },

  about: {
    title: "关于 Meridian",
    version: "版本",
    commit: "提交",
    commitUnknown: "此版本未记录",
    buildProfile: "构建",
    buildProfileDebug: "开发版",
    buildProfileRelease: "正式版",
    runtime: "运行时",
    platform: "平台",
    license: "许可",
    licenseValue: "MIT —— 免费且开源",
    project: "项目",
    projectUnknown: "未知 —— 此构建没有 origin 远程",
    openProjectPage: "打开项目主页",
    diagnostics: "诊断信息",
    copyDiagnostics: "复制诊断信息",
    copyDiagnosticsHint:
      "仅含版本、平台与构建信息。不含仓库名、路径或任何使用数据。",
    copied: "已复制到剪贴板。",
    copyFailed: "无法使用剪贴板。",
    showReleaseNotes: "版本日志",
  },

  updates: {
    title: "更新",
    currentVersion: "当前版本 {version}。",
    checkNow: "检查更新",
    checking: "检查中…",
    upToDate: "Meridian {version} 已是最新版本。",
    available: "Meridian {version} 可更新。",
    publishedOn: "发布于 {date}。",
    downloadSize: "需下载 {size}。",
    releaseNotes: "本次改动",
    install: "下载并安装",
    installing: "下载中… {percent}%",
    downloadingUnknown: "下载中…",
    downloadingReceived: "已下载 {received}。",
    preparing: "准备中…",
    readyToRestart: "已就绪。Meridian 将重启以完成更新。",
    restartNow: "立即重启",
    restartLater: "稍后",
    failed: "检查更新失败。",
    failedDetail: "没有做任何改动，你仍可继续使用当前版本。",
    installFailed: "下载未能完成。",
    installFailedDetail: "没有替换任何文件，当前版本照常可用。",
    lastChecked: "上次检查：{when}。",
    neverChecked: "从未检查过。",
    autoCheck: "启动时自动检查",
    autoCheckNote: "Meridian 会在启动后不久检查一次。",
    disclosureTitle: "这会发送什么",
    disclosure:
      "检查动作只是向 GitHub 查询本项目的最新 release 标签。请求只携带应用版本号，不含仓库名、文件路径、使用数据，也不含你或这台机器的任何标识。GitHub 看到的请求，和任何人访问项目主页没有区别。关闭此开关后，Meridian 不会自行发起任何网络请求。",
    unavailableInThisBuild:
      "此构建未配置更新源，因此无法检查新版本。由项目发布流水线产出的构建可以。",
    notConfiguredTitle: "此构建未配置更新",
    debugBuildTitle: "这是开发构建",
    debugBuildDetail:
      "Meridian 不会替换正在开发中的构建。正式版会自行更新。",
    checkingHint: "正在向项目发布页查询最新版本。",
  },

  changelog: {
    title: "版本日志",
    subtitle: "每个版本的改动记录。",
    empty: "此构建未附带版本日志。",
    failed: "无法读取版本日志。",
    unreleased: "未发布",
    releasedOn: "发布于 {date}",
    thisBuild: "当前版本",
    languageOfNotes: "版本日志语言",
    viewOnGitHub: "在 GitHub 查看",
    bundledNote: "随此构建附带；更新的版本可能要在升级后才显示。",
  },

  feedback: {
    title: "反馈",
    subtitle: "问题与需求提交到本项目的公开议题区。",
    bug: "报告问题",
    bugNote:
      "会在 GitHub 上打开一个新议题，并预填版本与平台信息。在你于 GitHub 上按下提交之前，什么都没有发出。",
    feature: "提出需求",
    featureNote: "打开一个议题，描述你希望 Meridian 具备的能力。",
    openOnGitHub: "在 GitHub 打开",
    summaryLabel: "标题",
    summaryPlaceholder: "一句话即可，可以留空。",
    bugTitle: "有个地方不对劲",
    featureTitle: "一个功能需求",
    composing: "准备中…",
    failed: "无法生成报告链接。",
    failedDetail:
      "此构建不知道要报告到哪个项目。你仍可从上方打开项目主页。",
    privacyTitle: "会预填什么",
    privacy:
      "只有应用版本、macOS 版本和机器架构。你在 GitHub 上按提交之前不会发送任何内容，且可以先删掉其中任何一项。",
    warning: "请不要把仓库内容、凭据或令牌粘贴进议题 —— 那是公开的。",
  },

  errors: {
    title: "出了点问题",
    detail: "技术细节",
    retryable: "可以重试。",
    copyCode: "复制错误码",
    byCode: {
      invalid_repository: "这个文件夹不是 Git 仓库。",
      no_working_copy: "这是一个裸仓库，没有工作区。",
      io: "文件操作失败。",
      invalid_request: "Meridian 拒绝了这个请求。",
      index_locked: "另一个 Git 进程正在使用该仓库，请关闭后重试。",
      checkout_conflicts: "签出会覆盖本地改动。",
      checkout_failed: "签出失败。",
      unknown_revision: "找不到修订：{revision}",
      invalid_reference_name: "「{name}」不是合法的分支名或标签名。",
      reference_exists: "「{name}」已存在。",
      reference_not_found: "「{name}」已不存在。",
      operation_in_progress: "该仓库还有另一个操作正在进行中。",
      network_unreachable: "无法连接远程。请检查远程地址和本机网络。",
      auth_required: "远程拒绝了凭据。请检查它使用的密钥或凭据助手。",
      push_rejected: "远程拒绝了推送。请先拉取远程的提交，再重新推送。",
      cancelled: "已取消。",
      unclassified: "出了点问题。",
    },
    /**
     * 同一个错误码可能是完全不同的事故：合并因冲突停下，并不等于签出会覆盖改动。
     * `ErrorPayload.params.class` 指明具体情形，这里给出对应的一句话。
     */
    byClass: {
      merge_conflicts: "合并「{name}」产生了冲突。请在工作区解决后继续。",
      upstream_diverged: "「{name}」与远程已经分叉，无法快进。",
      publish_rejected: "远程拒绝发布「{name}」。请先拉取远程的提交，再重新发布。",
      push_rejected: "远程拒绝了这次推送。请先拉取远程的提交，再重新推送。",
      force_push_lease_mismatch: "远程上的「{name}」在上次读取后已经移动，因此强制推送被拒绝。",
      credential_unavailable: "这个远程没有可用凭据。请配置 git 凭据助手。",
      credential_rejected: "已保存的凭据被远程拒绝，请重新登录。",
      username_required: "远程要求用户名，但没有可用的用户名。",
      unsupported_credential_type: "远程要求的凭据类型 Meridian 无法提供，请配置 git 凭据助手。",
      ssh_unavailable: "找不到可用的 SSH 密钥。请把密钥加入 ssh-agent，或改用 HTTPS 远程。",
    },
  },
};

export default zhCN;
