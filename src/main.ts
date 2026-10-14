import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n, currentLocale } from "./i18n";
import "./styles/tokens.css";
import "./styles/base.css";

// Before mount, so the very first paint is already in the right language and
// the document advertises it for assistive technology and hyphenation.
document.documentElement.lang = currentLocale();

createApp(App).use(createPinia()).use(i18n).mount("#app");
