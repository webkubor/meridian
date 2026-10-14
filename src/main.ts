import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n, currentLocale } from "./i18n";
import { captureGlobalError, installGlobalErrorHandlers } from "./diagnostics/global";
import { installFailureLog } from "./diagnostics/logSink";
import "./styles/tokens.css";
import "./styles/atmosphere.css";
import "./styles/base.css";

// Before mount, so the very first paint is already in the right language and
// the document advertises it for assistive technology and hyphenation.
document.documentElement.lang = currentLocale();

// The log sink is installed before the first composable runs, so a failure
// during bootstrap is written to the local log immediately. The *toast* sink is
// deliberately left unset until `App.vue` mounts: a failure raised before then
// stays pending and is shown as soon as the shell can take it, which is the
// only way it reaches the user at all. (The static panel in `index.html` covers
// the window before this module runs.)
installFailureLog();

declare global {
  interface Window {
    __meridianReady?: () => void;
  }
}

/**
 * Mount inside the guard, so a throw during setup or the first render leaves a
 * message on screen instead of a blank window. `captureGlobalError` records it
 * and writes it to the local log; the static panel in `index.html` is what the
 * owner reads.
 */
try {
  const app = createApp(App).use(createPinia()).use(i18n);
  installGlobalErrorHandlers(app);
  app.config.errorHandler = (error, _instance, info) => {
    captureGlobalError(error, `vue:${info}`);
    window.__meridianReady?.();
  };
  app.mount("#app");
  window.__meridianReady?.();
} catch (error) {
  captureGlobalError(error, "boot");
  const panel = document.getElementById("boot-message");
  if (panel) panel.textContent = error instanceof Error ? error.message : String(error);
  const boot = document.getElementById("boot");
  if (boot) boot.hidden = false;
}
