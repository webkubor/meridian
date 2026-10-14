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
      sourceValues: {
        env: "the MERIDIAN_DEBUG environment variable",
        setting: "this switch",
        off: "off",
      },
      chip: "debug",
    },
    devtools: {
      /** The webview decides inspector support when the window is created. */
      restartHint: "Developer tools need a restart of Meridian to become available.",
      restart: "Restart to enable",
    },
    logs: {
      directory: "Log folder",
      file: "Log file",
      reveal: "Show in Finder",
      title: "Recent log entries",
      refresh: "Refresh log",
      empty: "No log entries are available.",
      scrollLabel: "Log tail, scrollable",
    },
    crash: {
      title: "The previous run ended unexpectedly",
      recoveredTitle: "A command hit an internal error last run",
      uncleanTitle: "The previous run did not finish cleanly",
      kind: {
        panic: "a crash",
        recovered_panic: "a recovered internal error",
        unclean_exit: "an unclean exit",
      },
      dismiss: "Dismiss",
      location: "Location",
      backtrace: "Backtrace excerpt",
    },
    failures: {
      title: "Recent failures",
      empty: "No failures recorded.",
      report: "Report this problem",
      copy: "Copy details",
      copyReport: "Copy full report",
      command: "Command",
      occurred: "Occurred",
      detail: "Technical detail",
      focus: "Focused from the error you were shown",
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
      caption: "Recent commands, with their result and duration",
      scrollLabel: "Recent commands, scrollable",
    },
    actions: {
      copyReport: "Copy diagnostic report",
      export: "Export diagnostics file",
      openDevtools: "Open developer tools",
      report: "Report a problem",
      details: "Diagnostics",
      /** Shown when the toast cap hid failures; they are all in the recorder. */
      suppressed: "{count} more errors were recorded — open diagnostics to see them.",
      exported: "Diagnostics exported to {path}",
    },
    internalError: "An internal error occurred. Open diagnostics for details.",
  },
};

export default diagnostics;
export type DiagnosticsEn = typeof diagnostics;
