// What the student sees when the server fails, simulated: these are hard to cause on the real stack.
// The page itself still comes from nginx, with its CSP.
import { expect, test } from "./fixtures.js";

const CATALOG = "**/api/courses/log200/exercises";

// A signed-in student with the sum exercise open, every API answer simulated
async function openSumWithFakeApi(page) {
    await page.route(CATALOG, (route) => route.fulfill({ json: [{ id: "sum", open: true, opens_at: null, closes_at: null }] }));
    await page.route(`${CATALOG}/sum/statement`, (route) => route.fulfill({ body: "Lire deux entiers." }));
    await page.route(`${CATALOG}/sum/submissions`, (route) => route.fulfill({ json: [] }));
    await page.goto("/");
    await page.getByRole("button", { name: "sum" }).click();
    await expect(page.getByText("Lire deux entiers.")).toBeVisible();
}

test("the API starting up is explained", async ({ page }) => {
    await page.route(CATALOG, (route) => route.fulfill({ status: 502 }));
    await page.goto("/");
    await expect(page.getByRole("status")).toHaveText("Le serveur démarre ou ne répond pas : réessayez dans un instant.");
});

test("an unreachable server is explained", async ({ page }) => {
    await page.route(CATALOG, (route) => route.abort("connectionrefused"));
    await page.goto("/");
    await expect(page.getByRole("status")).toHaveText("Serveur injoignable.");
});

test("a refused submission says why and lets the student submit again", async ({ page }) => {
    await openSumWithFakeApi(page);
    await page.route("**/api/submissions", (route) => route.fulfill({ status: 422 }));
    await page.getByLabel("Code Python").fill("x");
    await page.getByRole("button", { name: "Soumettre" }).click();
    await expect(page.getByRole("status")).toHaveText("Le code est vide ou dépasse 1 Mio.");
    await expect(page.getByRole("button", { name: "Soumettre" })).toBeEnabled();
});

test("an expired session sends the student back to sign in", async ({ page }) => {
    await openSumWithFakeApi(page);
    await page.route("**/api/submissions", (route) => route.fulfill({ status: 401 }));
    await page.getByLabel("Code Python").fill("x");
    await page.getByRole("button", { name: "Soumettre" }).click();
    await expect(page.getByRole("status")).toHaveText("Session expirée : reconnectez-vous.");
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
});
