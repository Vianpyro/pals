<script lang="ts">
    import type { Run } from "@log791/contracts/Run";
    import { history, progress, submit } from "@log791/ui/api";
    import { onMount } from "svelte";
    import type { Translate } from "./lib/i18n.ts";
    import { failureText, isSignedOut, runText } from "./lib/messages.ts";

    let {
        t,
        say,
        course,
        live,
        streaming,
        id,
        statement,
        onSignedOut,
    }: {
        t: Translate;
        say: (text: string) => void;
        course: string;
        /** The runs as the event stream last told them, by id (App.svelte) */
        live: Record<string, Run>;
        /** False once the stream stopped for good: then this page polls (ADR-0008) */
        streaming: boolean;
        id: string;
        statement: string;
        onSignedOut: () => void;
    } = $props();

    // shortcut: the judge has only the Python pack until packs are data (ADR-0013)
    const PACK = "python";
    const LANGUAGE = "Python";
    const POLL_MS = 1000;

    let runs: Run[] = $state([]);
    // The history as last read, each run replaced by what the stream told of it since
    const shown = $derived(runs.map((run) => live[run.id] ?? run));
    // The run just submitted, whose progress the status line tells
    let followed: string | null = $state(null);
    let source = $state("");
    let sending = $state(false);

    const judged = (run: Run) => run.state !== "waiting" && run.state !== "running";

    $effect(() => {
        const run = followed === null ? undefined : live[followed];
        if (!run) return;
        say(runText(t, run));
        if (judged(run)) followed = null;
    });

    async function showHistory() {
        const outcome = await history(course, id);
        if (!outcome.ok && isSignedOut(outcome.failure)) return onSignedOut();
        // Emptied on error too: the list of the exercise shown before must not stay under this one
        runs = outcome.ok ? outcome.value : [];
        if (!outcome.ok) {
            const { failure } = outcome;
            say(failure.kind === "status" ? t("exercise.historyUnavailable", { status: failure.status }) : failureText(t, failure));
        }
    }

    // Only when the event stream stopped for good (ADR-0008)
    async function poll(run: string) {
        for (;;) {
            const outcome = await progress(run);
            if (!outcome.ok) return say(failureText(t, outcome.failure));
            say(runText(t, outcome.value));
            if (outcome.value.state !== "waiting" && outcome.value.state !== "running") return;
            await new Promise((resolve) => setTimeout(resolve, POLL_MS));
        }
    }

    async function send(event: SubmitEvent) {
        event.preventDefault();
        sending = true;
        try {
            const outcome = await submit({ course, exercise: id, language_pack: PACK, source });
            if (!outcome.ok) {
                if (isSignedOut(outcome.failure)) {
                    say(t("submission.sessionExpired"));
                    return onSignedOut();
                }
                say(failureText(t, outcome.failure, { 403: "exercise.notOpen", 404: "exercise.notFound", 422: "submission.invalid" }));
                return;
            }
            say(t("run.waiting"));
            if (streaming) {
                // The stream tells the rest, in the status line and in the history
                followed = outcome.value.id;
                await showHistory();
            } else {
                await poll(outcome.value.id);
                await showHistory();
            }
        } finally {
            sending = false;
        }
    }

    onMount(showHistory);
</script>

<section>
    <!-- Text, never HTML: the statement comes from untrusted content (ADR-0023) -->
    <pre>{statement}</pre>
    <form onsubmit={send}>
        <label for="source">{t("exercise.source", { language: LANGUAGE })}</label><br />
        <textarea id="source" rows="15" cols="80" spellcheck="false" required bind:value={source}></textarea><br />
        <button disabled={sending}>{t("exercise.submit")}</button>
    </form>

    <h3 id="history-title">{t("exercise.history")}</h3>
    <!-- reversed: the newest run, listed first, carries the highest number -->
    <ol reversed aria-labelledby="history-title">
        {#each shown as run (run.id)}
            <li>{t("exercise.run", { at: new Date(run.enqueued_at), status: runText(t, run) })}</li>
        {/each}
    </ol>
</section>
