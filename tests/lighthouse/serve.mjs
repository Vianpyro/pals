// The demo as GitHub Pages serves it: plain files under a subfolder, compressed, with no API behind
// them. Uncompressed files would cost the page a load time no visitor sees.
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";

const TYPES = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".mjs": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".json": "application/json",
    ".svg": "image/svg+xml",
    ".wasm": "application/wasm",
    ".png": "image/png",
    ".ico": "image/x-icon",
    ".woff2": "font/woff2",
    ".txt": "text/plain; charset=utf-8",
    ".py": "text/plain; charset=utf-8",
};
const COMPRESSIBLE = /^(text\/|application\/(json|wasm)|image\/svg)/;

/**
 * Serves the files of root under prefix, on a free port of the loopback.
 * @param {string} root @param {string} prefix "/demo/"
 * @returns {Promise<{url: string, close: () => Promise<void>}>}
 */
export function serve(root, prefix) {
    const base = resolve(root);
    const server = createServer(async (request, response) => {
        const { pathname } = new URL(request.url ?? "/", "http://localhost");
        if (!pathname.startsWith(prefix)) return refuse(response);
        let file = resolve(base, `.${decodeURIComponent(pathname.slice(prefix.length - 1))}`);
        // ../ out of the folder: nothing outside it is served
        if (file !== base && !file.startsWith(base + sep)) return refuse(response);
        if (pathname.endsWith("/")) file = join(file, "index.html");

        let body;
        try {
            body = await readFile(file);
        } catch {
            return refuse(response);
        }
        const type = TYPES[extname(file)] ?? "application/octet-stream";
        const headers = { "Content-Type": type, "Cache-Control": "max-age=600", Vary: "Accept-Encoding" };
        if (COMPRESSIBLE.test(type) && /\bgzip\b/.test(request.headers["accept-encoding"] ?? "")) {
            body = gzipSync(body);
            headers["Content-Encoding"] = "gzip";
        }
        response.writeHead(200, { ...headers, "Content-Length": body.length });
        response.end(request.method === "HEAD" ? undefined : body);
    });

    return new Promise((ready) => {
        // 127.0.0.1, not localhost: localhost may resolve to ::1, where nothing listens
        server.listen(0, "127.0.0.1", () => {
            const { port } = /** @type {import("node:net").AddressInfo} */ (server.address());
            ready({
                url: `http://127.0.0.1:${port}${prefix}`,
                close: () => new Promise((closed) => server.close(() => closed())),
            });
        });
    });
}

/** @param {import("node:http").ServerResponse} response */
function refuse(response) {
    response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Not found\n");
}
