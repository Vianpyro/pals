// Minimum and recommended hardware per role, from the measures of tests/load/run.sh.
// Usage: node tests/load/sizing.js <budget.json> <measure.json>...
// Prints a Markdown report and fails when a measured memory is over its budget.
//
// A job's duration depends on how many judges share a machine, so judges alone do not scale: past
// a ceiling, more of them only make each job slower. A machine is therefore sized from what bursts
// measure, the rate it sustains at each number of judges, and the load from how many such machines
// keep the queue under the target utilization. The queue stays a bounded M/M/c: c judges serve one
// job at a time and a student has one job waiting at most (apps/api/src/main.rs, enqueue_replacing).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const LIMITS = {
    sandbox_mib: 256, // MEMORY in apps/judge/src/sandbox.rs: a student's program may reach it
    utilization: 0.7, // Also the longest wait after a burst, as a share of the period
    period: 180, // Seconds between two submissions of a student (docs/architecture.typ, target load)
};
const PROFILES = [
    { name: "Minimale", students: 250, spare: 0 },
    { name: "Recommandée", students: 1250, spare: 1 },
];
// Within this share of the best rate, more judges add wait rather than throughput
const AT_CEILING = 0.9;

export const worstBurst = ({ students, service, judges }) => (students * service) / judges;

// Erlang B's recurrence, then C: the factorials of the closed form overflow at these sizes
function probabilityOfWaiting(judges, load) {
    let blocked = 1;
    for (let k = 1; k <= judges; k++) blocked = (load * blocked) / (k + load * blocked);
    return blocked / (1 - (load / judges) * (1 - blocked));
}

export function waitP95({ students, service, period, judges }) {
    const load = (students * service) / period;
    if (judges <= load) return Infinity;
    const waiting = probabilityOfWaiting(judges, load);
    if (waiting <= 0.05) return 0;
    return Math.log(waiting / 0.05) / ((judges - load) / service);
}

const rateOf = ({ jobs, duration_s }) => jobs.done / duration_s;

export function capacity(measures, pack) {
    const bursts = measures
        .filter((measure) => measure.mode === "burst" && measure.pack === pack)
        .sort((a, b) => a.judges - b.judges);
    if (bursts.some(({ jobs }) => jobs.done === 0)) throw new Error(`${pack}: a burst judged no job`);
    const best = Math.max(...bursts.map(rateOf));
    const run = bursts.find((measure) => rateOf(measure) >= AT_CEILING * best);
    return {
        pack,
        judges: run.judges,
        rate: rateOf(run),
        service: run.jobs.service_mean_s,
        reached: run.judges < bursts.at(-1).judges,
        run,
    };
}

function slowest(measures) {
    const bursts = measures.filter((measure) => measure.mode === "burst");
    if (bursts.length === 0) throw new Error("no burst to measure a machine's rate from");
    const packs = [...new Set(bursts.map((measure) => measure.pack))];
    return packs
        .map((pack) => capacity(bursts, pack))
        .reduce((lowest, ceiling) => (ceiling.rate < lowest.rate ? ceiling : lowest));
}

function largestSteady(measures) {
    const steady = measures.filter((measure) => measure.mode === "steady");
    if (steady.length === 0) throw new Error("no steady measure to size web and database from");
    return steady.reduce((largest, measure) => (measure.students > largest.students ? measure : largest));
}

const scaled = (idle, peak, scale) => idle + scale * (peak - idle);

function sizeProfile(profile, ceiling, load, measures, limits) {
    const { utilization, period, sandbox_mib } = limits;
    const { students, spare } = profile;
    const scale = students / load.students;
    const vms = Math.ceil(students / period / utilization / ceiling.rate);
    const judges = vms * ceiling.judges;
    const cores = (cpu) => ((cpu / load.duration_s) * scale) / utilization;
    const webCores = cores(load.cpu_s.api);
    const dbCores = cores(load.cpu_s.postgres);
    const { memory_mib, jobs } = ceiling.run;
    const rows = load.jobs.done + load.jobs.replaced + load.jobs.failed;
    const held = vms === 1 && measures.some((measure) =>
        measure.mode === "steady" && measure.pack === ceiling.pack
        && measure.students >= students && measure.judges <= judges
        && measure.http.errors === 0 && measure.jobs.failed === 0);

    return {
        name: profile.name,
        students,
        pack: ceiling.pack,
        basis: held ? "mesuré" : "extrapolé",
        judge: {
            vms,
            judges,
            judges_per_vm: ceiling.judges,
            // One core per sandbox, since a student may use it, and what the judges themselves took
            cores_per_vm: ceiling.judges + Math.ceil(ceiling.run.cpu_s.judges / ceiling.run.duration_s),
            memory_mib_per_vm: ceiling.judges * (sandbox_mib + memory_mib.overhead_each + memory_mib.judge_each),
            spare_vms: spare,
            ceiling_reached: ceiling.reached,
            wait_p95_s: waitP95({ students, service: jobs.service_mean_s, period, judges }),
            worst_burst_s: students / (vms * ceiling.rate),
        },
        web: {
            cores_exact: webCores,
            cores: Math.ceil(webCores),
            memory_mib: scaled(load.memory_mib.api_idle, load.memory_mib.api, scale),
        },
        db: {
            cores_exact: dbCores,
            cores: Math.ceil(dbCores),
            memory_mib: scaled(load.memory_mib.postgres_idle, load.memory_mib.postgres, scale),
            disk_mib_per_hour: (load.disk_mib.database_growth / rows) * (students * 3600) / period,
        },
    };
}

export function size(measures, profiles = PROFILES, limits = LIMITS) {
    const load = largestSteady(measures);
    const ceiling = slowest(measures);
    return profiles.map((profile) => sizeProfile(profile, ceiling, load, measures, limits));
}

const seconds = (value) => (Number.isFinite(value) ? `${value.toFixed(1)} s` : "instable");
const gib = (mib) => `${(mib / 1024).toFixed(2)} Gio`;
const coresText = (cores) => `${cores} ${cores > 1 ? "cœurs" : "cœur"}`;

function modelled(measure, period) {
    const load = { students: measure.students, service: measure.jobs.service_mean_s, judges: measure.judges };
    return measure.mode === "burst"
        ? { measured: measure.jobs.wait_max_s, model: worstBurst(load) }
        : { measured: measure.jobs.wait_p95_s, model: waitP95({ ...load, period }) };
}

export function report(measures, profiles = PROFILES, limits = LIMITS) {
    const sized = size(measures, profiles, limits);
    const packs = [...new Set(measures.filter((m) => m.mode === "burst").map((m) => m.pack))];
    const lines = [
        "## Configurations",
        "",
        `Pack le plus limitant : **${sized[0].pack}**. Utilisation cible : ${limits.utilization * 100} %. `
        + `Une machine \`judge\` y est supposée comparable à celle de la mesure (${measures[0].host_cores} cœurs).`,
        "",
        "| Profil | Étudiants | Base | Machines `judge` | Judges | P95 d'attente | Pire rafale | Par machine `judge` | `web` | `db` | Disque par heure |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ];
    for (const { name, students, basis, judge, web, db } of sized) {
        const spare = judge.spare_vms ? ` + ${judge.spare_vms} de secours` : "";
        lines.push(
            `| ${name} | ${students} | ${basis} | ${judge.vms}${spare} | ${judge.judges} `
            + `| ${seconds(judge.wait_p95_s)} | ${seconds(judge.worst_burst_s)} `
            + `| ${judge.judges_per_vm} judges, ${coresText(judge.cores_per_vm)}, ${gib(judge.memory_mib_per_vm)} `
            + `| ${coresText(web.cores)}, ${gib(web.memory_mib)} | ${coresText(db.cores)}, ${gib(db.memory_mib)} `
            + `| ${db.disk_mib_per_hour.toFixed(0)} Mio |`,
        );
    }
    lines.push(
        "",
        "## Plafond d'une machine",
        "",
        "| Pack | Judges | Débit | État |",
        "|---|---|---|---|",
    );
    for (const pack of packs) {
        const ceiling = capacity(measures, pack);
        const state = ceiling.reached ? "plafond atteint" : "plafond non atteint : mesurer plus de judges";
        lines.push(`| ${pack} | ${ceiling.judges} | ${ceiling.rate.toFixed(2)} job/s | ${state} |`);
    }
    lines.push(
        "",
        "## Modèle et mesures",
        "",
        "Attente au P95 en régime `steady`, attente maximale en `burst`.",
        "",
        "| Mode | Pack | Étudiants | Judges | S moyen | Débit | CPU par job | Jobs jugés | Erreurs HTTP | Attente mesurée | Modèle |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
    );
    const ordered = [...measures].sort((a, b) =>
        a.mode.localeCompare(b.mode) || a.pack.localeCompare(b.pack) || a.judges - b.judges);
    for (const measure of ordered) {
        const { measured, model } = modelled(measure, limits.period);
        const { jobs, cpu_s } = measure;
        const cpu = jobs.done ? (cpu_s.sandboxes + cpu_s.judges) / jobs.done : 0;
        lines.push(
            `| ${measure.mode} | ${measure.pack} | ${measure.students} | ${measure.judges} `
            + `| ${jobs.service_mean_s.toFixed(2)} s | ${rateOf(measure).toFixed(2)}/s | ${cpu.toFixed(2)} s `
            + `| ${jobs.done} | ${measure.http.errors} | ${seconds(measured)} | ${seconds(model)} |`,
        );
    }
    return lines.join("\n");
}

export function checkBudgets(measures, budget) {
    const over = [];
    for (const { mode, pack, memory_mib } of measures) {
        for (const [name, limit] of Object.entries(budget)) {
            const used = memory_mib[name];
            if (used === undefined) over.push(`${name}: absent de ${mode} ${pack}`);
            else if (used > limit) over.push(`${name}: ${used} Mio > ${limit} Mio (${mode} ${pack})`);
        }
    }
    return over;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    const [budgetFile, ...files] = process.argv.slice(2);
    const measures = files.map((file) => JSON.parse(readFileSync(file, "utf8")));
    console.log(report(measures));
    const over = checkBudgets(measures, JSON.parse(readFileSync(budgetFile, "utf8")));
    for (const line of over) console.error(`OVER  ${line}`);
    process.exitCode = over.length > 0 ? 1 : 0;
}
