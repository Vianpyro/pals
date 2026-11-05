import assert from "node:assert/strict";
import { test } from "node:test";
import { argumentsOf, chooseLanguage, translator } from "./i18n.ts";

const AVAILABLE = ["en", "fr"];

test("the language chosen in the platform comes first", () => {
    assert.equal(chooseLanguage("fr", ["en-US"], AVAILABLE), "fr");
});

test("then the browser's, by language rather than region", () => {
    assert.equal(chooseLanguage(null, ["de-DE", "fr-CA", "en"], AVAILABLE), "fr");
});

test("a chosen language the platform lacks gives way to the browser's", () => {
    assert.equal(chooseLanguage("de", ["fr-CA"], AVAILABLE), "fr");
});

test("English when nothing matches", () => {
    assert.equal(chooseLanguage(null, ["de-DE"], AVAILABLE), "en");
});

const english = {
    greeting: "Hello, {name}.",
    runs: "{count, plural, one {# run} other {# runs}}",
    signOut: "Sign out",
};

test("formats a message in the chosen language, with its arguments", () => {
    const t = translator("fr", { greeting: "Bonjour, {name}." }, english);
    assert.equal(t("greeting", { name: "Céline" }), "Bonjour, Céline.");
});

test("plurals follow the language's rules", () => {
    const t = translator("fr", { runs: "{count, plural, one {# soumission} other {# soumissions}}" }, english);
    assert.equal(t("runs", { count: 0 }), "0 soumission"); // French: 0 is singular
    assert.equal(t("runs", { count: 2 }), "2 soumissions");
});

test("a key missing from a contributed language falls back to English", () => {
    const t = translator("de", { greeting: "Hallo, {name}." }, english);
    assert.equal(t("signOut"), "Sign out");
});

test("finds the arguments a message expects, inside plurals too", () => {
    assert.deepEqual(argumentsOf("{at, date, long}: {count, plural, one {# run of <b>{who}</b>} other {#}}"), [
        "at",
        "count",
        "who",
    ]);
});
