"use strict";(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[5,7127,9508],{43685:(e,t,r)=>{r.d(t,{q:()=>s});var n=r(95155);function s({playing:e,onClick:t,className:r=""}){return(0,n.jsx)("button",{type:"button",onClick:t,"aria-label":e?"Pause animation":"Play animation",title:e?"Pause":"Play",className:`absolute top-2.5 right-2.5 z-10 inline-flex items-center justify-center w-7 h-7 rounded-md border border-stone-200 bg-white/90 backdrop-blur-sm text-stone-500 hover:text-stone-900 hover:bg-white hover:border-stone-300 transition-colors ${r}`,children:e?(0,n.jsxs)("svg",{viewBox:"0 0 10 10",className:"w-[10px] h-[10px]","aria-hidden":"true",children:[(0,n.jsx)("rect",{x:"1.5",y:"1",width:"2",height:"8",fill:"currentColor"}),(0,n.jsx)("rect",{x:"6.5",y:"1",width:"2",height:"8",fill:"currentColor"})]}):(0,n.jsx)("svg",{viewBox:"0 0 10 10",className:"w-[11px] h-[11px] translate-x-[0.5px]","aria-hidden":"true",children:(0,n.jsx)("polygon",{points:"2,1 9,5 2,9",fill:"currentColor"})})})}},60005:(e,t,r)=>{r.r(t),r.d(t,{RagPipelineStepper:()=>l});var n=r(95155),s=r(12115),o=r(89379),a=r(43685);let i=[{name:"parse",sub:"pdf/html → text",payload:`Input: handbook_2025.pdf (184 pages)

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

Total: 312 vectors, 312 bm25 postings
Disk: 9.4 MB dense, 1.1 MB sparse.

Ready to serve queries.`},{name:"retrieve",sub:"top-k over query",payload:`Query: "how long is paid parental leave?"

→ embed query  (d=3072)
→ dense top-10 by cosine
→ bm25 top-10, RRF-fuse

top-5:
  c_027  0.812  "Full-time employees are eligible..."
  c_028  0.604  "Short-term disability may be combined..."
  c_104  0.571  "Leave requests must be filed 30 days..."
  c_211  0.553  "Bereavement leave covers up to 5 days..."
  c_062  0.547  "Jury duty leave does not affect PTO..."`},{name:"rerank",sub:"cross-encoder on top-k",payload:`Reranker: bge-reranker-v2-m3

Input: (query, chunk) pairs
Scores after cross-encoder:
  c_027  8.91  ← jumps up
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

Latency: 420 ms prefill + 680 ms decode
Citations: [c_027], [c_104]  ✓ grounded`}];function l(){let{playing:e,playingRef:t,toggle:r}=(0,o.N)(),[l,d]=(0,s.useState)(0),c=(0,s.useRef)(null),p=(0,s.useRef)(null),u=(0,s.useRef)(0);(0,s.useEffect)(()=>{let e=r=>{null!==p.current&&t.current&&(u.current+=r-p.current,u.current>=4200&&(u.current=0,d(e=>(e+1)%i.length))),p.current=r,c.current=requestAnimationFrame(e)};return c.current=requestAnimationFrame(e),()=>{null!==c.current&&cancelAnimationFrame(c.current)}},[t]);let m=i[l],b=l<4;return(0,n.jsx)("div",{className:"not-prose my-8 relative",children:(0,n.jsxs)("div",{className:"border border-stone-200 rounded-lg bg-white shadow-sm relative overflow-hidden",children:[(0,n.jsx)(a.q,{playing:e,onClick:r}),(0,n.jsxs)("div",{className:"px-4 py-2.5 bg-stone-50 border-b border-stone-200 flex items-center gap-3",children:[(0,n.jsx)("span",{className:"font-mono text-xs font-bold px-2 py-0.5 rounded",style:{background:b?"#fefce8":"#eff6ff",color:b?"#a16207":"#1d4ed8",border:`1px solid ${b?"#fde68a":"#bfdbfe"}`},children:b?"ingest":"query"}),(0,n.jsxs)("span",{className:"font-mono text-xs text-stone-700 font-semibold",children:[l+1,". ",m.name]}),(0,n.jsx)("span",{className:"font-mono text-[11px] text-stone-500",children:m.sub})]}),(0,n.jsx)("div",{className:"flex items-center gap-1 px-4 pt-3 pb-2 overflow-x-auto",children:i.map((e,t)=>{let r=t<l,s=t===l;return(0,n.jsxs)("button",{onClick:()=>{u.current=0,d(t)},className:"flex items-center gap-1 group shrink-0",children:[(0,n.jsx)("div",{className:"px-2 py-1 rounded-md font-mono text-[10px] font-semibold transition-colors",style:{background:s?"#1c1917":r?"#e7e5e4":"#fafaf9",color:s?"white":r?"#292524":"#a8a29e",border:`1px solid ${s?"#1c1917":"#e7e5e4"}`},children:e.name}),t<i.length-1&&(0,n.jsx)("span",{className:"text-stone-300 text-[10px]",children:"›"})]},t)})}),(0,n.jsx)("div",{className:"px-4 pb-4",children:(0,n.jsx)("pre",{className:"font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-stone-800 bg-stone-50 border border-stone-200 rounded-md p-3 min-h-[260px]",children:m.payload})}),(0,n.jsxs)("div",{className:"flex items-center justify-between px-4 py-2 border-t border-stone-100 bg-stone-50",children:[(0,n.jsx)("button",{onClick:()=>{u.current=0,d(e=>Math.max(0,e-1))},disabled:0===l,className:"font-mono text-xs text-stone-500 hover:text-stone-900 disabled:opacity-30 px-2 py-1 rounded hover:bg-stone-100",children:"← prev"}),(0,n.jsxs)("span",{className:"font-mono text-[11px] text-stone-500",children:[e?"auto-advancing":"paused"," \xb7 stage ",l+1," / ",i.length]}),(0,n.jsx)("button",{onClick:()=>{u.current=0,d(e=>Math.min(i.length-1,e+1))},disabled:l===i.length-1,className:"font-mono text-xs text-stone-500 hover:text-stone-900 disabled:opacity-30 px-2 py-1 rounded hover:bg-stone-100",children:"next →"})]})]})})}},89379:(e,t,r)=>{r.d(t,{N:()=>s});var n=r(12115);function s(){let[e,t]=(0,n.useState)(!0),[r,s]=(0,n.useState)(!0),o=(0,n.useRef)(null);(0,n.useEffect)(()=>{window.matchMedia("(prefers-reduced-motion: reduce)").matches&&t(!1)},[]),(0,n.useEffect)(()=>{let e=o.current;if(!e||"u"<typeof IntersectionObserver)return;let t=new IntersectionObserver(([e])=>s(e.isIntersecting),{rootMargin:"200px 0px"});return t.observe(e),()=>t.disconnect()},[]);let a=e&&r,i=(0,n.useRef)(a);return i.current=a,{playing:a,playingRef:i,toggle:()=>t(e=>!e),play:()=>t(!0),pause:()=>t(!1),containerRef:o}}}}]);