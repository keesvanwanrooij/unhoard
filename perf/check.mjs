// Performance check (T-P4): compares the latest measurements with the budgets
// (perf/budgets.json) and the committed baseline (perf/baseline.json).
//
// Fails above a budget. Against the baseline, fails at more than 20% slower in CI and warns
// at more than 10% slower locally. `--update-baseline` writes the current numbers as the new
// baseline; do that on purpose, in a pull request that says why.
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const read = (p) => (existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : {});

/** Criterion results (mean, in milliseconds) from target/criterion/<name>/new/estimates.json. */
function criterion() {
  const dir = "target/criterion";
  const out = {};
  if (!existsSync(dir)) return out;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name, "new", "estimates.json");
    if (existsSync(p)) out[name] = JSON.parse(readFileSync(p, "utf8")).mean.point_estimate / 1e6;
  }
  return out;
}

const current = { ...read("perf/results/layout.json"), ...read("perf/results/ui.json"), ...criterion() };
const budgets = read("perf/budgets.json");
const baseline = read("perf/baseline.json");
const ci = Boolean(process.env.CI);

if (process.argv.includes("--update-baseline")) {
  writeFileSync("perf/baseline.json", `${JSON.stringify(current, null, 2)}\n`);
  console.log(`baseline written with ${String(Object.keys(current).length)} measurements`);
  process.exit(0);
}

let failed = false;
const rows = [];
for (const [key, value] of Object.entries(current).sort()) {
  const b = budgets[key];
  const base = baseline[key];
  const ratio = base ? value / base : null;
  let state = "ok";
  if (b && value > b.maxMs) {
    state = `OVER ${b.budget} (${String(b.maxMs)} ms)`;
    failed = true;
  } else if (ratio !== null && ratio > 1.2 && value > 1) {
    state = ci ? "REGRESSION >20%" : "warning: >20% slower";
    if (ci) failed = true;
  } else if (ratio !== null && ratio > 1.1 && value > 1) {
    state = "warning: >10% slower";
  }
  rows.push([key, value.toFixed(2), base ? base.toFixed(2) : "-", b ? `${b.budget} ≤ ${String(b.maxMs)}` : "", state]);
}
const missing = Object.keys(budgets).filter((k) => !(k in current));
console.table(rows.map(([k, v, base, budget, state]) => ({ measure: k, ms: v, baseline: base, budget, state })));
if (missing.length > 0) console.log(`not measured in this run: ${missing.join(", ")}`);
process.exit(failed ? 1 : 0);
