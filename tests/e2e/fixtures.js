import AxeBuilder from "@axe-core/playwright";
import { test as base, expect } from "@playwright/test";

export { expect };

export const test = base.extend({
    // Set per browser in playwright.config.js
    student: ["etudiant1", { option: true }],

    // Fails any test whose page broke the CSP. Only nginx sends it, so a style or script the
    // production build injects would otherwise break unseen under SEB (spec D12, D25).
    page: async ({ page }, use) => {
        await page.addInitScript(() => {
            window.cspViolations = [];
            document.addEventListener("securitypolicyviolation", (event) =>
                window.cspViolations.push(`${event.violatedDirective} ${event.blockedURI}`),
            );
        });
        await use(page);
        expect(await page.evaluate(() => window.cspViolations), "CSP violations").toEqual([]);
    },
});

// WCAG 2.1 AA on what the page shows now (architecture, Accessibility)
export async function expectAccessible(page) {
    const { violations } = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"]).analyze();
    expect(violations.map((violation) => `${violation.id}: ${violation.help}`)).toEqual([]);
}

// The development accounts of .devcontainer/ldap
export async function signIn(page, student) {
    await page.goto("/");
    await page.getByLabel("Identifiant").fill(student);
    await page.getByLabel("Mot de passe").fill("log791-dev");
    await page.getByRole("button", { name: "Se connecter" }).click();
    await expect(page.getByRole("status")).toHaveText(/^Bonjour, /);
}
