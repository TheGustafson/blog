"use strict";(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[5,7127],{43685:(e,t,r)=>{r.d(t,{q:()=>a});var n=r(95155);function a({playing:e,onClick:t,className:r=""}){return(0,n.jsx)("button",{type:"button",onClick:t,"aria-label":e?"Pause animation":"Play animation",title:e?"Pause":"Play",className:`absolute top-2.5 right-2.5 z-10 inline-flex items-center justify-center w-7 h-7 rounded-md border border-stone-200 bg-white/90 backdrop-blur-sm text-stone-500 hover:text-stone-900 hover:bg-white hover:border-stone-300 transition-colors ${r}`,children:e?(0,n.jsxs)("svg",{viewBox:"0 0 10 10",className:"w-[10px] h-[10px]","aria-hidden":"true",children:[(0,n.jsx)("rect",{x:"1.5",y:"1",width:"2",height:"8",fill:"currentColor"}),(0,n.jsx)("rect",{x:"6.5",y:"1",width:"2",height:"8",fill:"currentColor"})]}):(0,n.jsx)("svg",{viewBox:"0 0 10 10",className:"w-[11px] h-[11px] translate-x-[0.5px]","aria-hidden":"true",children:(0,n.jsx)("polygon",{points:"2,1 9,5 2,9",fill:"currentColor"})})})}},60005:(e,t,r)=>{r.r(t),r.d(t,{RagPipelineStepper:()=>l});var n=r(95155),a=r(12115),s=r(89379),o=r(43685);let i=[{name:"parse",sub:"pdf/html → text",payload:`Input: handbook_2025.pdf (184 pages)

Extracted text (first 600 chars):
"Section 7.3 Parental Leave. Full-time employees
are eligible for up to 16 weeks of paid parental
leave following the birth or adoption of a child.
Eligibility begins after 12 months of continuous
employment. Part-time employees receive a
pro-rated benefit. For details on combining
leave with short-term disability, see Section
7.4 ..."`},{name:"chunk",sub:"split into passages",payload:`Chunker: recursive, 400 tokens, 60 overlap
→ 312 chunks

chunk_id=c_027  (section 7.3, page 41)
  "Full-time employees are eligible for up to 16
  weeks of paid parental leave..."

chunk_id=c_028  (section 7.4, page 42)
  "Short-term disability may be combined with
  parental leave in the following cases..."`},{name:"embed",sub:"vectors per chunk",payload:`Embed model: text-embedding-3-large (d=3072)

c_027 → [0.018, -0.042, 0.091, ..., 0.003]
c_028 → [0.021, -0.019, 0.074, ..., -0.012]
...
c_311 → [-0.004, 0.033, 0.055, ..., 0.027]

Write to index with metadata:
{section, page, doc_id, date}`},{name:"index",sub:"vector store + bm25",payload:`Dense index: HNSW, M=32, efConstruction=200
Sparse index: BM25 (for hybrid retrieval)

Total: 312 vectors, 312 indexed documents
Float32 vector payload: 312 \xd7 3072 \xd7 4 = 3,833,856 bytes.
Graph, metadata and lexical-index overhead excluded.

Ready to serve queries.`},{name:"retrieve",sub:"top-k over query",payload:`Query: "how long is paid parental leave?"

→ embed query  (d=3072)
→ dense top-10 by cosine
→ bm25 top-10, RRF-fuse

top-5:
  c_027  0.0328  "Full-time employees are eligible..."
  c_028  0.0315  "Short-term disability may be combined..."
  c_104  0.0308  "Leave requests must be filed 30 days..."
  c_211  0.0299  "Bereavement leave covers up to 5 days..."
  c_062  0.0288  "Jury duty leave does not affect PTO..."`},{name:"rerank",sub:"cross-encoder on top-k",payload:`Reranker: bge-reranker-v2-m3

Input: (query, chunk) pairs
Scores after cross-encoder:
  c_027  8.91  ← remains first
  c_028  3.22
  c_104  1.05
  c_211 -0.44
  c_062 -0.71

Keep top-3 → [c_027, c_028, c_104]`},{name:"assemble",sub:"build the prompt",payload:`System: "Answer using ONLY the context.
Cite chunk ids in brackets."

Context:
  [c_027] Full-time employees are eligible for up
  to 16 weeks of paid parental leave...
  [c_028] Short-term disability may be combined
  with parental leave...
  [c_104] Leave requests must be filed 30 days in
  advance via the HR portal.

User: how long is paid parental leave?`},{name:"generate",sub:"final answer",payload:`Model output:

"Full-time employees receive up to 16 weeks of
paid parental leave following the birth or
adoption of a child [c_027]. Part-time employees
get a pro-rated amount. Leave should be
requested 30 days in advance [c_104]."

No model call or latency measurement runs in this example.
Citations: [c_027], [c_104]  identifiers found; claim support still needs checking`}];function l({caption:e}={}){let{playing:t,playingRef:r,toggle:d,containerRef:c}=(0,s.N)(),[p,u]=(0,a.useState)(0),m=(0,a.useRef)(null),x=(0,a.useRef)(null),h=(0,a.useRef)(0);(0,a.useEffect)(()=>{let e=t=>{null!==x.current&&r.current&&(h.current+=t-x.current,h.current>=4200&&(h.current=0,u(e=>(e+1)%i.length))),x.current=t,m.current=requestAnimationFrame(e)};return m.current=requestAnimationFrame(e),()=>{null!==m.current&&cancelAnimationFrame(m.current)}},[r]);let b=i[p],f=p<4;return(0,n.jsxs)("div",{ref:c,className:"not-prose my-8 relative",children:[(0,n.jsxs)("div",{className:"border border-stone-200 rounded-lg bg-white shadow-sm relative overflow-hidden",children:[(0,n.jsx)(o.q,{playing:t,onClick:d}),(0,n.jsxs)("div",{className:"pl-4 pr-14 py-2.5 bg-stone-50 border-b border-stone-200 flex flex-wrap items-center gap-3",children:[(0,n.jsx)("span",{className:"font-mono text-xs font-bold px-2 py-0.5 rounded",style:{background:f?"#fefce8":"#eff6ff",color:f?"#a16207":"#1d4ed8",border:`1px solid ${f?"#fde68a":"#bfdbfe"}`},children:f?"constructed ingest":"constructed query"}),(0,n.jsxs)("span",{className:"font-mono text-xs text-stone-700 font-semibold",children:[p+1,". ",b.name]}),(0,n.jsx)("span",{className:"font-mono text-[11px] text-stone-500",children:b.sub})]}),(0,n.jsx)("div",{className:"flex flex-wrap items-center gap-2 px-4 pt-3 pb-2",children:i.map((e,r)=>{let a=r<p,s=r===p;return(0,n.jsxs)("button",{type:"button","aria-pressed":p===r,onClick:()=>{h.current=0,t&&d(),u(r)},className:"flex items-center gap-1 group shrink-0",children:[(0,n.jsx)("div",{className:"px-2 py-2 rounded-md font-mono text-xs font-semibold transition-colors",style:{background:s?"#1c1917":a?"#e7e5e4":"#fafaf9",color:s?"white":a?"#292524":"#a8a29e",border:`1px solid ${s?"#1c1917":"#e7e5e4"}`},children:e.name}),r<i.length-1&&(0,n.jsx)("span",{"aria-hidden":"true",className:"text-stone-300 text-[10px]",children:"→"})]},r)})}),(0,n.jsx)("div",{className:"px-4 pb-4",children:(0,n.jsx)("pre",{className:"font-mono text-[11px] leading-relaxed whitespace-pre-wrap break-words text-stone-800 bg-stone-50 border border-stone-200 rounded-md p-3 min-h-[260px]",children:b.payload})}),(0,n.jsxs)("div",{className:"flex items-center justify-between px-4 py-2 border-t border-stone-100 bg-stone-50",children:[(0,n.jsx)("button",{onClick:()=>{h.current=0,t&&d(),u(e=>Math.max(0,e-1))},disabled:0===p,className:"font-mono text-xs text-stone-500 hover:text-stone-900 disabled:opacity-30 px-2 py-1 rounded hover:bg-stone-100",children:"← prev"}),(0,n.jsxs)("span",{className:"font-mono text-[11px] text-stone-500",children:[t?"auto-advancing":"paused"," \xb7 stage ",p+1," / ",i.length]}),(0,n.jsx)("button",{onClick:()=>{h.current=0,t&&d(),u(e=>Math.min(i.length-1,e+1))},disabled:p===i.length-1,className:"font-mono text-xs text-stone-500 hover:text-stone-900 disabled:opacity-30 px-2 py-1 rounded hover:bg-stone-100",children:"next →"})]})]}),e&&(0,n.jsx)("p",{className:"mt-4 text-sm leading-relaxed text-stone-500",children:e})]})}},89379:(e,t,r)=>{r.d(t,{N:()=>a});var n=r(12115);function a(){let[e,t]=(0,n.useState)(!0),[r,a]=(0,n.useState)(!0),s=(0,n.useRef)(null);(0,n.useEffect)(()=>{window.matchMedia("(prefers-reduced-motion: reduce)").matches&&t(!1)},[]),(0,n.useEffect)(()=>{let e=s.current;if(!e||"u"<typeof IntersectionObserver)return;let t=new IntersectionObserver(([e])=>a(e.isIntersecting),{rootMargin:"200px 0px"});return t.observe(e),()=>t.disconnect()},[]);let o=e&&r,i=(0,n.useRef)(o);return i.current=o,{playing:o,playingRef:i,toggle:()=>t(e=>!e),play:()=>t(!0),pause:()=>t(!1),containerRef:s}}}}]);