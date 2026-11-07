import React from 'react';

export function FileIcon({size=24}) {return <svg width={size} height={size} viewBox="0 0 32 36" fill="none" aria-hidden="true"><path d="M7 2h12l9 9v21a2 2 0 0 1-2 2H7a3 3 0 0 1-3-3V5a3 3 0 0 1 3-3Z" stroke="currentColor" strokeWidth="2"/><path d="M19 2v10h9M10 20h12M10 26h8" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/></svg>}
export function Robot({phase,drag}) {return <svg className={`robot ${phase} ${drag?'excited':''}`} viewBox="0 0 440 360" role="img" aria-label={phase==='feeding'?'Plug-Fest Bot happily munching your file':'Plug-Fest Bot, a cute green robot with big eyes and an eager smile'}>
 <ellipse cx="220" cy="337" rx="111" ry="10" fill="#233d33" opacity=".08"/>
 <g className="robot-body">
 <path d="M220 66V43" stroke="#2f4a40" strokeWidth="8" strokeLinecap="round"/><circle cx="220" cy="32" r="12" fill="#b6dc7e"/><circle cx="216" cy="28" r="3" fill="#eaf7ce"/>
 <g className="arm left-arm"><path d="M105 215C70 222 57 204 51 183" fill="none" stroke="#779a64" strokeWidth="17" strokeLinecap="round"/><path d="M39 163l10 20 13-18" fill="none" stroke="#9dbe7e" strokeWidth="13" strokeLinecap="round" strokeLinejoin="round"/></g>
 <g className="arm right-arm"><path d="M335 215c31 6 46-5 53-27" fill="none" stroke="#779a64" strokeWidth="17" strokeLinecap="round"/><path d="M378 168l9 20 15-17" fill="none" stroke="#9dbe7e" strokeWidth="13" strokeLinecap="round" strokeLinejoin="round"/></g>
 <rect x="141" y="292" width="46" height="39" rx="16" fill="#779a64"/><rect x="253" y="292" width="46" height="39" rx="16" fill="#779a64"/>
 <rect x="87" y="145" width="29" height="68" rx="14" fill="#779a64"/><rect x="324" y="145" width="29" height="68" rx="14" fill="#779a64"/>
 <rect x="103" y="69" width="234" height="244" rx="72" fill="#9ebc7e"/>
 <rect x="103" y="64" width="234" height="239" rx="72" fill="#c4e69a"/>
 <path d="M132 115c9-17 21-24 38-27" stroke="#e6f6ca" strokeWidth="8" strokeLinecap="round"/>
 <g className="eyes"><ellipse cx="173" cy="151" rx="39" ry="43" fill="#fffef5"/><ellipse cx="267" cy="151" rx="39" ry="43" fill="#fffef5"/><g className="pupils"><ellipse cx="181" cy="158" rx="15" ry="19" fill="#273c35"/><ellipse cx="259" cy="158" rx="15" ry="19" fill="#273c35"/><circle cx="185" cy="152" r="5" fill="#fff"/><circle cx="263" cy="152" r="5" fill="#fff"/></g></g>
 <ellipse cx="140" cy="204" rx="14" ry="8" fill="#e5a994" opacity=".8"/><ellipse cx="300" cy="204" rx="14" ry="8" fill="#e5a994" opacity=".8"/>
 <g className="mouth"><path d="M163 214Q220 205 277 214V231c0 35-24 48-57 48s-57-13-57-48Z" fill="#273c35"/><path d="M193 215h24v14a5 5 0 0 1-5 5h-14a5 5 0 0 1-5-5Z" fill="#fffef5"/><ellipse cx="231" cy="273" rx="29" ry="15" fill="#edaa95"/></g>
 <circle cx="219" cy="292" r="3" fill="#91b176"/>
 </g></svg>}
