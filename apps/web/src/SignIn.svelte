<script lang="ts">
    import type { Identity } from "@log791/contracts/Identity";
    import { signIn } from "@log791/ui/api";
    import type { Translate } from "./lib/i18n.ts";
    import { failureText } from "./lib/messages.ts";

    let {
        t,
        say,
        onSignedIn,
    }: { t: Translate; say: (text: string) => void; onSignedIn: (identity: Identity) => void } = $props();

    let id = $state("");
    let password = $state("");

    async function submit(event: SubmitEvent) {
        event.preventDefault();
        const outcome = await signIn({ id, password });
        if (!outcome.ok) {
            say(failureText(t, outcome.failure, { 401: "signIn.refused", 503: "signIn.directoryDown" }));
            return;
        }
        id = "";
        password = "";
        onSignedIn(outcome.value);
    }
</script>

<form onsubmit={submit}>
    <label>{t("signIn.id")} <input name="id" autocomplete="username" required bind:value={id} /></label>
    <label>
        {t("signIn.password")}
        <input name="password" type="password" autocomplete="current-password" required bind:value={password} />
    </label>
    <button>{t("signIn.submit")}</button>
</form>
