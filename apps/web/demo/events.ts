// Stands in for @log791/ui/events in the demo's build (vite.demo.config.js)
import type * as Events from "@log791/ui/events";
import { server } from "./instance.ts";

// Typed as the real one: the type checker refuses any drift from it
export const followRuns: typeof Events.followRuns = server.followRuns;
