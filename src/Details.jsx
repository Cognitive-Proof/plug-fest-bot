import React, {useState, Component} from 'react';
import {C2paManifest} from 'c2pa-react-component';
import {AiDisclaimer} from 'c2pa-react-ai-disclaimer';
import {Signals} from './signals/Signals.jsx';
const LEVELS=[{level:2,label:'Summary'},{level:3,label:'Provenance graph'},{level:4,label:'Forensic'}];

// Keeps a manifest the components can't render from taking the whole page down.
class Guard extends Component{
 state={failed:false};
 static getDerivedStateFromError(){return {failed:true};}
 componentDidCatch(error){console.warn('Content Credentials view failed',error);}
 render(){return this.state.failed?<p className="details-empty">Plug-Fest Bot couldn’t lay out these credentials.</p>:this.props.children;}
}

export function Details({outcome,crjson}){
 const [level,setLevel]=useState(2);
 if(!outcome?.manifests?.length)return <div className="details" id="details"><div className="details-empty"><strong>No Content Credentials found</strong><span>This file carries no readable C2PA manifest, so there is no signed history to show.</span>{outcome?.error&&<code>{outcome.error}</code>}</div></div>;
 return <div className="details" id="details">
 <Guard><AiDisclaimer manifest={outcome} level={2}/></Guard>
 <div className="level-toggle" role="radiogroup" aria-label="Detail level">{LEVELS.map(l=><button key={l.level} role="radio" aria-checked={level===l.level} className={level===l.level?'active':''} onClick={()=>setLevel(l.level)}><span>{l.level}</span> {l.label}</button>)}</div>
 <div className="manifest-view"><Guard key={level}><C2paManifest manifest={outcome} level={level} plugin={[AiDisclaimer]} className="c2pa-card--fluid"/></Guard></div>
 <Guard><Signals crjson={crjson}/></Guard>
 </div>;
}
