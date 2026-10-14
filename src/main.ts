import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n, currentLocale } from "./i18n";
import { installGlobalErrorHandlers } from "./diagnostics/global";
import "./styles/tokens.css";
import "./styles/atmosphere.css";
import "./styles/base.css";

// Before mount, so the very first paint is already in the right language and
// the document advertises it for assistive technology and hyphenation.
document.documentElement.lang = currentLocale();

const app = createApp(App).use(createPinia()).use(i18n);
installGlobalErrorHandlers(app);
app.mount("#app");
