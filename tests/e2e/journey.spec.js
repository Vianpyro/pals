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

test("a correct program passes and is listed first in the history", async ({ page, student }) => {
    test.slow(); // the judge runs every case in a sandbox
    await signIn(page, student);
    await openSum(page);
    const history = page.getByRole("list", { name: "Vos soumissions" }).getByRole("listitem");
    const before = await history.count();

    await page.getByLabel("Code Python").fill(SOLUTION);
    await page.getByRole("button", { name: "Soumettre" }).click();

    await expect(page.getByRole("status")).toHaveText("Réussi : tous les tests passent.", VERDICT);
    await expect(history).toHaveCount(before + 1);
    await expect(history.first()).toContainText("Réussi");
    await expectAccessible(page);
});

test("a wrong program fails", async ({ page, student }) => {
    test.slow();
    await signIn(page, student);
    await openSum(page);
    await page.getByLabel("Code Python").fill("print(0)\n");
    await page.getByRole("button", { name: "Soumettre" }).click();
    await expect(page.getByRole("status")).toHaveText("Échoué : au moins un test ne passe pas.", VERDICT);
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
