import type { DiagnosticsEn } from "../en/diagnostics";

const diagnosticsZh: DiagnosticsEn = {
  diagnostics: {
    title: "诊断",
    open: "打开诊断",
    description: "本机日志和近期命令结果可帮助查明问题原因。",
    debug: {
      label: "调试模式",
      enabled: "调试模式开启时，会在本机记录详细命令参数。",
      disabled: "当前仅记录命令名称和参数键名。",
      envLocked: "调试模式由 MERIDIAN_DEBUG=1 启用，无法在此更改。",
      source: "模式来源：{source}",
      chip: "调试",
    },
    logs: {
      directory: "日志文件夹",
      file: "日志文件",
      reveal: "在 Finder 中显示",
      title: "近期日志",
      refresh: "刷新日志",
      empty: "没有可用的日志记录。",
    },
    crash: {
      title: "上次运行意外结束",
      dismiss: "关闭提示",
      location: "位置",
      backtrace: "回溯摘录",
    },
    failures: {
      title: "近期失败操作",
      empty: "没有记录失败操作。",
      report: "报告此问题",
      copy: "复制详情",
      command: "命令",
      occurred: "发生时间",
      detail: "技术详情",
    },
    calls: {
      title: "近期命令",
      empty: "没有记录命令。",
      command: "命令",
      started: "开始时间",
      duration: "耗时",
      outcome: "结果",
      arguments: "参数键名",
      success: "成功",
      failed: "失败",
    },
    actions: {
      copyReport: "复制诊断报告",
      export: "导出诊断文件",
      openDevtools: "打开开发者工具",
      report: "报告问题",
      details: "详情",
      copied: "已复制诊断报告。",
      exported: "诊断文件已导出到 {path}",
    },
    internalError: "发生内部错误。打开诊断以查看详情。",
    loadFailed: "无法加载诊断信息。",
  },
};

export default diagnosticsZh;
