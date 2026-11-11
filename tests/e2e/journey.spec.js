// The student's journey against the real API and judge: the safety net of the Svelte rewrite (spec D14).
// Elements are found by role and accessible name, which the rewrite must keep.
import { expect, expectAccessible, signIn, test } from "./fixtures.js";

const SOLUTION = "import sys\nprint(sum(map(int, sys.stdin.read().split())))\n";
// The judge's first run of the day starts its sandbox: allow more than the default 5 s
const VERDICT = { timeout: 60_000 };

async function openSum(page) {
    await page.getByRole("button", { name: "sum" }).click();
    await expect(page.getByText("Lire deux entiers sur une ligne")).toBeVisible();
}

test("the sign-in form and the catalog are accessible", async ({ page, student }) => {
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
    await expectAccessible(page);

    await signIn(page, student);
    await expect(page.getByRole("list", { name: "Exercices" }).getByRole("button", { name: "sum" })).toBeEnabled();
    await expectAccessible(page);
});

test("a wrong password is refused", async ({ page, student }) => {
    await page.goto("/");
    await page.getByLabel("Identifiant").fill(student);
    await page.getByLabel("Mot de passe").fill("not-the-password");
    await page.getByRole("button", { name: "Se connecter" }).click();
    await expect(page.getByRole("status")).toHaveText("Identifiant ou mot de passe refusé.");
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
});

test("an account without a role in the offering is told so", async ({ page }) => {
    await page.goto("/");
    // Signs in, but demo.sh leaves it off the offering's roster
    await page.getByLabel("Identifiant").fill("admin1");
    await page.getByLabel("Mot de passe").fill("log791-dev");
    await page.getByRole("button", { name: "Se connecter" }).click();
    await expect(page.getByRole("status")).toHaveText("Vous n'êtes pas inscrit à ce cours.");
});

test("a correct program passes and is listed first in the history", async ({ page, student }) => {
    test.slow(); // the judge runs every case in a sandbox
    // The verdict comes over the event stream, through nginx: nothing polls the run
    const polled = [];
    page.on("request", (request) => {
        if (/\/api\/submissions\/[0-9a-f-]{36}$/.test(request.url())) polled.push(request.url());
    });
    await signIn(page, student);
    await openSum(page);
    const history = page.getByRole("list", { name: "Vos soumissions" }).getByRole("listitem");
    const before = await history.count();

    await page.getByLabel("Code Python").fill(SOLUTION);
    await page.getByRole("button", { name: "Soumettre" }).click();

    await expect(page.getByRole("status")).toHaveText("Réussi : tous les tests passent.", VERDICT);
    await expect(history).toHaveCount(before + 1);
    await expect(history.first()).toContainText("Réussi");
    expect(polled).toEqual([]);
    await expectAccessible(page);
});

test("without the event stream, the verdict still arrives by polling", async ({ page, student }) => {
    test.slow();
    // As from an API without the stream: the browser gives up on it, and the page polls (ADR-0008)
    await page.route("**/api/events", (route) => route.fulfill({ status: 404 }));
    await signIn(page, student);
    await openSum(page);
    await page.getByLabel("Code Python").fill(SOLUTION);
    await page.getByRole("button", { name: "Soumettre" }).click();
    await expect(page.getByRole("status")).toHaveText("Réussi : tous les tests passent.", VERDICT);
});

test("a wrong program fails", async ({ page, student }) => {
    test.slow();
    await signIn(page, student);
    await openSum(page);
    await page.getByLabel("Code Python").fill("print(0)\n");
    await page.getByRole("button", { name: "Soumettre" }).click();
    await expect(page.getByRole("status")).toHaveText("Échoué : au moins un test ne passe pas.", VERDICT);
});

// Drafts are tested here rather than in a file of their own: files run in parallel, and another
// test of the same student writing the same draft would turn this file's saves into conflicts.
const editor = (page) => page.getByLabel("Code Python");
const saved = (page) => expect(editor(page)).toHaveAccessibleDescription("Brouillon enregistré.");

test("the draft is found again after a reload", async ({ page, student }) => {
    const code = `print(${Date.now()})\n`;
    await signIn(page, student);
    await openSum(page);
    await editor(page).fill(code);
    await expect(editor(page)).toHaveAccessibleDescription("Enregistrement du brouillon…");
    await saved(page);

    await page.reload();
    await openSum(page);
    await expect(editor(page)).toHaveValue(code);
});

test("a draft saved from another tab meanwhile lets the student choose", async ({ context, page, student }) => {
    await signIn(page, student);
    await openSum(page);
    await saved(page); // its revision read before the other tab writes
    const other = await context.newPage(); // same session
    await other.goto("/");
    await openSum(other);
    await saved(other);

    const theirs = `print("other tab ${Date.now()}")\n`;
    await other.getByLabel("Code Python").fill(theirs);
    await saved(other);
    const mine = `print("this tab ${Date.now()}")\n`;
    await editor(page).fill(mine);

    await expect(page.getByRole("status")).toHaveText(
        "Un autre onglet a enregistré ce code entre-temps : choisissez la version à garder.",
    );
    await expect(editor(page)).toHaveAccessibleDescription("Brouillon non enregistré.");
    await expect(page.getByLabel("Version enregistrée")).toHaveValue(theirs);
    await expect(editor(page)).toHaveValue(mine);
    await expectAccessible(page);

    await page.getByRole("button", { name: "Garder ma version" }).click();
    await saved(page);
    await expect(page.getByLabel("Version enregistrée")).toHaveCount(0);
    await other.reload();
    await openSum(other);
    await expect(other.getByLabel("Code Python")).toHaveValue(mine);
});

test("taking the saved version replaces the editor's", async ({ context, page, student }) => {
    await signIn(page, student);
    await openSum(page);
    await saved(page);
    const other = await context.newPage();
    await other.goto("/");
    await openSum(other);
    const theirs = `print("kept ${Date.now()}")\n`;
    await other.getByLabel("Code Python").fill(theirs);
    await saved(other);

    await editor(page).fill("print('dropped')\n");
    await page.getByRole("button", { name: "Prendre la version enregistrée" }).click();
    await expect(editor(page)).toHaveValue(theirs);
    await saved(page);
});

test("the session survives a reload and ends on sign-out", async ({ page, student }) => {
    await signIn(page, student);
    await page.reload();
    await expect(page.getByRole("list", { name: "Exercices" })).toBeVisible();

    await page.getByRole("button", { name: "Se déconnecter" }).click();
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
    await page.reload();
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
});
