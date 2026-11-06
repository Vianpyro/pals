// The demo's one server, shared by api.ts and events.ts, which the demo's build puts in place of the real client.
import { readExercises } from "./content.ts";
import { createPython } from "./python.ts";
import { createServer } from "./server.ts";
import { browserStorage } from "./storage.ts";

const files = import.meta.glob<string>(
    ["../../../examples/content/exercises/*/statement.md", "../../../examples/content/exercises/*/assessment/*/*.{in,out}"],
    { query: "?raw", import: "default", eager: true },
);

const python = createPython(
    () => new Worker(new URL("./python.worker.ts", import.meta.url), { type: "module" }),
    // runMs: the judge's limit per case (apps/judge/src/main.rs); loadMs: about 12 MB to fetch on a slow network
    { runMs: 10_000, loadMs: 120_000 },
);

// The course .devcontainer/demo.sh publishes the examples as, and App.svelte opens by default
export const server = createServer(browserStorage(), { course: "log200", exercises: readExercises(files) }, python);
