// A student's drafts, with the revisions of ADR-0038: a write based on an older revision is refused.
import type { Draft } from "@log791/contracts/Draft";
import type { DraftWrite } from "@log791/contracts/DraftWrite";
import type { DraftSave } from "@log791/ui/api";
import type { Records } from "./records.ts";

const NEVER_WRITTEN: Draft = { revision: 0, source: "" };

export const readDraft = (records: Records, key: string) => records.read<Draft>(key) ?? NEVER_WRITTEN;

/** Null when the browser has no room left for it. */
export function writeDraft(records: Records, key: string, { base, source }: DraftWrite): DraftSave | null {
    const current = readDraft(records, key);
    if (base !== current.revision) return { kind: "conflict", current };
    const revision = current.revision + 1;
    return records.write(key, { revision, source }) ? { kind: "saved", revision } : null;
}
