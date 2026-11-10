// Checks the JavaScript signals evaluator against the Python reference: for every `<name>.crjson.json`
// (or `<name>.json`) with a `<name>.signals.json` next to it, the JS output must equal the Python one.
// Usage: node scripts/check-signals-parity.mjs [dir ...]   (default: ../conformance/asset-rubrics/test)
import {existsSync, readFileSync, readdirSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {evaluateSignals} from '../src/signals/evaluator.js';

const rubric = JSON.parse(readFileSync(new URL('../src/signals/asset-rubric-signals-local.json', import.meta.url)));
const dirs = process.argv.length > 2 ? process.argv.slice(2) : ['../conformance/asset-rubrics/test'];

// Key order differs between the two, so compare with keys sorted.
const canon = v => (Array.isArray(v) ? v.map(canon) : v && typeof v === 'object'
  ? Object.fromEntries(Object.keys(v).sort().map(k => [k, canon(v[k])])) : v);

let checked = 0;
const failures = [];
for (const dir of dirs.map(d => resolve(d))) {
  for (const file of readdirSync(dir).filter(f => f.endsWith('.signals.json')).sort()) {
    const base = file.slice(0, -'.signals.json'.length);
    const input = [`${base}.crjson.json`, `${base}.json`].map(f => join(dir, f)).find(existsSync);
    if (!input) continue;
    const errors = [];
    const got = evaluateSignals(rubric, JSON.parse(readFileSync(input)), {onError: e => errors.push(e)});
    const want = JSON.parse(readFileSync(join(dir, file)));
    checked++;
    if (JSON.stringify(canon(got)) !== JSON.stringify(canon(want)) || errors.length) {
      failures.push({file: join(dir, file), errors, got, want});
    }
  }
}
for (const f of failures) {
  console.log(`DIFF ${f.file}`);
  for (const e of f.errors) console.log(`  ${e}`);
  console.log(`  js:     ${JSON.stringify(f.got.manifests.map(m => [...m.localInceptions, ...m.localTransformations].map(s => s.trait)))}`);
  console.log(`  python: ${JSON.stringify(f.want.manifests.map(m => [...m.localInceptions, ...m.localTransformations].map(s => s.trait)))}`);
}
console.log(`${checked - failures.length} of ${checked} match the Python evaluator`);
process.exit(failures.length || !checked ? 1 : 0);
