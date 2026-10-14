const diagnostics = {
  diagnostics: {
    title: "Diagnostics",
    open: "Open diagnostics",
    description: "Local logs and recent command results help explain what happened.",
    debug: {
      label: "Debug mode",
      enabled: "Detailed command arguments are recorded locally while debug mode is on.",
      disabled: "Only command names and argument keys are recorded.",
      envLocked: "Debug mode is enabled by MERIDIAN_DEBUG=1 and cannot be changed here.",
      source: "Mode source: {source}",
      chip: "debug",
    },
    logs: {
      directory: "Log folder",
      file: "Log file",
      reveal: "Show in Finder",
      title: "Recent log entries",
      refresh: "Refresh log",
      empty: "No log entries are available.",
    },
    crash: {
      title: "The previous run ended unexpectedly",
      dismiss: "Dismiss",
      location: "Location",
      backtrace: "Backtrace excerpt",
    },
    failures: {
      title: "Recent failures",
      empty: "No failures recorded.",
      report: "Report this problem",
      copy: "Copy details",
      command: "Command",
      occurred: "Occurred",
      detail: "Technical detail",
    },
    calls: {
      title: "Recent commands",
      empty: "No commands recorded.",
      command: "Command",
      started: "Started",
      duration: "Duration",
      outcome: "Result",
      arguments: "Argument keys",
      success: "Succeeded",
      failed: "Failed",
    },
    actions: {
      copyReport: "Copy diagnostic report",
      export: "Export diagnostics file",
      openDevtools: "Open developer tools",
      report: "Report a problem",
      details: "Details",
      copied: "Diagnostic report copied.",
      exported: "Diagnostics exported to {path}",
    },
    internalError: "An internal error occurred. Open diagnostics for details.",
    loadFailed: "Could not load diagnostics.",
  },
};

export default diagnostics;
export type DiagnosticsEn = typeof diagnostics;
