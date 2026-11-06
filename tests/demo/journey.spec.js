// The student's journey in the demo: the real components, with Python running in the browser.
import { readFileSync } from "node:fs";
import { expect, expectAccessible, test } from "../e2e/fixtures.js";

// Pyodide is downloaded by each new browser context, and a timed-out run reloads it
const VERDICT = { timeout: 120_000 };
const SOLUTION = "import sys\nprint(sum(map(int, sys.stdin.read().split())))\n";

const conformance = (file) => readFileSync(`packs/languages/python/conformance/${file}`, "utf-8");
const editor = (page) => page.getByLabel("Code Python");
const history = (page) => page.getByRole("list", { name: "Vos soumissions" }).getByRole("listitem");

// Relative to the subfolder the demo is served from, as on GitHub Pages
async function signIn(page) {
    await page.goto("./");
    await page.getByLabel("Identifiant").fill("etudiant1");
    await page.getByLabel("Mot de passe").fill("log791-dev");
    await page.getByRole("button", { name: "Se connecter" }).click();
    await expect(page.getByRole("status")).toHaveText(/^Bonjour, /);
}

async function openSum(page) {
    await page.getByRole("button", { name: "sum" }).click();
    await expect(page.getByText("Lire deux entiers sur une ligne")).toBeVisible();
    await expect(editor(page)).not.toHaveAttribute("readonly");
}

test("the page says it is a demo, gives its account and is accessible", async ({ page }) => {
    await page.goto("./");
    await expect(page.getByRole("complementary")).toContainText("etudiant1 / log791-dev");
    await expect(page.getByRole("complementary")).toContainText("pas sur le juge");
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
    await expectAccessible(page);
});

test("a wrong password is refused", async ({ page }) => {
    await page.goto("./");
    await page.getByLabel("Identifiant").fill("etudiant1");
    await page.getByLabel("Mot de passe").fill("not-the-password");
    await page.getByRole("button", { name: "Se connecter" }).click();
    await expect(page.getByRole("status")).toHaveText("Identifiant ou mot de passe refusé.");
});

test("a correct program passes in the browser, with no call to a server", async ({ page }) => {
    test.slow();
    const calls = [];
    page.on("request", (request) => {
        if (request.url().includes("/api/")) calls.push(request.url());
    });
    await signIn(page);
    await openSum(page);

    await editor(page).fill(SOLUTION);
    await page.getByRole("button", { name: "Soumettre" }).click();

    await expect(page.getByRole("status")).toHaveText("Réussi : tous les tests passent.", VERDICT);
    await expect(history(page).first()).toContainText("Réussi");
    expect(calls).toEqual([]);
    await expectAccessible(page);
});

test("the Python pack's conformance programs get the judge's verdicts", async ({ page }) => {
    test.slow();
    await signIn(page);
    await openSum(page);

    const programs = [
        ["wrong-answer/zero.py", "Échoué"],
        ["crashed/exception.py", "Échoué"],
        ["crashed/syntax-error.py", "Échoué"],
        ["timed-out/endless.py", "Échoué"],
        // After the timeout: on the new worker
        ["passed/sum.py", "Réussi"],
    ];
    for (const [index, [file, verdict]] of programs.entries()) {
        await editor(page).fill(conformance(file));
        await page.getByRole("button", { name: "Soumettre" }).click();
        await expect(history(page)).toHaveCount(index + 1);
        await expect(history(page).first(), file).toContainText(verdict, VERDICT);
    }
});

test("a program runs as its own process would", async ({ page }) => {
    test.slow();
    await signIn(page);
    await openSum(page);

    const programs = [
        // Raw standard streams
        ["import sys\na, b = sys.stdin.buffer.read().split()\nsys.stdout.buffer.write(b'%d\\n' % (int(a) + int(b)))\n", "Réussi"],
        // An exit status of 0 is a success, and only that
        ["import sys\nprint(sum(map(int, input().split())))\nsys.exit(0)\n", "Réussi"],
        ["import sys\nprint(sum(map(int, input().split())))\nsys.exit(1)\n", "Échoué"],
        // A previous program's globals are gone: this one passes only if it does not see them
        ["leak = 1\n" + SOLUTION, "Réussi"],
        ["if 'leak' in globals(): raise SystemExit(1)\n" + SOLUTION, "Réussi"],
    ];
    for (const [index, [source, verdict]] of programs.entries()) {
        await editor(page).fill(source);
        await page.getByRole("button", { name: "Soumettre" }).click();
        await expect(history(page)).toHaveCount(index + 1);
        await expect(history(page).first(), source).toContainText(verdict, VERDICT);
    }
});

test("the session and the draft are found again after a reload", async ({ page }) => {
    await signIn(page);
    await openSum(page);
    await editor(page).fill("print('kept')\n");
    await expect(editor(page)).toHaveAccessibleDescription("Brouillon enregistré.");

    await page.reload();
    await openSum(page);
    await expect(editor(page)).toHaveValue("print('kept')\n");
});

test("a draft saved from another tab meanwhile lets the student choose", async ({ context, page }) => {
    await signIn(page);
    await openSum(page);
    const other = await context.newPage();
    await other.goto("./");
    await openSum(other);

    await other.getByLabel("Code Python").fill("print('theirs')\n");
    await expect(other.getByLabel("Code Python")).toHaveAccessibleDescription("Brouillon enregistré.");
    await editor(page).fill("print('mine')\n");

    await expect(page.getByLabel("Version enregistrée")).toHaveValue("print('theirs')\n");
    await expect(editor(page)).toHaveValue("print('mine')\n");
});

test("signing out ends the session", async ({ page }) => {
    await signIn(page);
    await page.getByRole("button", { name: "Se déconnecter" }).click();
    await page.reload();
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
});
