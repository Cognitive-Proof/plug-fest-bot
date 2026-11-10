// Copies the C2PA Conformance Task Force's signals rubric into the bot as JSON.
// Usage: node scripts/sync-signals-rubric.mjs [path/to/conformance]   (default: ../conformance)
// Reads asset-rubrics/asset-rubric-signals-local.yml at that checkout's current commit and writes
// src/signals/asset-rubric-signals-local.json, recording the commit it came from.
import {execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {parseAllDocuments} from 'yaml';

const repo = resolve(process.argv[2] ?? '../conformance');
const path = 'asset-rubrics/asset-rubric-signals-local.yml';
const git = (...args) => execFileSync('git', ['-C', repo, ...args], {encoding: 'utf8'}).trim();
if (git('status', '--porcelain', '--', path)) throw new Error(`${path} has uncommitted changes in ${repo}`);

const docs = parseAllDocuments(readFileSync(resolve(repo, path), 'utf8')).map(d => d.toJS());
const metadata = docs[0] && typeof docs[0] === 'object' && !Array.isArray(docs[0]) ? docs[0] : {};
// As the Python evaluator: list documents hold statements; other mappings are statements unless blocks.
const statements = docs.slice(1).flatMap(d => (Array.isArray(d) ? d : d && typeof d === 'object' && !('block' in d) ? [d] : []));

const out = {
  source: {repository: 'https://github.com/c2pa-org/conformance', commit: git('rev-parse', 'HEAD'), branch: git('rev-parse', '--abbrev-ref', 'HEAD'), path},
  metadata,
  statements,
};
writeFileSync('src/signals/asset-rubric-signals-local.json', JSON.stringify(out, null, 1) + '\n');
console.log(`${statements.length} statements from ${out.source.branch} @ ${out.source.commit.slice(0, 7)}`);
