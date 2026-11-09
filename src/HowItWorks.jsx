import React, {useEffect, useRef} from 'react';

// Plain-language summary of "The policy" in c2pa-ai-examples/README.md.
const STEPS=[
 ['Read the credentials','Plug-Fest Bot looks for a C2PA manifest (Content Credentials) in your file, or fetches it when the file links to one online (as Adobe’s cloud Content Credentials do). If there isn’t one, or its signature or validation fails, the answer is Unknown.'],
 ['Skip update-only steps','Steps that only open, publish or edit metadata can’t change the content, so they take the answer of the file they came from.'],
 ['Find the origin','How was the content first made? Plug-Fest Bot reads the declared source type (camera capture, digital art, trained model…), or follows the file it was opened from and checks that the same way.'],
 ['Check the ingredients','Pieces placed into the content are checked the same way. Pieces later deleted, and model inputs like prompts or reference images, don’t count.'],
 ['Look at later edits','Crops, rotations and other preserving edits change nothing. Edits that may add content need to say whether AI made it. Undeclared or unrecognised edits, redacted history, or an AI disclosure with no source type make the answer unclear.'],
 ['Decide','AI if everything is AI and the history is complete. Includes AI if any generative AI is declared in the current content. Unknown if anything is unclear or steps may be missing. Otherwise, No AI.'],
];

export function HowItWorks({open,onClose}){
 const ref=useRef(null);
 useEffect(()=>{const d=ref.current;if(!d)return;if(open&&!d.open)d.showModal();if(!open&&d.open)d.close();},[open]);
 return <dialog ref={ref} className="how-dialog" aria-labelledby="how-title" onClose={onClose} onClick={e=>{if(e.target===ref.current)onClose();}}>
 <div className="how-inner">
 <div className="how-head"><div><div className="eyebrow"><span/> HOW IT WORKS</div><h2 id="how-title">How Plug-Fest Bot tastes a file</h2></div><button className="close" onClick={onClose} aria-label="Close">×</button></div>
 <p className="how-lede">Your file goes to our server, which reads its Content Credentials with c2pa-rs and walks the signed history. We keep a copy of each file you upload, with its verdict and any Right / Wrong answer you give, so we can find the files Plug-Fest Bot gets wrong.</p>
 <figure className="flow"><a href="how-it-works.svg" target="_blank" rel="noopener"><img src="how-it-works.svg" width="1491" height="1872" loading="lazy" alt="Flowchart of how Plug-Fest Bot decides a file’s verdict, following the steps below."/></a><figcaption><a href="how-it-works.svg" target="_blank" rel="noopener">Open full size ↗</a></figcaption></figure>
 <ol className="steps">{STEPS.map(([title,body])=><li key={title}><strong>{title}</strong><span>{body}</span></li>)}</ol>
 <p className="how-note"><strong>Good to know:</strong> the verdict describes what the file’s signed provenance <em>declares</em>. Plug-Fest Bot doesn’t analyse the pixels, so a file with no credentials, or credentials that leave things out, can’t be judged.</p>
 </div>
 </dialog>;
}
