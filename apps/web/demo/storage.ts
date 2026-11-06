// Where the demo keeps what a server would.

export type KeyValues = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/** Lost on reload: for tests, and for a browser that refuses localStorage. */
export function memoryStorage(): KeyValues {
    const items = new Map<string, string>();
    return {
        getItem: (key) => items.get(key) ?? null,
        setItem: (key, value) => void items.set(key, value),
        removeItem: (key) => void items.delete(key),
    };
}

/** The browser's storage when it allows it: reading `localStorage` throws when site data is blocked. */
export function browserStorage(): KeyValues {
    try {
        return localStorage;
    } catch {
        return memoryStorage();
    }
}
