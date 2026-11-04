import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import { entryBytes } from "./size-budget.js";

// A build as Vite writes it: an entry, a shared chunk it imports, a CSS file and a lazy chunk
function dist() {
    const dir = mkdtempSync(join(tmpdir(), "budget-"));
    mkdirSync(join(dir, "assets"));
    mkdirSync(join(dir, ".vite"));
    const files = {
        "index.html": "<!doctype html><script type=module src=/assets/main.js></script>",
        "assets/main.js": "import './shared.js';".repeat(50),
        "assets/shared.js": "export const shared = 1;".repeat(80),
        "assets/main.css": "body { color: black }".repeat(30),
        "assets/editor.js": "x".repeat(100_000),
    };
    for (const [path, text] of Object.entries(files)) writeFileSync(join(dir, path), text);
    writeFileSync(join(dir, ".vite/manifest.json"), JSON.stringify({
        "index.html": {
            file: "assets/main.js",
            isEntry: true,
            imports: ["_shared.js"],
            dynamicImports: ["editor.ts"],
            css: ["assets/main.css"],
        },
        "_shared.js": { file: "assets/shared.js" },
        "editor.ts": { file: "assets/editor.js", isDynamicEntry: true },
    }));
    const gz = (path) => gzipSync(files[path], { level: 9 }).length;
    return { dir, gz };
}

test("counts the page, its entry, static imports and CSS, gzipped; not what loads on demand", () => {
    const { dir, gz } = dist();
    const expected = gz("index.html") + gz("assets/main.js") + gz("assets/shared.js") + gz("assets/main.css");
    assert.equal(entryBytes(dir, "index.html"), expected);
});

test("names an entry missing from the build", () => {
    const { dir } = dist();
    assert.throws(() => entryBytes(dir, "staff/index.html"), /staff\/index\.html/);
});
