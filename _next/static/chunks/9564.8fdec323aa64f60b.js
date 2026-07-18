"use strict";(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[7127,9508,9564],{43685:(e,t,s)=>{s.d(t,{q:()=>n});var r=s(95155);function n({playing:e,onClick:t,className:s=""}){return(0,r.jsx)("button",{type:"button",onClick:t,"aria-label":e?"Pause animation":"Play animation",title:e?"Pause":"Play",className:`absolute top-2.5 right-2.5 z-10 inline-flex items-center justify-center w-7 h-7 rounded-md border border-stone-200 bg-white/90 backdrop-blur-sm text-stone-500 hover:text-stone-900 hover:bg-white hover:border-stone-300 transition-colors ${s}`,children:e?(0,r.jsxs)("svg",{viewBox:"0 0 10 10",className:"w-[10px] h-[10px]","aria-hidden":"true",children:[(0,r.jsx)("rect",{x:"1.5",y:"1",width:"2",height:"8",fill:"currentColor"}),(0,r.jsx)("rect",{x:"6.5",y:"1",width:"2",height:"8",fill:"currentColor"})]}):(0,r.jsx)("svg",{viewBox:"0 0 10 10",className:"w-[11px] h-[11px] translate-x-[0.5px]","aria-hidden":"true",children:(0,r.jsx)("polygon",{points:"2,1 9,5 2,9",fill:"currentColor"})})})}},69564:(e,t,s)=>{s.r(t),s.d(t,{MCP_Handshake:()=>a});var r=s(95155),n=s(12115),o=s(89379),i=s(43685);let l=[{label:"initialize",dir:"→",body:`{
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
}`,note:"result comes back as structured content"},{label:"model resumes",dir:"\xb7",body:'assistant: "7 times 9 is 63."',note:"the result re-enters the context and the model finishes"}];function a(){let{playing:e,playingRef:t,toggle:s}=(0,o.N)(),[a,d]=(0,n.useState)(0),c=(0,n.useRef)(0),u=(0,n.useRef)(null),x=(0,n.useRef)(null);(0,n.useEffect)(()=>{let e=s=>{if(null!==x.current&&t.current){let e=s-x.current;c.current+=e,c.current>=2400&&(c.current=0,d(e=>(e+1)%l.length))}x.current=s,u.current=requestAnimationFrame(e)};return u.current=requestAnimationFrame(e),()=>{null!==u.current&&cancelAnimationFrame(u.current)}},[t]);let m=l[a],h="→"===m.dir,p="←"===m.dir,b="\xb7"===m.dir;return(0,r.jsx)("div",{className:"not-prose my-8 relative",children:(0,r.jsx)("div",{className:"border border-stone-200 rounded-lg bg-white overflow-hidden shadow-sm",children:(0,r.jsxs)("div",{className:"relative",children:[(0,r.jsx)(i.q,{playing:e,onClick:s}),(0,r.jsxs)("div",{className:"px-5 pt-5 pb-4 border-b border-stone-100 bg-stone-50/50",children:[(0,r.jsxs)("div",{className:"flex items-center gap-3 text-xs font-mono text-stone-500 mb-1 tracking-wider",children:[(0,r.jsxs)("span",{children:["STEP ",a+1," / ",l.length]}),(0,r.jsx)("span",{className:"text-stone-300",children:"\xb7"}),(0,r.jsx)("span",{children:"MCP LIFECYCLE"})]}),(0,r.jsx)("div",{className:"font-mono text-sm text-stone-800 font-medium",children:m.label})]}),(0,r.jsxs)("div",{className:"grid grid-cols-[1fr_56px_1fr] items-stretch",children:[(0,r.jsxs)("div",{className:`px-5 py-4 border-r border-stone-100 ${h||b?"bg-blue-50/40":"bg-white"}`,children:[(0,r.jsx)("div",{className:"text-[10px] font-mono text-blue-700 tracking-widest mb-1",children:"CLIENT (HOST)"}),(0,r.jsx)("div",{className:"text-xs text-stone-500",children:"Claude Code, Cursor, your app…"})]}),(0,r.jsx)("div",{className:"flex items-center justify-center bg-stone-50/40",children:(0,r.jsx)("div",{className:`text-2xl font-mono transition-all duration-500 ${h?"text-blue-600 translate-x-1":p?"text-violet-600 -translate-x-1":"text-stone-400"}`,style:{transitionTimingFunction:"cubic-bezier(0.4, 0, 0.2, 1)"},children:m.dir})}),(0,r.jsxs)("div",{className:`px-5 py-4 border-l border-stone-100 ${p?"bg-violet-50/40":"bg-white"}`,children:[(0,r.jsx)("div",{className:"text-[10px] font-mono text-violet-700 tracking-widest mb-1",children:"SERVER"}),(0,r.jsx)("div",{className:"text-xs text-stone-500",children:"Filesystem, GitHub, calc…"})]})]}),(0,r.jsx)("div",{className:"px-5 pt-4 pb-3 border-t border-stone-100 bg-[#faf8f1]",children:(0,r.jsx)("pre",{className:"text-[11.5px] leading-[1.55] font-mono text-stone-800 whitespace-pre overflow-x-auto min-h-[150px]",style:{transition:"opacity 900ms ease-in-out",opacity:1},children:m.body},a)}),(0,r.jsx)("div",{className:"px-5 py-3 border-t border-stone-100 bg-stone-50/40 text-xs text-stone-600 italic min-h-[38px]",children:m.note}),(0,r.jsx)("div",{className:"px-5 py-3 border-t border-stone-100 flex items-center gap-1.5",children:l.map((e,t)=>(0,r.jsx)("div",{className:`h-1 rounded-full transition-all duration-500 ${t===a?"w-8 bg-stone-700":t<a?"w-3 bg-stone-400":"w-3 bg-stone-200"}`},t))})]})})})}},89379:(e,t,s)=>{s.d(t,{N:()=>n});var r=s(12115);function n(){let[e,t]=(0,r.useState)(!0),[s,n]=(0,r.useState)(!0),o=(0,r.useRef)(null);(0,r.useEffect)(()=>{window.matchMedia("(prefers-reduced-motion: reduce)").matches&&t(!1)},[]),(0,r.useEffect)(()=>{let e=o.current;if(!e||"u"<typeof IntersectionObserver)return;let t=new IntersectionObserver(([e])=>n(e.isIntersecting),{rootMargin:"200px 0px"});return t.observe(e),()=>t.disconnect()},[]);let i=e&&s,l=(0,r.useRef)(i);return l.current=i,{playing:i,playingRef:l,toggle:()=>t(e=>!e),play:()=>t(!0),pause:()=>t(!1),containerRef:o}}}}]);