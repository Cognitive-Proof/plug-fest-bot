import React, {useState, useRef, useEffect} from 'react';
import {FileIcon, Robot} from './Robot.jsx';
import {Details} from './Details.jsx';
import {HowItWorks} from './HowItWorks.jsx';
import {Feedback} from './Feedback.jsx';
import {VERDICTS, SAMPLES} from './verdicts.js';
import {describeNounsAndVerbs} from 'c2pa-react-component-nouns-verbs';

const MIN_CHOMP=1500;
const wait=ms=>new Promise(r=>setTimeout(r,ms));

async function eat(file,sample,signal){
 const body=new FormData();body.append('file',file);if(sample)body.append('sample','true');
 let res;
 try{res=await fetch('/api/eat',{method:'POST',body,signal});}
 catch(e){if(e.name==='AbortError')throw e;throw new Error('Plug-Fest Bot’s kitchen isn’t answering. Is the server running?');}
 const data=await res.json().catch(()=>null);
 if(!res.ok)throw new Error(res.status===413?'That one’s too big for Plug-Fest Bot to swallow.':data?.error||data?.reason||`The server said ${res.status}.`);
 if(!data||!VERDICTS[data.verdict])throw new Error('Plug-Fest Bot got a reply he didn’t understand.');
 return data;
}

const hostOf=url=>{try{return new URL(url).host;}catch{return url;}};
const fmtSize=n=>n<1024?`${n} B`:n<1024*1024?`${(n/1024).toFixed(1)} KB`:`${(n/1024/1024).toFixed(1)} MB`;

export function App(){
 const [phase,setPhase]=useState('idle'),[drag,setDrag]=useState(false),[file,setFile]=useState(null),[snacks,setSnacks]=useState(0);
 const [result,setResult]=useState(null),[error,setError]=useState(''),[more,setMore]=useState(false),[howOpen,setHowOpen]=useState(false);
 const input=useRef(null),dragDepth=useRef(0),abort=useRef(null);
 useEffect(()=>()=>abort.current?.abort(),[]);
 const busy=phase==='feeding';
 const feed=async(chosen,isSample=false)=>{
  if(!chosen||busy)return;
  setFile({name:chosen.name,size:chosen.size});setDrag(false);dragDepth.current=0;setPhase('feeding');setResult(null);setError('');setMore(false);
  abort.current?.abort();const ctl=abort.current=new AbortController();
  const chomp=wait(MIN_CHOMP);
  try{const data=await eat(chosen,isSample,ctl.signal);await chomp;setResult(data);setPhase('done');setSnacks(x=>x+1);}
  catch(e){if(e.name==='AbortError')return;await chomp;setError(e.message);setPhase('error');}
 };
 const sample=async(name)=>{
  if(busy)return;
  try{const res=await fetch(`samples/${name}`);if(!res.ok)throw new Error();const blob=await res.blob();feed(new File([blob],name,{type:blob.type||'image/jpeg'}),true);}
  catch{setFile({name,size:0});setError('Plug-Fest Bot couldn’t find that sample.');setPhase('error');}
 };
 const v=result&&VERDICTS[result.verdict];
 const summary=result?describeNounsAndVerbs(result.outcome):'';
 const speech=drag?'Ooh, for me?':busy?'nom nom nom…':phase==='done'?v.speech:phase==='error'?'Blech.':'Got any files?';
 const headline=drag?'Drop it. He’s ready.':busy?'One very happy robot.':phase==='done'?v.headline:phase==='error'?'Plug-Fest Bot couldn’t digest that.':'Feed him a file.';
 return <div className="page"><header><a className="logo" href="./" aria-label="Plug-Fest Bot home"><span className="logo-face"><i/><i/></span>plug-fest bot<span className="logo-dot">.</span></a><span className="header-note"><span/> A friendly little AI taster</span><button className="about" onClick={()=>setHowOpen(true)}>How it works <span>↗</span></button></header>
 <main><div className="intro"><div className="eyebrow"><span/> SMALL ROBOT. BIG APPETITE.</div><h1>A little byte to eat.</h1><p>Meet Plug-Fest Bot. Feed him a file and he’ll taste its Content Credentials for AI.</p></div>
 <section className={`feeding-area ${drag?'dragging':''} ${busy?'busy':''}`} aria-label="Feed Plug-Fest Bot a file" onDragEnter={e=>{e.preventDefault();dragDepth.current++;if(!busy)setDrag(true);}} onDragOver={e=>{e.preventDefault();e.dataTransfer.dropEffect=busy?'none':'copy';}} onDragLeave={e=>{e.preventDefault();dragDepth.current--;if(dragDepth.current===0)setDrag(false);}} onDrop={e=>{e.preventDefault();setDrag(false);dragDepth.current=0;feed(e.dataTransfer.files[0]);}}>
 <div className="robot-stage"><div className="orbit orbit-one"/><div className="orbit orbit-two"/><span className="spark spark-one">✧</span><span className="spark spark-two">✧</span><div className="speech" key={speech}>{speech}<span/></div>
 {busy&&<div className="flying-file"><FileIcon size={40}/><span>{file.name.split('.').pop()?.slice(0,5).toUpperCase()}</span></div>}
 <Robot phase={phase} drag={drag}/><span className="robot-label">PLUG-FEST BOT / MODEL 001</span></div>
 <div className={`upload-card ${phase} ${v?`verdict-${result.verdict}`:''}`}><div className="upload-symbol">{v?<span>{v.icon}</span>:phase==='error'?<span>!</span>:<FileIcon size={24}/>}</div><div className="upload-copy"><h2>{headline}</h2><p>{file&&phase!=='idle'&&!drag?<span className="filename">{file.name} <span>· {fmtSize(file.size)}</span></span>:'Drop a file here, or pick something tasty.'}</p></div><input ref={input} type="file" hidden onChange={e=>{feed(e.target.files[0]);e.target.value='';}}/><button className="choose" disabled={busy} onClick={()=>input.current.click()}>{busy?'Munching…':phase==='idle'?'Choose a file':phase==='error'?'Try another':'Feed again'}{!busy&&<span>↗</span>}</button>
 {busy&&<div className="progress chomping" role="progressbar" aria-label="Plug-Fest Bot is checking your file"><div/></div>}
 {v&&<div className="verdict">{summary&&<p className="summary">{summary}</p>}<span className={`chip chip-${result.verdict}`}><span aria-hidden="true">{v.icon}</span>{v.label}</span><p className="reason">{result.reason}{result.remoteManifest&&<span className="remote-note"> Content Credentials fetched from {hostOf(result.remoteManifest)}.</span>}</p><button className="more" aria-expanded={more} aria-controls="details" onClick={()=>setMore(m=>!m)}>{more?'Show less':'Tell me more'} <span>{more?'↑':'↓'}</span></button>{result.id&&<Feedback key={result.id} id={result.id} verdict={result.verdict}/>}</div>}
 {phase==='error'&&<div className="verdict"><p className="reason">{error}</p></div>}</div>
 {more&&result&&<Details outcome={result.outcome}/>}
 <div className="below-card"><span><svg width="13" height="15" viewBox="0 0 16 18" fill="none" aria-hidden="true"><rect x="2" y="7" width="12" height="9" rx="2" stroke="currentColor" strokeWidth="1.5"/><path d="M5 7V4a3 3 0 0 1 6 0v3" stroke="currentColor" strokeWidth="1.5"/></svg> Files you upload are kept to help Plug-Fest Bot learn.</span><span className="samples"><span>Try a sample:</span>{SAMPLES.map(s=><button key={s.file} disabled={busy} onClick={()=>sample(s.file)}>{s.label}</button>)}</span></div>
 </section>
 <div className="how"><span className="how-title">GOOD FILES. GOOD MOOD.</span><p>Pick any file. Watch Plug-Fest Bot chomp. See what its credentials say.</p><span className="snack-count"><span>♡</span> {snacks===0?'Always hungry':`${snacks} ${snacks===1?'snack':'snacks'} enjoyed`}</span></div>
 <div className="sr-only" role="status" aria-live="polite">{phase==='done'?`Plug-Fest Bot tasted ${file.name}: ${v.label}. ${result.reason}`:phase==='error'?`Plug-Fest Bot couldn’t digest that. ${error}`:busy?'Plug-Fest Bot is checking your file.':''}</div>
 </main><footer><span>A small moment of joy, one file at a time.</span><span>Made for Plug-Fest <span className="tiny-heart">♥</span></span></footer>
 <HowItWorks open={howOpen} onClose={()=>setHowOpen(false)}/></div>
}
