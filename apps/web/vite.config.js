import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
    root: "src",
    plugins: [svelte()],
    build: {
        outDir: "../dist",
        emptyOutDir: true,
        // adoptedStyleSheets, which a strict style-src needs (spec D6, D10)
        target: ["safari16.4", "chrome111"],
        // Read by scripts/size-budget.js
        manifest: true,
        // Every asset stays a file: fonts and styles inlined as data: URIs would break the CSP
        assetsInlineLimit: 0,
    },
    server: {
        // Reachable through the dev container's forwarded port, in VS Code and in Zed
        host: true,
        // The API behind /api/, as nginx does; the dev server sends no CSP, the web service does
        proxy: {
            "/api": { target: "http://localhost:8080", rewrite: (path) => path.replace(/^\/api/, "") },
        },
    },
});
