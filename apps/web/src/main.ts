import { mount } from "svelte";
import App from "./App.svelte";
import { loadTranslator } from "./lib/i18n.ts";

const { language, t } = await loadTranslator();
// Screen readers pronounce the page in the language it is written in (WCAG 3.1.1)
document.documentElement.lang = language;
mount(App, { target: document.body, props: { t } });
