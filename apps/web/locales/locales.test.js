// The translations the project maintains (ADR-0011): French has exactly English's keys, and every
// message parses and expects the same arguments as its English source. A contributed language
// added here may lack keys, which show in English, but never misuse one.
import { argumentsOf } from "@log791/ui/i18n";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

const read = (language) => JSON.parse(readFileSync(new URL(`${language}.json`, import.meta.url), "utf8"));
const english = read("en");
const maintained = ["fr"];
const languages = readdirSync(new URL(".", import.meta.url))
    .filter((file) => file.endsWith(".json") && file !== "en.json")
    .map((file) => file.replace(/\.json$/, ""));

for (const language of maintained) {
    test(`${language} has exactly English's keys`, () => {
        assert.deepEqual(Object.keys(read(language)).sort(), Object.keys(english).sort());
    });
}

for (const language of ["en", ...languages]) {
    test(`${language}: every message parses and expects English's arguments`, () => {
        for (const [key, message] of Object.entries(read(language))) {
            assert.ok(key in english, `${key} is not an English key`);
            assert.deepEqual(argumentsOf(message).sort(), argumentsOf(english[key]).sort(), key);
        }
    });
}
