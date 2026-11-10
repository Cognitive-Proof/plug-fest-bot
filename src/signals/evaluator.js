// JavaScript port of the C2PA Conformance Task Force's signals rubric evaluator
// (conformance/asset-rubrics/c2pa_signals_rubric_evaluator.py and the json-formula engine setup in
// c2pa_conformance_rubric_evaluator.py). Only the signals (AI disclosure) part is ported; the
// conformance and integrity evaluators are not. Keep it in step with the Python: the parity check
// in scripts/check-signals-parity.mjs compares the two.
import JsonFormula, {dataTypes} from '@adobe/json-formula';

/** Arity of each named expression, from call sites in all bodies and from `$args[N]` references. */
function inferArities(expressions) {
  const arities = Object.fromEntries(Object.keys(expressions).map(n => [n, 0]));
  const bodies = Object.values(expressions).map(v => (v == null ? '' : String(v)));
  for (const name of Object.keys(expressions)) {
    const call = new RegExp(`\\b${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*\\(`, 'g');
    for (const body of bodies) {
      for (const m of body.matchAll(call)) {
        const rest = body.slice(m.index + m[0].length);
        let count = 0;
        if (!rest.startsWith(')')) {
          let depth = 1;
          count = 1;
          for (const c of rest) {
            if (c === '(') depth++;
            else if (c === ')' && --depth === 0) break;
            else if (c === ',' && depth === 1) count++;
          }
        }
        arities[name] = Math.max(arities[name], count);
      }
    }
  }
  for (const [name, v] of Object.entries(expressions)) {
    const body = v == null ? '' : String(v);
    const idx = [...body.matchAll(/\$args\[(\d+)\]/g)].map(m => Number(m[1]));
    if (idx.length) arities[name] = Math.max(arities[name], Math.max(...idx) + 1);
    else if (/\$args\b/.test(body)) arities[name] = Math.max(arities[name], 1);
  }
  return arities;
}

/**
 * A json-formula engine whose custom functions are the rubric's named expressions. Arguments are
 * passed as the `$args` global, saved and restored around each call so nested calls can't
 * overwrite an outer call's arguments; `@` stays the call site's data.
 */
function createEngine(metadata) {
  const variables = {...(metadata.variables || {})};
  const expressions = {...(metadata.expressions || {})};
  const arities = inferArities(expressions);

  const dollarVars = new Set(Object.keys(variables));
  for (const v of Object.values(expressions)) {
    for (const m of String(v ?? '').matchAll(/\$[a-zA-Z0-9_]+/g)) dollarVars.add(m[0]);
  }
  const allowed = [...dollarVars, ...Object.keys(expressions)];
  const compiler = new JsonFormula();
  const customFunctions = {};
  for (const [name, v] of Object.entries(expressions)) {
    let ast = null;
    try {
      ast = compiler.compile(String(v ?? '').trim(), allowed);
    } catch {
      // Reported when called, as the Python does.
    }
    const arity = arities[name];
    const run = (data, interpreter) => {
      if (!ast) throw new Error(`Expression '${name}' failed to compile`);
      return interpreter.search(ast, data);
    };
    customFunctions[name] = {
      _func: arity === 0 ? (args, data, interpreter) => run(data, interpreter) : (args, data, interpreter) => {
        const saved = interpreter.globals.$args;
        interpreter.globals.$args = [...args];
        try {
          return run(data, interpreter);
        } finally {
          if (saved == null) delete interpreter.globals.$args;
          else interpreter.globals.$args = saved;
        }
      },
      _signature: Array.from({length: arity}, () => ({types: [dataTypes.TYPE_ANY]})),
    };
  }
  return {engine: new JsonFormula(customFunctions), variables};
}

function indexMapping(data) {
  const mapping = {};
  (data.manifests || []).forEach((m, i) => {
    if (m.label) mapping[m.label] = String(i);
  });
  return mapping;
}

/** Manifest nodes, their ingredient edges, and pseudo nodes for ingredients without a manifest. */
function buildProvenanceDag(data, mapping) {
  const dag = {};
  const manifests = data.manifests || [];
  for (const manifest of manifests) {
    if (!manifest.label) continue;
    const id = mapping[manifest.label] ?? 'Unknown';
    dag[id] = {urn: manifest.label, manifest, edges: [], signals: [], isPseudo: false};
    const claim = manifest['claim.v2'] || manifest.claim || {};
    if (claim['dc:format']) dag[id].mimeType = claim['dc:format'];
  }

  let next = manifests.length;
  for (const manifest of manifests) {
    const label = manifest.label;
    if (!label) continue;
    const id = mapping[label] ?? 'Unknown';
    const assertions = manifest.assertions || {};
    for (const [key, value] of Object.entries(assertions)) {
      if (key.startsWith('c2pa.ingredient')) {
        const url = value.activeManifest?.url;
        const relationship = value.relationship;
        if (url) {
          let parentUrn = url;
          const at = url.indexOf('urn:c2pa:');
          if (at >= 0) parentUrn = url.slice(at).split('/')[0];
          const parentId = mapping[parentUrn] ?? parentUrn;
          dag[id].edges.push({parent: parentId, relationship});
          const mime = value['dc:format'];
          if (mime && dag[parentId] && !('mimeType' in dag[parentId])) dag[parentId].mimeType = mime;
        } else {
          // No manifest: still a node if a source type is declared on the ingredient or, failing
          // that, on a c2pa.opened action in this manifest.
          let dst = value.digitalSourceType;
          if (!dst) {
            outer: for (const [aKey, aVal] of Object.entries(assertions)) {
              if (!aKey.startsWith('c2pa.actions')) continue;
              for (const act of aVal.actions || []) {
                if (act.action === 'c2pa.opened' && act.digitalSourceType) {
                  dst = act.digitalSourceType;
                  break outer;
                }
              }
            }
          }
          if (dst) {
            const pseudoId = String(next++);
            const safeTitle = (value['dc:title'] ?? 'flat-ingredient').replace(/[^a-zA-Z0-9_\-.]/g, '_');
            const pseudoLabel = `urn:c2pa:pseudo:${label}:${safeTitle}_${pseudoId}`;
            dag[pseudoId] = {
              urn: pseudoLabel,
              manifest: {
                label: pseudoLabel,
                assertions: {
                  'c2pa.actions.v2': {allActionsIncluded: true, actions: [{action: 'c2pa.created', digitalSourceType: dst}]},
                },
              },
              edges: [],
              signals: [],
              isPseudo: true,
              parentNodeId: id,
              mimeType: value['dc:format'],
            };
            dag[id].edges.push({parent: pseudoId, relationship});
          }
        }
      } else if (key.startsWith('c2pa.thumbnail')) {
        // Newer c2pa-rs crJSON gives a thumbnail's type as `mimeType` rather than `format` (the only
        // deviation from the Python, which reads `format`; it affects dc:format, not signals).
        const format = value.format ?? value.mimeType;
        if (format && !('mimeType' in dag[id])) dag[id].mimeType = format;
      }
    }
  }
  return dag;
}

function assertedBy(manifest) {
  const subject = manifest?.signature?.certificateInfo?.subject || {};
  const out = {CN: subject.CN ?? 'Unknown CN', O: subject.O ?? 'Unknown O'};
  if (subject.OU) out.OU = subject.OU;
  return {subject: out};
}

/** Truthiness as the Python evaluator decides it; `multiple` when a list has more than one item. */
function toSignal(val) {
  if (Array.isArray(val)) return {on: val.length > 0, multiple: val.length > 1};
  if (typeof val === 'boolean') return {on: val, multiple: false};
  if (typeof val === 'number') return {on: val > 0, multiple: false};
  return {on: val != null, multiple: false};
}

/**
 * Evaluates a signals rubric against crJSON. `rubric` is `{metadata, statements}`, the rubric's
 * YAML documents as JSON (see scripts/sync-signals-rubric.mjs). Returns the same shape as the
 * Python evaluator's output.
 */
export function evaluateSignals(rubric, data, {onError = () => {}} = {}) {
  const mapping = indexMapping(data);
  const dag = buildProvenanceDag(data, mapping);
  const {engine, variables} = createEngine(rubric.metadata);

  for (const [nodeId, node] of Object.entries(dag)) {
    for (const stmt of rubric.statements) {
      if (!stmt.expression || !stmt.id) continue;
      try {
        const {on, multiple} = toSignal(engine.search(stmt.expression.trim(), node.manifest, {...variables}));
        if (on) node.signals.push({trait: stmt.id, reportText: stmt.reportText?.true?.en ?? stmt.id, multiple});
      } catch (e) {
        onError(`Error evaluating ${stmt.id} on node ${nodeId}: ${e.message}`);
      }
    }
  }

  const order = Object.keys(dag).sort((a, b) => (dag[a].isPseudo - dag[b].isPseudo) || (Number(a) - Number(b)));
  const newIndex = Object.fromEntries(order.map((id, i) => [id, i]));
  const manifests = order.map(id => {
    const node = dag[id];
    const entry = {
      assertedBy: assertedBy(node.isPseudo ? dag[node.parentNodeId].manifest : node.manifest),
      'dc:format': node.mimeType ?? null,
      localInceptions: node.signals.filter(s => s.trait.startsWith('inception:')),
      localTransformations: node.signals.filter(s => s.trait.startsWith('transformation:')),
      ingredients: node.edges.flatMap(e => {
        const key = String(e.parent);
        if (key in newIndex) return [{index: newIndex[key], relationship: e.relationship}];
        const n = Number.parseInt(key, 10);
        return Number.isNaN(n) || String(n) !== key.trim() ? [] : [{index: n, relationship: e.relationship}];
      }),
    };
    if (node.isPseudo) entry.pseudo = true;
    return entry;
  });

  const meta = rubric.metadata.rubric_metadata || {};
  return {rubricName: meta.name ?? 'Unknown', rubricVersion: meta.version ?? 'Unknown', manifests};
}
