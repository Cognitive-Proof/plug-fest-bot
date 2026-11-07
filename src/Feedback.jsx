import React, {useEffect, useRef, useState} from 'react';
import {VERDICTS} from './verdicts.js';

async function send(id,correct,expected){
 const res=await fetch('/api/feedback',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({id,correct,expected})});
 if(!res.ok)throw new Error();
}

// "Is this right?" under a verdict. Wrong opens a dialog to pick what it should have been.
export function Feedback({id,verdict}){
 const [status,setStatus]=useState('ask'),[open,setOpen]=useState(false),[pick,setPick]=useState('');
 const ref=useRef(null);
 useEffect(()=>{const d=ref.current;if(!d)return;if(open&&!d.open)d.showModal();if(!open&&d.open)d.close();},[open]);
 const submit=async(correct,expected)=>{
  setStatus('sending');
  try{await send(id,correct,expected);setStatus(correct?'right':'wrong');setOpen(false);}
  catch{setStatus('error');}
 };
 if(status==='right')return <div className="feedback"><p>Thanks! Plug-Fest Bot is glad he got it right.</p></div>;
 if(status==='wrong')return <div className="feedback"><p>Thanks! Noted that this should be <strong>{VERDICTS[pick].label}</strong>.</p></div>;
 const sending=status==='sending';
 return <div className="feedback">
  <p>{status==='error'&&!open?'Couldn’t save that. Try again?':'Did Plug-Fest Bot get it right?'}</p>
  <div className="feedback-buttons"><button disabled={sending} onClick={()=>submit(true)}>Right</button><button disabled={sending} onClick={()=>{setPick('');setStatus('ask');setOpen(true);}}>Wrong</button></div>
  <dialog ref={ref} className="how-dialog feedback-dialog" aria-labelledby="feedback-title" onClose={()=>setOpen(false)} onClick={e=>{if(e.target===ref.current)setOpen(false);}}>
   <form className="how-inner" onSubmit={e=>{e.preventDefault();if(pick)submit(false,pick);}}>
    <div className="how-head"><div><div className="eyebrow"><span/> HELP PLUG-FEST BOT LEARN</div><h2 id="feedback-title">What should it be?</h2></div><button type="button" className="close" onClick={()=>setOpen(false)} aria-label="Close">×</button></div>
    <p className="how-lede">Plug-Fest Bot said <strong>{VERDICTS[verdict].label}</strong>. Pick the right answer for this file.</p>
    <div className="feedback-options" role="radiogroup">{Object.entries(VERDICTS).filter(([key])=>key!==verdict).map(([key,v])=><label key={key} className={pick===key?'selected':''}><input type="radio" name="expected" value={key} checked={pick===key} onChange={()=>setPick(key)}/><span className={`chip chip-${key}`}><span aria-hidden="true">{v.icon}</span>{v.label}</span></label>)}</div>
    {status==='error'&&<p className="feedback-error">Couldn’t save that. Try again?</p>}
    <div className="feedback-actions"><button type="button" onClick={()=>setOpen(false)}>Cancel</button><button type="submit" className="primary" disabled={!pick||sending}>{sending?'Saving…':'Submit'}</button></div>
   </form>
  </dialog>
 </div>;
}
