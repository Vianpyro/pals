<script lang="ts">
    import type { Run } from "@log791/contracts/Run";
    import { draft, history, progress, saveDraft, submit } from "@log791/ui/api";
    import { onMount } from "svelte";
    import type { Cases } from "./lib/cases.ts";
    import { Autosave, type SaveState } from "./lib/draft.ts";
    import type { Translate } from "./lib/i18n.ts";
    import { caseText, failureText, isSignedOut, runText, saveText } from "./lib/messages.ts";

    let {
        t,
        say,
        course,
        live,
        cases,
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
        /** The visible cases the event stream reported, by run (App.svelte) */
        cases: Cases;
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
    // The newest run's: the stream replays only the latest run of each exercise (ADR-0030)
    const results = $derived(shown.length > 0 ? (cases[shown[0].id] ?? []) : []);
    // The run just submitted, whose progress the status line tells
    let followed: string | null = $state(null);
    let source = $state("");
    let sending = $state(false);
    // Read-only until the server's copy arrives, which would replace anything typed before
    let loading = $state(true);
    let draftState: SaveState = $state({ kind: "saved" });
    let saver: Autosave | undefined;

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

    function showDraftState(state: SaveState) {
        draftState = state;
        if (state.kind === "conflict") return say(t("draft.conflict"));
        if (state.kind !== "failed") return;
        if (!isSignedOut(state.failure)) return say(failureText(t, state.failure, { 403: "exercise.notOpen", 422: "draft.tooLarge" }));
        say(t("submission.sessionExpired"));
        onSignedOut();
    }

    async function openDraft() {
        const outcome = await draft(course, id);
        if (!outcome.ok && isSignedOut(outcome.failure)) return onSignedOut();
        if (!outcome.ok) say(failureText(t, outcome.failure, { 403: "exercise.notOpen", 404: "exercise.notFound" }));
        // Without the server's copy, from revision 0: a save over the one it holds then shows both (ADR-0038)
        const start = outcome.ok ? outcome.value : { revision: 0, source: "" };
        source = start.source;
        saver = new Autosave(start, (write) => saveDraft(course, id, write), showDraftState);
        loading = false;
    }

    function takeSaved() {
        if (saver) source = saver.takeTheirs();
    }

    /** Sends the draft now: before signing out, or before another exercise reads its own (App.svelte) */
    export function flush() {
        return saver?.flush();
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
        // The submitted code is also the draft to find again; the submission does not wait for it
        void saver?.flush();
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

    onMount(() => {
        void showHistory();
        void openDraft();
        return () => void saver?.close();
    });
</script>

<section>
    <!-- Text, never HTML: the statement comes from untrusted content (ADR-0023) -->
    <pre>{statement}</pre>
    <form onsubmit={send}>
        <label for="source">{t("exercise.source", { language: LANGUAGE })}</label><br />
        <textarea
            id="source"
            rows="15"
            cols="80"
            spellcheck="false"
            required
            readonly={loading}
            aria-describedby="draft-state"
            bind:value={source}
            oninput={(event) => saver?.edit(event.currentTarget.value)}
        ></textarea>
        <p id="draft-state">{loading ? "" : saveText(t, draftState)}</p>
        <button disabled={sending}>{t("exercise.submit")}</button>
    </form>

    {#if draftState.kind === "conflict"}
        <!-- Both versions shown, nothing replaced until the student chooses (ADR-0038) -->
        <section aria-labelledby="conflict-title">
            <h3 id="conflict-title">{t("draft.conflictTitle")}</h3>
            <p>{t("draft.conflictHelp")}</p>
            <label for="saved-version">{t("draft.savedVersion")}</label><br />
            <textarea id="saved-version" rows="15" cols="80" spellcheck="false" readonly value={draftState.current.source}></textarea><br />
            <button type="button" onclick={() => saver?.keepMine()}>{t("draft.keepMine")}</button>
            <button type="button" onclick={takeSaved}>{t("draft.takeSaved")}</button>
        </section>
    {/if}

    {#if results.length > 0}
        <h3 id="cases-title">{t("cases.title")}</h3>
        <ul aria-labelledby="cases-title">
            {#each results as result (result.position)}
                <li>
                    {t("case.result", { number: result.position + 1, verdict: caseText(t, result.verdict) })}
                    <!-- Text, never HTML: the program's output is the student's -->
                    {#if result.actual === ""}
                        <p>{t("case.noOutput")}</p>
                    {:else}
                        <pre>{result.actual}</pre>
                    {/if}
                    {#if result.truncated}
                        <p>{t("case.truncated")}</p>
                    {/if}
                </li>
            {/each}
        </ul>
    {/if}

    <h3 id="history-title">{t("exercise.history")}</h3>
    <!-- reversed: the newest run, listed first, carries the highest number -->
    <ol reversed aria-labelledby="history-title">
        {#each shown as run (run.id)}
            <li>{t("exercise.run", { at: new Date(run.enqueued_at), status: runText(t, run) })}</li>
        {/each}
    </ol>
</section>
