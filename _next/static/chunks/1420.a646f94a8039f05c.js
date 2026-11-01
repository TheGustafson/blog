"use strict";(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[1420,8343],{23781:(e,t,s)=>{s.d(t,{q:()=>n});var r=s(47617);function n({playing:e,onClick:t,className:s=""}){return(0,r.jsx)("button",{type:"button",onClick:t,"aria-label":e?"Pause animation":"Play animation",title:e?"Pause":"Play",className:`absolute top-2.5 right-2.5 z-10 inline-flex items-center justify-center w-7 h-7 rounded-md border border-stone-200 bg-white/90 backdrop-blur-sm text-stone-500 hover:text-stone-900 hover:bg-white hover:border-stone-300 transition-colors ${s}`,children:e?(0,r.jsxs)("svg",{viewBox:"0 0 10 10",className:"w-[10px] h-[10px]","aria-hidden":"true",children:[(0,r.jsx)("rect",{x:"1.5",y:"1",width:"2",height:"8",fill:"currentColor"}),(0,r.jsx)("rect",{x:"6.5",y:"1",width:"2",height:"8",fill:"currentColor"})]}):(0,r.jsx)("svg",{viewBox:"0 0 10 10",className:"w-[11px] h-[11px] translate-x-[0.5px]","aria-hidden":"true",children:(0,r.jsx)("polygon",{points:"2,1 9,5 2,9",fill:"currentColor"})})})}},31420:(e,t,s)=>{s.r(t),s.d(t,{MCP_Handshake:()=>a});var r=s(47617),n=s(81253),i=s(60707),o=s(23781);let l=[{label:"initialize",dir:"→",body:`{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": { "protocolVersion": "2025-06-18" }
}`,note:"client says hello, declares protocol version"},{label:"initialize (response)",dir:"←",body:`{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "capabilities": { "tools": {}, "resources": {} },
    "serverInfo": { "name": "calc", "version": "0.1" }
  }
}`,note:"server reports what it offers"},{label:"tools/list",dir:"→",body:`{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}`,note:"client asks: what tools do you expose?"},{label:"tools/list (response)",dir:"←",body:`{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      { "name": "add", "inputSchema": { ... } },
      { "name": "multiply", "inputSchema": { ... } }
    ]
  }
}`,note:"server returns schemas the model can read"},{label:"user prompt → model decides",dir:"\xb7",body:`user: "What is 7 times 9?"
model emits tool_use:
  { "name": "multiply", "args": { "a": 7, "b": 9 } }`,note:"the model picks a tool based on the listed schemas"},{label:"tools/call",dir:"→",body:`{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "multiply",
    "arguments": { "a": 7, "b": 9 }
  }
}`,note:"host forwards the call to the MCP server"},{label:"tools/call (response)",dir:"←",body:`{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      { "type": "text", "text": "63" }
    ]
  }
}`,note:"result comes back as structured content"},{label:"model resumes",dir:"\xb7",body:'assistant: "7 times 9 is 63."',note:"the result re-enters the context and the model finishes"}];function a(){let{playing:e,playingRef:t,toggle:s,containerRef:a}=(0,i.N)(),[c,d]=(0,n.useState)(0),u=(0,n.useRef)(0),m=(0,n.useRef)(null),x=(0,n.useRef)(null);(0,n.useEffect)(()=>{let e=s=>{if(null!==x.current&&t.current){let e=s-x.current;u.current+=e,u.current>=2400&&(u.current=0,d(e=>(e+1)%l.length))}x.current=s,m.current=requestAnimationFrame(e)};return m.current=requestAnimationFrame(e),()=>{null!==m.current&&cancelAnimationFrame(m.current)}},[t]),(0,n.useEffect)(()=>{if(!window.matchMedia("(prefers-reduced-motion: reduce)").matches)return;let e=requestAnimationFrame(()=>{d(l.length-1)});return()=>cancelAnimationFrame(e)},[]);let h=l[c],p="→"===h.dir,b="←"===h.dir,f="\xb7"===h.dir;return(0,r.jsxs)("div",{ref:a,"data-series-visual":"MCP_Handshake",className:"series-figure arc-ten-figure not-prose my-8 relative",children:[(0,r.jsx)("div",{className:"arc-ten-heading",children:"Establishing an MCP connection"}),(0,r.jsx)("div",{className:"arc-ten-surface border border-stone-200 rounded-lg bg-white overflow-hidden shadow-sm",children:(0,r.jsxs)("div",{className:"relative",children:[(0,r.jsx)(o.q,{playing:e,onClick:s}),(0,r.jsxs)("div",{className:"px-5 pt-5 pb-4 border-b border-stone-100 bg-stone-50/50",children:[(0,r.jsxs)("div",{className:"flex items-center gap-3 text-xs font-mono text-stone-500 mb-1 tracking-wider",children:[(0,r.jsxs)("span",{children:["STEP ",c+1," / ",l.length]}),(0,r.jsx)("span",{className:"text-stone-300",children:"\xb7"}),(0,r.jsx)("span",{children:"MCP LIFECYCLE"})]}),(0,r.jsx)("div",{className:"font-mono text-sm text-stone-800 font-medium",children:h.label})]}),(0,r.jsxs)("div",{className:"grid grid-cols-[1fr_56px_1fr] items-stretch",children:[(0,r.jsxs)("div",{className:`px-5 py-4 border-r border-stone-100 ${p||f?"bg-blue-50/40":"bg-white"}`,children:[(0,r.jsx)("div",{className:"text-[10px] font-mono text-blue-700 tracking-widest mb-1",children:"CLIENT (HOST)"}),(0,r.jsx)("div",{className:"text-xs text-stone-500",children:"Claude Code, Cursor, your app…"})]}),(0,r.jsx)("div",{className:"flex items-center justify-center bg-stone-50/40",children:(0,r.jsx)("div",{className:`text-2xl font-mono transition-all duration-500 ${p?"text-blue-600 translate-x-1":b?"text-violet-600 -translate-x-1":"text-stone-400"}`,style:{transitionTimingFunction:"cubic-bezier(0.4, 0, 0.2, 1)"},children:h.dir})}),(0,r.jsxs)("div",{className:`px-5 py-4 border-l border-stone-100 ${b?"bg-violet-50/40":"bg-white"}`,children:[(0,r.jsx)("div",{className:"text-[10px] font-mono text-violet-700 tracking-widest mb-1",children:"SERVER"}),(0,r.jsx)("div",{className:"text-xs text-stone-500",children:"Filesystem, GitHub, calc…"})]})]}),(0,r.jsx)("div",{className:"px-5 pt-4 pb-3 border-t border-stone-100 bg-[#faf8f1]",children:(0,r.jsx)("pre",{className:"text-[11.5px] leading-[1.55] font-mono text-stone-800 whitespace-pre overflow-x-auto min-h-[150px]",style:{transition:"opacity 900ms ease-in-out",opacity:1},children:h.body},c)}),(0,r.jsx)("div",{className:"px-5 py-3 border-t border-stone-100 bg-stone-50/40 text-xs text-stone-600 italic min-h-[38px]",children:h.note}),(0,r.jsx)("div",{className:"px-5 py-3 border-t border-stone-100 flex items-center gap-1.5",children:l.map((e,t)=>(0,r.jsx)("div",{className:`h-1 rounded-full transition-all duration-500 ${t===c?"w-8 bg-stone-700":t<c?"w-3 bg-stone-400":"w-3 bg-stone-200"}`},t))})]})})]})}},60707:(e,t,s)=>{s.d(t,{N:()=>n});var r=s(81253);function n(){let[e,t]=(0,r.useState)(!0),[s,n]=(0,r.useState)(!0),i=(0,r.useRef)(null);(0,r.useEffect)(()=>{window.matchMedia("(prefers-reduced-motion: reduce)").matches&&t(!1)},[]),(0,r.useEffect)(()=>{let e=i.current;if(!e||"u"<typeof IntersectionObserver)return;let t=new IntersectionObserver(([e])=>n(e.isIntersecting),{rootMargin:"200px 0px"});return t.observe(e),()=>t.disconnect()},[]);let o=e&&s,l=(0,r.useRef)(o);return l.current=o,{playing:o,playingRef:l,toggle:()=>t(e=>!e),play:()=>t(!0),pause:()=>t(!1),containerRef:i}}}}]);