// Python in the browser (ADR-0008). Not the sandbox of spec D15: this page holds no session and no secret.
import harness from "./harness.py?raw";
import type { WorkerMessage } from "./python.ts";

// CPython 3.14, when the judge runs 3.15: the page's banner says so (vite.demo.config.js).
// shortcut: from a CDN with no checksum, since a demo has nothing to steal; the manifest of spec D24 if this ever serves an exam
const PYODIDE = "https://cdn.jsdelivr.net/pyodide/v314.0.7/full/pyodide.mjs";

const say = (message: WorkerMessage) => self.postMessage(message);

try {
    const { loadPyodide } = await import(/* @vite-ignore */ PYODIDE);
    const pyodide = await loadPyodide();
    const run: (source: string, input: string) => string = pyodide.runPython(harness);

    self.onmessage = ({ data: { source, input } }: MessageEvent<{ source: string; input: string }>) => {
        try {
            const [success, stdout] = JSON.parse(run(source, input)) as [boolean, string];
            say({ kind: "exited", success, stdout });
        } catch {
            // The harness catches what the program raises: this is the interpreter itself failing
            say({ kind: "broken" });
        }
    };
    say({ kind: "ready" });
} catch {
    say({ kind: "load-failed" });
}
