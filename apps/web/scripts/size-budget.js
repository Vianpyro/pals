// Fails the build when a page loads more than its budget before any interaction (spec D11).
// Usage: node scripts/size-budget.js <dist> <page.html>=<KB gzip>...
// Counts the page, its entry chunk, the chunks it imports statically and their CSS;
// chunks imported with import() load on demand and have budgets of their own.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

export function entryBytes(dist, page) {
    const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8"));
    if (!manifest[page]?.isEntry) throw new Error(`${page} is not an entry of this build`);
    const files = new Set([page]);
    const visit = (key) => {
        const chunk = manifest[key];
        if (files.has(chunk.file)) return;
        files.add(chunk.file);
        for (const css of chunk.css ?? []) files.add(css);
        for (const imported of chunk.imports ?? []) visit(imported);
    };
    visit(page);
    let bytes = 0;
    for (const file of files) bytes += gzipSync(readFileSync(join(dist, file)), { level: 9 }).length;
    return bytes;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    const [dist, ...budgets] = process.argv.slice(2);
    let over = false;
    for (const budget of budgets) {
        const [page, kb] = budget.split("=");
        const used = entryBytes(dist, page) / 1024;
        over ||= used > Number(kb);
        console.log(`${used > Number(kb) ? "OVER" : "ok  "}  ${page}  ${used.toFixed(1)} / ${kb} KB gzip`);
    }
    process.exitCode = over ? 1 : 0;
}
