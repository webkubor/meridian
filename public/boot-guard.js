/**
 * The guard in front of the app bundle.
 *
 * A module that fails to load, a syntax error, or a blank screen leaves the
 * owner with a window and no explanation — the failure handlers in `main.ts`
 * are installed *after* the imports, so anything that breaks earlier is silent.
 * This file runs before the bundle: it keeps a static panel on screen until the
 * app takes over, and renders the first error it sees into that panel, with a
 * copy button.
 *
 * Plain ES5-compatible script on purpose: it has to work when the module graph
 * it protects never runs at all. The copy into the local log is best-effort and
 * depends on the Tauri bridge, so it is wrapped and may be absent (the
 * browser-based test environment has no bridge).
 */
(function () {
  var el = document.getElementById("boot");
  var heading = document.getElementById("boot-message");
  var copy = document.getElementById("boot-copy");
  var state = { ready: false, messages: [] };

  function say(text) {
    state.messages.push(text);
    if (heading) heading.textContent = String(text).slice(0, 400);
    if (el) el.hidden = false;
    writeToLog(text);
  }

  function writeToLog(text) {
    try {
      var bridge = window.__TAURI_INTERNALS__;
      if (!bridge || typeof bridge.invoke !== "function") return;
      bridge.invoke("diagnostics_log_write", {
        args: { level: "error", message: "boot: " + String(text), source: "boot-guard" },
      });
    } catch (error) {
      /* No bridge yet, or it refuses; the panel is the report either way. */
    }
  }

  window.addEventListener("error", function (event) {
    if (state.ready) return;
    say((event && (event.message || (event.error && event.error.message))) || "Script failed to load.");
  });

  window.addEventListener("unhandledrejection", function (event) {
    if (state.ready) return;
    var reason = event && event.reason;
    say((reason && (reason.message || reason.toString())) || "A promise was rejected before the app started.");
  });

  if (copy) {
    copy.addEventListener("click", function () {
      var text = state.messages.join("\n\n") || "Meridian did not finish starting.";
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).catch(function () {
          if (heading) heading.textContent = text;
        });
      } else if (heading) {
        heading.textContent = text;
      }
    });
  }

  // The panel shows what happened while starting; the app calls this and it goes.
  window.__meridianReady = function () {
    state.ready = true;
    if (el) el.hidden = true;
  };
})();
