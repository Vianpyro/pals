// The page's language and messages (ADR-0011). English is bundled, since every key falls back to it;
// another language is a chunk of its own, loaded only by the students who read it.
import { chooseLanguage, translator, type Messages } from "@log791/ui/i18n";
import english from "../../locales/en.json";

export type Key = keyof typeof english;
export type Translate = (key: Key, values?: Record<string, string | number | Date>) => string;

const others = import.meta.glob<Messages>(["../../locales/*.json", "!../../locales/en.json"], { import: "default" });
const languageOf = (path: string) => path.replace(/^.*\/(.+)\.json$/, "$1");

export async function loadTranslator(): Promise<{ language: string; t: Translate }> {
    const available = ["en", ...Object.keys(others).map(languageOf)];
    // shortcut: no language setting in the platform yet, so the browser's; the account will hold it (ADR-0011)
    const language = chooseLanguage(null, navigator.languages, available);
    const load = Object.entries(others).find(([path]) => languageOf(path) === language)?.[1];
    try {
        return { language, t: translator<Key>(language, load ? await load() : english, english) };
    } catch {
        // The language's file did not arrive: English beats a blank page
        return { language: "en", t: translator<Key>("en", english, english) };
    }
}
