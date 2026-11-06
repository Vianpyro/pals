// What the demo keeps, as JSON in the browser's storage.
import type { KeyValues } from "./storage.ts";

// The origin is shared with the account's other GitHub Pages projects
const PREFIX = "pals-demo:";

export type Records = ReturnType<typeof createRecords>;

export function createRecords(storage: KeyValues) {
    return {
        /** Null when absent, or when what is stored there is not JSON. */
        read<T>(key: string): T | null {
            try {
                return JSON.parse(storage.getItem(PREFIX + key) ?? "null") as T | null;
            } catch {
                return null;
            }
        },

        /** False when the browser has no room left. */
        write(key: string, value: unknown): boolean {
            try {
                storage.setItem(PREFIX + key, JSON.stringify(value));
                return true;
            } catch {
                return false;
            }
        },

        remove: (key: string) => storage.removeItem(PREFIX + key),
    };
}
