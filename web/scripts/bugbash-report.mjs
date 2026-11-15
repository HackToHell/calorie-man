// Turn a bug bash output directory into REPORT.md (printed to stdout).
// Usage: node web/scripts/bugbash-report.mjs <reportDir>
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { basename, join, relative } from 'node:path';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: bugbash-report.mjs <dir>');
  process.exit(2);
}

const lines = [];
const p = (s = '') => lines.push(s);

p(`# Bug bash report ${basename(dir)}`);
p();
p(`Generated ${new Date().toISOString()} on ${process.platform}.`);
p();
p('## Automated steps');
p();
p('| Step | Result | Time | Log |');
p('|---|---|---|---|');
const steps = existsSync(join(dir, 'steps.tsv'))
  ? readFileSync(join(dir, 'steps.tsv'), 'utf8').trim().split('\n').filter(Boolean).map((l) => l.split('\t'))
  : [];
for (const [name, status, secs] of steps) {
  p(`| ${name} | ${status === '0' ? '✅ pass' : `❌ exit ${status}`} | ${secs}s | logs/${name}.log |`);
}
p();

// Soft-assertion findings from the sweep/fuzz run.
const findings = [];
const resultsPath = join(dir, 'results.json');
if (existsSync(resultsPath)) {
  const results = JSON.parse(readFileSync(resultsPath, 'utf8'));
  const walk = (suite, path = []) => {
    for (const s of suite.suites ?? []) walk(s, [...path, s.title]);
    for (const spec of suite.specs ?? []) {
      for (const t of spec.tests ?? []) {
        for (const r of t.results ?? []) {
          for (const e of r.errors ?? []) {
            const msg = (e.message ?? '').replace(/\u001b\[[0-9;]*m/g, '');
            findings.push({ test: [...path, spec.title].filter(Boolean).join(' › '), project: t.projectName, msg });
          }
        }
      }
    }
  };
  walk(results);
  p('## Sweep & fuzz findings');
  p();
  if (!findings.length) p('None. Every screen rendered cleanly in every viewport and scheme, and fuzzing found no server errors.');
  for (const f of findings) {
    p(`### [${f.project}] ${f.test}`);
    p();
    p('```');
    p(f.msg.split('\n').slice(0, 30).join('\n'));
    p('```');
    p();
  }
  p();
}

const screensDir = join(dir, 'screens');
if (existsSync(screensDir)) {
  p('## Screenshots');
  p();
  p('Review every image; automated checks do not catch ugly. Look for misalignment, cut-off text, unreadable dark-mode colors, empty states that make no sense, and numbers that look wrong.');
  p();
  for (const proj of readdirSync(screensDir)) {
    const files = readdirSync(join(screensDir, proj)).filter((f) => f.endsWith('.png'));
    p(`- **${proj}** (${files.length}): ${relative(dir, join(screensDir, proj))}/`);
  }
  p();
}

p('## Manual charters');
p();
p('Work through the charters in docs/BUGBASH.md against the demo server. Record each bug below with the template.');
p();
p('## Bugs found');
p();
p('| # | Sev | Area | Summary | Status | Regression test |');
p('|---|---|---|---|---|---|');
p('| 1 | | | | open | |');
p();
console.log(lines.join('\n'));
