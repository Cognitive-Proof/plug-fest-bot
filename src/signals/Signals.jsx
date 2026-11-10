import React, {useMemo} from 'react';
import rubric from './asset-rubric-signals-local.json';
import {evaluateSignals} from './evaluator.js';

// How each signal reads for AI: declared generative AI, possibly AI or unclear, or neither.
const TONE = {
  'inception:signal_fullyGenAIMedia': 'ai',
  'inception:signal_partlyGenAICreation': 'ai',
  'transformation:signal_perceptibleGenAI': 'ai',
  'inception:signal_compositionMayContainGenAI': 'maybe',
  'inception:signal_screenCaptureMayContainGenAI': 'maybe',
  'inception:signal_mediaUnknownProvenance': 'maybe',
  'transformation:signal_perceptiblePossiblyGenAI': 'maybe',
  'transformation:signal_ambiguousActions': 'maybe',
};
// An ingredient's role in the step that uses it.
const RELATIONSHIP = {parentOf: 'parent of', componentOf: 'placed in', inputTo: 'input to'};

/** The C2PA Conformance Task Force's local signals for each step of the file's history. */
export function Signals({crjson}) {
  const result = useMemo(() => {
    try {
      return crjson?.manifests?.length ? evaluateSignals(rubric, crjson) : null;
    } catch (e) {
      console.warn('Signals rubric failed', e);
      return null;
    }
  }, [crjson]);
  if (!result) return null;

  const title = i => {
    const claim = crjson.manifests[i]?.['claim.v2'] || crjson.manifests[i]?.claim;
    return result.manifests[i].pseudo ? 'Unsigned ingredient' : claim?.['dc:title'] || `Step ${i + 1}`;
  };
  // Which step each step is an ingredient of.
  const usedIn = {};
  result.manifests.forEach((m, i) => m.ingredients.forEach(g => (usedIn[g.index] ??= []).push({parent: i, relationship: g.relationship})));

  return <section className="signals" aria-labelledby="signals-title">
    <div className="signals-head">
      <h3 id="signals-title">C2PA signals</h3>
      <a href={`${rubric.source.repository}/blob/${rubric.source.commit}/${rubric.source.path}`} target="_blank" rel="noopener">Task Force rubric v{result.rubricVersion} ↗</a>
    </div>
    <p className="signals-note">What the Conformance Task Force’s signals rubric finds in each signed step on its own. Plug-Fest Bot’s verdict above weighs the whole history.</p>
    <ol className="signals-steps">{result.manifests.map((m, i) => {
      const signals = [...m.localInceptions, ...m.localTransformations];
      return <li key={i}>
        <div className="signals-step">
          <strong>{i === 0 ? 'This file' : title(i)}</strong>
          {i === 0 && title(0) !== 'Step 1' && <span>{title(0)}</span>}
          {(usedIn[i] || []).map((u, k) => <span key={k}>{RELATIONSHIP[u.relationship] || u.relationship || 'ingredient of'} {u.parent === 0 ? 'this file' : title(u.parent)}</span>)}
        </div>
        <div className="signals-list">{signals.length
          ? signals.map(s => <span key={s.trait} className={`signal signal-${TONE[s.trait] || 'none'}`} title={s.trait}>{s.reportText}</span>)
          : <span className="signal-empty">No signals</span>}</div>
      </li>;
    })}</ol>
  </section>;
}
