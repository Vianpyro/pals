// Same origin as the page: nginx sends /api/ to the API, so the session cookie goes with every call
const API = "/api";
const POLL_MS = 1000;
// shortcut: one course picked by the URL, until the API lists a student's courses (ADR-0012)
const course = new URLSearchParams(location.search).get("course") ?? "log200";
// The exercise whose statement is shown, picked in the catalog
let current = null;

const UNREACHABLE = "Le serveur démarre ou ne répond pas : réessayez dans un instant.";
const SIGN_IN_ERRORS = {
    401: "Identifiant ou mot de passe refusé.",
    503: "L'annuaire ne répond pas, réessayez plus tard.",
};
const CATALOG_ERRORS = {
    404: "Ce cours n'existe pas.",
    502: UNREACHABLE,
};
const STATEMENT_ERRORS = {
    403: "Cet exercice n'est pas ouvert.",
    404: "Cet exercice n'existe pas.",
    502: UNREACHABLE,
};
const SUBMISSION_ERRORS = {
    ...STATEMENT_ERRORS,
    422: "Le code est vide ou dépasse 1 Mio.",
};
const STATES = {
    waiting: "En attente d'un juge…",
    running: "Correction en cours…",
    failed: "La correction a échoué : prévenez l'enseignant.",
    replaced: "Remplacée par une soumission plus récente.",
};

const element = (id) => document.getElementById(id);
const say = (text) => {
    element("status").textContent = text;
};
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function show(view) {
    element("sign-in").hidden = view !== "sign-in";
    element("course").hidden = view !== "course";
}

const when = (iso) =>
    new Date(iso).toLocaleString("fr-CA", { dateStyle: "long", timeStyle: "short" });

// Only a label: whether the exercise is open is the server's call (ADR-0020)
function availability({ open, opens_at, closes_at }) {
    if (open) {
        return closes_at ? `jusqu'au ${when(closes_at)}` : "";
    }
    if (!opens_at) {
        return "pas encore planifié";
    }
    return new Date(opens_at) > new Date() ? `ouvre le ${when(opens_at)}` : "fermé";
}

function item(entry) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = entry.id;
    button.disabled = !entry.open;
    button.addEventListener("click", () => openExercise(entry.id));

    const li = document.createElement("li");
    const note = availability(entry);
    li.append(button, note ? ` — ${note}` : "");
    return li;
}

async function openCourse() {
    const response = await call("GET", `/courses/${encodeURIComponent(course)}/exercises`);
    if (response.status === 401) {
        show("sign-in");
        return;
    }
    if (!response.ok) {
        say(CATALOG_ERRORS[response.status] ?? `Erreur ${response.status}.`);
        return;
    }
    const entries = await response.json();
    // textContent and createElement, never innerHTML: identifiers come from content (ADR-0023)
    element("catalog").replaceChildren(...entries.map(item));
    if (entries.length === 0) {
        say("Aucun exercice publié pour l'instant.");
    }
    show("course");
}

function call(method, path, body) {
    const options = { method };
    if (body !== undefined) {
        options.headers = { "Content-Type": "application/json" };
        options.body = JSON.stringify(body);
    }
    return fetch(API + path, options);
}

async function openExercise(exercise) {
    const path = `/courses/${encodeURIComponent(course)}/exercises/${encodeURIComponent(exercise)}/statement`;
    const response = await call("GET", path);
    if (response.status === 401) {
        show("sign-in");
        return;
    }
    if (!response.ok) {
        say(STATEMENT_ERRORS[response.status] ?? `Erreur ${response.status}.`);
        return;
    }
    // textContent, never innerHTML: the statement comes from untrusted content (ADR-0023)
    element("statement").textContent = await response.text();
    current = exercise;
    say("");
    element("exercise").hidden = false;
}

// shortcut: polls every second, until verdicts arrive over SSE (architecture, Web application)
async function follow(id) {
    for (; ;) {
        const response = await call("GET", `/submissions/${id}`);
        if (!response.ok) {
            say(`Erreur ${response.status}.`);
            return;
        }
        const { state, passed } = await response.json();
        if (state === "done") {
            say(passed ? "Réussi : tous les tests passent." : "Échoué : au moins un test ne passe pas.");
            return;
        }
        say(STATES[state] ?? state);
        if (state !== "waiting" && state !== "running") {
            return;
        }
        await sleep(POLL_MS);
    }
}

element("sign-in").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = new FormData(event.target);
    const response = await call("POST", "/sessions", {
        id: form.get("id"),
        password: form.get("password"),
    });
    if (!response.ok) {
        say(SIGN_IN_ERRORS[response.status] ?? `Erreur ${response.status}.`);
        return;
    }
    const identity = await response.json();
    event.target.reset();
    say(`Bonjour, ${identity.name ?? identity.id}.`);
    await openCourse();
});

element("sign-out").addEventListener("click", async () => {
    await call("DELETE", "/sessions");
    say("");
    current = null;
    element("exercise").hidden = true;
    show("sign-in");
});

element("submission").addEventListener("submit", async (event) => {
    event.preventDefault();
    const button = event.submitter;
    button.disabled = true;
    try {
        const response = await call("POST", "/submissions", {
            course,
            exercise: current,
            // shortcut: the judge has only the Python pack until packs are data (ADR-0013)
            language_pack: "python",
            source: element("source").value,
        });
        if (response.status === 401) {
            say("Session expirée : reconnectez-vous.");
            show("sign-in");
            return;
        }
        if (response.status !== 202) {
            say(SUBMISSION_ERRORS[response.status] ?? `Erreur ${response.status}.`);
            return;
        }
        const { id } = await response.json();
        await follow(id);
    } finally {
        button.disabled = false;
    }
});

// fetch rejects only when the server cannot be reached at all
window.addEventListener("unhandledrejection", () => say("Serveur injoignable."));

openCourse();
