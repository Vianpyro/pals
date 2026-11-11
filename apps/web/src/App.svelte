<script lang="ts">
    import type { CatalogEntry } from "@log791/contracts/CatalogEntry";
    import type { Identity } from "@log791/contracts/Identity";
    import type { Run } from "@log791/contracts/Run";
    import { catalog, signOut, statement } from "@log791/ui/api";
    import { followRuns } from "@log791/ui/events";
    import { onMount } from "svelte";
    import Catalog from "./Catalog.svelte";
    import Exercise from "./Exercise.svelte";
    import SignIn from "./SignIn.svelte";
    import { withCase, withRun, type Cases } from "./lib/cases.ts";
    import type { Translate } from "./lib/i18n.ts";
    import { failureText, isSignedOut } from "./lib/messages.ts";

    let { t }: { t: Translate } = $props();

    // shortcut: one offering picked by the URL, until the API lists a student's offerings (ADR-0012)
    const offering = new URLSearchParams(location.search).get("offering") ?? "log200-a26";

    let view: "loading" | "sign-in" | "course" = $state("loading");
    let entries: CatalogEntry[] = $state([]);
    // A new object on every opening, so the exercise reloads its history even when it is the same one
    let exercise: { id: string; statement: string } | null = $state(null);
    let shown: { flush(): Promise<void> | undefined } | undefined = $state();
    let status = $state("");
    const say = (text: string) => (status = text);

    // The student's runs as the event stream last told them, by id; one stream per tab (ADR-0008)
    let live: Record<string, Run> = $state({});
    let cases: Cases = $state({});
    let streaming = $state(true);
    $effect(() => {
        if (view !== "course") return;
        streaming = true;
        const stream = followRuns({
            onRun: (run) => {
                live[run.id] = run;
                cases = withRun(cases, run);
            },
            onCase: (result) => (cases = withCase(cases, result)),
            onClosed: () => (streaming = false),
        });
        return () => stream.close();
    });

    async function openCourse() {
        const outcome = await catalog(offering);
        if (!outcome.ok) {
            if (isSignedOut(outcome.failure)) view = "sign-in";
            else say(failureText(t, outcome.failure, { 403: "catalog.notEnrolled", 404: "catalog.unknownCourse" }));
            return;
        }
        entries = outcome.value;
        if (entries.length === 0) say(t("catalog.empty"));
        view = "course";
    }

    async function openExercise(id: string) {
        // Saved first: reopening the same exercise must read this draft, not the one before it
        await shown?.flush();
        const outcome = await statement(offering, id);
        if (!outcome.ok) {
            if (isSignedOut(outcome.failure)) view = "sign-in";
            else say(failureText(t, outcome.failure, { 403: "exercise.notOpen", 404: "exercise.notFound" }));
            return;
        }
        exercise = { id, statement: outcome.value };
        say("");
    }

    async function signedIn(identity: Identity) {
        say(t("signIn.greeting", { name: identity.name ?? identity.id }));
        await openCourse();
    }

    async function leave() {
        // While the session still lets it be saved
        await shown?.flush();
        await signOut();
        say("");
        exercise = null;
        view = "sign-in";
    }

    onMount(openCourse);
</script>

<main>
    <h1>P.A.L.S.</h1>

    {#if view === "sign-in"}
        <SignIn {t} {say} onSignedIn={signedIn} />
    {:else if view === "course"}
        <section>
            <button type="button" onclick={leave}>{t("signOut")}</button>
            <h2 id="catalog-title">{t("catalog.title")}</h2>
            <Catalog {t} {entries} labelledBy="catalog-title" onOpen={openExercise} />

            {#if exercise}
                {#key exercise}
                    <Exercise
                        bind:this={shown}
                        {t}
                        {say}
                        {offering}
                        {live}
                        {cases}
                        {streaming}
                        id={exercise.id}
                        statement={exercise.statement}
                        onSignedOut={() => (view = "sign-in")}
                    />
                {/key}
            {/if}
        </section>
    {/if}

    <p role="status">{status}</p>
</main>
