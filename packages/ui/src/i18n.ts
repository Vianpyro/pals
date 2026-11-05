// Interface strings by key, one ICU MessageFormat file per language (ADR-0011).
// English is the source and is complete; another language may lack keys, which then show in English.
import { IntlMessageFormat } from "intl-messageformat";

export type Messages = Readonly<Record<string, string>>;
type Values = Record<string, string | number | Date>;

const SOURCE = "en";

/**
 * The first of: the language set in the platform, the browser's, English (ADR-0011).
 * A region is ignored: fr-CA is fr. The platform's own setting comes first because
 * Safe Exam Browser does not pass on the student's.
 */
export function chooseLanguage(chosen: string | null, browser: readonly string[], available: readonly string[]): string {
    for (const tag of [chosen, ...browser]) {
        const language = tag?.toLowerCase().split("-")[0];
        if (language && available.includes(language)) return language;
    }
    return SOURCE;
}

/**
 * Formats keys of `english` in `language`. The type of `english` names the keys, so a key that
 * does not exist fails to compile; a key the language lacks shows its English text.
 */
export function translator<Key extends string>(language: string, messages: Messages, english: Readonly<Record<Key, string>>) {
    const formats = new Map<Key, IntlMessageFormat>();
    return (key: Key, values?: Values): string => {
        let format = formats.get(key);
        if (!format) {
            const own = Object.hasOwn(messages, key);
            format = new IntlMessageFormat(own ? messages[key] : english[key], own ? language : SOURCE);
            formats.set(key, format);
        }
        return String(format.format(values));
    };
}

/** The names a message expects, in order: a translation must use the same ones as English. */
export function argumentsOf(message: string): string[] {
    const names = new Set<string>();
    const walk = (elements: ReturnType<IntlMessageFormat["getAst"]>) => {
        for (const element of elements) {
            // Every element with a value names an argument, except literal text (0) and tags (8)
            if ("value" in element && element.type !== 0 && element.type !== 8) names.add(String(element.value));
            if ("options" in element) for (const option of Object.values(element.options)) walk(option.value);
            if ("children" in element) walk(element.children);
        }
    };
    walk(new IntlMessageFormat(message, SOURCE).getAst());
    return [...names];
}
