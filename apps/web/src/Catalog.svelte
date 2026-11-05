<script lang="ts">
    import type { CatalogEntry } from "@log791/contracts/CatalogEntry";
    import type { Translate } from "./lib/i18n.ts";
    import { availabilityText } from "./lib/messages.ts";

    let {
        t,
        entries,
        labelledBy,
        onOpen,
    }: { t: Translate; entries: CatalogEntry[]; labelledBy: string; onOpen: (id: string) => void } = $props();

    // One instant per list: two exercises never disagree about what time it is
    const now = $derived.by(() => {
        void entries;
        return new Date();
    });
</script>

<!-- Identifiers come from course content: Svelte inserts them as text, never as HTML (ADR-0023) -->
<ul aria-labelledby={labelledBy}>
    {#each entries as entry (entry.id)}
        {@const note = availabilityText(t, entry, now)}
        <li>
            <button type="button" disabled={!entry.open} onclick={() => onOpen(entry.id)}>{entry.id}</button>
            {#if note}— {note}{/if}
        </li>
    {/each}
</ul>
