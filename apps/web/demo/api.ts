// Stands in for @log791/ui/api in the demo's build (vite.demo.config.js)
import { server } from "./instance.ts";

export const { signIn, signOut, catalog, statement, history, draft, saveDraft, submit, progress } = server.api;
