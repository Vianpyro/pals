// The student's page with no server, for GitHub Pages: the same components, with the API client
// replaced by demo/. Only this build imports demo/, so none of it reaches production.
import { fileURLToPath } from "node:url";
import { defineConfig, mergeConfig } from "vite";
import base from "./vite.config.js";

const demo = (file) => fileURLToPath(new URL(`demo/${file}`, import.meta.url));

// The account is demo/server.ts's. Said plainly, or a visitor would read a Pyodide result as the judge's.
const banner = {
    name: "demo-banner",
    transformIndexHtml: () => [
        {
            tag: "aside",
            injectTo: "body-prepend",
            children: [
                {
                    tag: "p",
                    attrs: { lang: "fr" },
                    children:
                        "Démo sans serveur : votre code s'exécute dans votre navigateur avec Python 3.14 (Pyodide), pas sur le juge de la plateforme. Compte de démonstration : etudiant1 / log791-dev. N'entrez pas de vrai mot de passe.",
                },
                {
                    tag: "p",
                    attrs: { lang: "en" },
                    children:
                        "Serverless demo: your code runs in your browser on Python 3.14 (Pyodide), not on the platform's judge. Demo account: etudiant1 / log791-dev. Do not enter a real password.",
                },
            ],
        },
    ],
};

export default mergeConfig(
    base,
    defineConfig({
        // Relative: the site is served from a subfolder, /pals/demo/
        base: "./",
        plugins: [banner],
        resolve: {
            alias: {
                "@log791/ui/api": demo("api.ts"),
                "@log791/ui/events": demo("events.ts"),
            },
        },
        build: { outDir: "../dist-demo", manifest: false },
        // python.worker.ts imports Pyodide as a module
        worker: { format: "es" },
    }),
);
