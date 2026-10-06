(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[6052],{14155:(e,t,s)=>{"use strict";s.d(t,{MCP_PrimitivesPanel:()=>i});var o=s(95155),r=s(5790);let n=[{key:"tools",label:"Tools",tagline:"model-invocable actions",who:"the model can request them; the host controls execution",listMethod:"tools/list",callMethod:"tools/call",example:`{
  "name": "github_create_issue",
  "description": "Open a new GitHub issue in a repo.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "repo":  { "type": "string" },
      "title": { "type": "string" },
      "body":  { "type": "string" }
    },
    "required": ["repo", "title"]
  }
}`,summary:"Tools are functions the model can call. The server publishes a JSON-schema contract; the model fills the arguments; the host runs the call."},{key:"resources",label:"Resources",tagline:"read-only data, addressable by URI",who:"the host (or user) attaches them to context",listMethod:"resources/list",callMethod:"resources/read",example:`{
  "uri":       "file:///repo/README.md",
  "name":      "Project README",
  "mimeType":  "text/markdown"
}

// read:
{
  "uri": "file:///repo/README.md"
}
→ { "contents": [ { "uri": "file:///repo/README.md", "text": "# project …" } ] }`,summary:"Resources expose data that the host can attach to the model’s context. URIs like file://, git://, postgres:// let the host expose live data without inventing a new tool for every read."},{key:"prompts",label:"Prompts",tagline:"reusable prompt templates",who:"the user picks them from a menu",listMethod:"prompts/list",callMethod:"prompts/get",example:`{
  "name": "code_review",
  "description": "Review a diff for bugs and style.",
  "arguments": [
    { "name": "diff", "required": true }
  ]
}

// get:
{
  "name": "code_review",
  "arguments": { "diff": "…" }
}
→ { "messages": [ … composed prompt … ] }`,summary:"Prompts are server-authored templates the user can invoke (slash commands, menus). The server owns the phrasing; the host surfaces it as UI."}],a={tools:{bg:"#fffbeb",border:"#fde68a",text:"#92400e",accent:"#f59e0b"},resources:{bg:"#f5f3ff",border:"#ddd6fe",text:"#6d28d9",accent:"#8b5cf6"},prompts:{bg:"#ecfdf5",border:"#a7f3d0",text:"#047857",accent:"#10b981"}};function i(){let{containerRef:e,...t}=(0,r.OE)(5200*n.length,0),s=Math.floor(t.now/5200),i=n[s],l=a[i.key];return(0,o.jsx)("div",{ref:e,className:"not-prose my-8 relative",children:(0,o.jsx)("div",{className:"border border-stone-200 rounded-lg bg-white overflow-hidden shadow-sm",children:(0,o.jsxs)("div",{className:"relative",children:[(0,o.jsx)(r.p4,{...t}),(0,o.jsx)("p",{className:"px-4 pb-3 text-xs text-stone-500",children:"Example payload excerpts; JSON-RPC envelopes and some content are omitted."}),(0,o.jsx)("div",{className:"flex border-b border-stone-100",children:n.map((e,r)=>{let n=r===s,i=a[e.key];return(0,o.jsxs)("button",{type:"button","aria-pressed":s===r,onClick:()=>{t.inspect(5200*r)},className:`flex-1 px-4 py-3 text-left transition-all ${n?"bg-white":"bg-stone-50 hover:bg-stone-100"}`,style:n?{borderBottom:`2px solid ${i.accent}`}:{borderBottom:"2px solid transparent"},children:[(0,o.jsxs)("div",{className:"text-[10px] font-mono tracking-widest",style:{color:n?i.text:"#a8a29e"},children:["PRIMITIVE ",r+1]}),(0,o.jsx)("div",{className:"font-mono text-sm font-medium mt-0.5",style:{color:n?"#1a1a1a":"#78716c"},children:e.label}),(0,o.jsx)("div",{className:"text-[11px] text-stone-500 mt-0.5",children:e.tagline})]},e.key)})}),(0,o.jsx)("div",{className:"px-5 py-3 border-b border-stone-100",style:{background:l.bg},children:(0,o.jsxs)("div",{className:"flex flex-wrap items-center gap-x-6 gap-y-1 text-[11px] font-mono",children:[(0,o.jsxs)("span",{style:{color:l.text},children:[(0,o.jsx)("span",{className:"opacity-60",children:"control:"})," ",i.who]}),(0,o.jsxs)("span",{style:{color:l.text},children:[(0,o.jsx)("span",{className:"opacity-60",children:"list:"})," ",i.listMethod]}),(0,o.jsxs)("span",{style:{color:l.text},children:[(0,o.jsx)("span",{className:"opacity-60",children:"call:"})," ",i.callMethod]})]})}),(0,o.jsxs)("div",{className:"px-5 py-4 bg-[#faf8f1] border-b border-stone-100",children:[(0,o.jsx)("div",{className:"text-[10px] font-mono text-stone-500 tracking-widest mb-2",children:"EXAMPLE"}),(0,o.jsx)("pre",{tabIndex:0,role:"region","aria-label":"MCP primitive payload excerpt",className:"text-[11.5px] leading-[1.55] font-mono text-stone-800 whitespace-pre overflow-x-auto min-h-[220px]",children:i.example},s)]}),(0,o.jsx)("div",{className:"px-5 py-4 text-sm text-stone-700 leading-relaxed min-h-[76px]",children:i.summary})]})})})}},21413:(e,t,s)=>{Promise.resolve().then(s.t.bind(s,98500,23)),Promise.resolve().then(s.bind(s,41409)),Promise.resolve().then(s.bind(s,69564)),Promise.resolve().then(s.bind(s,34263)),Promise.resolve().then(s.bind(s,14155)),Promise.resolve().then(s.bind(s,13050))},41409:(e,t,s)=>{"use strict";s.d(t,{CodeWidget:()=>a});var o=s(95155),r=s(12115),n=s(47931);function a({title:e,children:t,defaultOpen:s=!1}){let i=(0,r.useId)(),[l,d]=(0,r.useState)(s);return(0,o.jsxs)("div",{className:"my-8 border border-stone-200 rounded-lg bg-white overflow-hidden shadow-sm",children:[(0,o.jsxs)("button",{type:"button","aria-expanded":l,"aria-controls":i,onClick:()=>d(!l),className:"w-full flex items-center justify-between px-4 py-3 bg-stone-50 hover:bg-stone-100 transition-colors text-left",children:[(0,o.jsx)("span",{className:"font-mono text-sm text-stone-700 font-medium tracking-tight",children:e}),(0,o.jsxs)("span",{className:"ml-3 shrink-0 flex items-center gap-2 text-stone-500 text-xs font-mono",children:[l?"collapse":"expand",(0,o.jsx)(n.c,{open:l})]})]}),l&&(0,o.jsx)("div",{id:i,role:"region","aria-label":`${e}: code`,tabIndex:0,className:"border-t border-stone-200 bg-[#f6f4ed] p-4 overflow-x-auto",children:(0,o.jsx)("div",{className:"prose-pre:my-0 prose-pre:border-none prose-pre:bg-transparent prose-pre:p-0",children:t})})]})}},69564:(e,t,s)=>{"use strict";s.r(t),s.d(t,{MCP_Handshake:()=>a});var o=s(95155),r=s(5790);let n=[{label:"initialize",dir:"→",body:`{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-11-25",
    "capabilities": {},
    "clientInfo": { "name": "demo", "version": "0.1" }
  }
}`,note:"client says hello, declares protocol version"},{label:"initialize (response)",dir:"←",body:`{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-11-25",
    "capabilities": { "tools": {}, "resources": {} },
    "serverInfo": { "name": "calc", "version": "0.1" }
  }
}`,note:"server reports what it offers"},{label:"notifications/initialized",dir:"→",body:'{ "jsonrpc": "2.0", "method": "notifications/initialized" }',note:"client confirms initialization before normal operations"},{label:"tools/list",dir:"→",body:`{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}`,note:"client asks: what tools do you expose?"},{label:"tools/list (response)",dir:"←",body:`{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      { "name": "add", "inputSchema": { "type": "object", "properties": { "a": { "type": "number" }, "b": { "type": "number" } }, "required": ["a", "b"] } },
      { "name": "multiply", "inputSchema": { "type": "object", "properties": { "a": { "type": "number" }, "b": { "type": "number" } }, "required": ["a", "b"] } }
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
}`,note:"result comes back as structured content"},{label:"model resumes",dir:"\xb7",body:'assistant: "7 times 9 is 63."',note:"the result re-enters the context and the model finishes"}];function a(){let{containerRef:e,...t}=(0,r.OE)(2400*n.length,0),s=Math.floor(t.now/2400),a=n[s],i="→"===a.dir,l="←"===a.dir,d="\xb7"===a.dir;return(0,o.jsx)("div",{ref:e,className:"not-prose my-8 relative",children:(0,o.jsx)("div",{className:"border border-stone-200 rounded-lg bg-white overflow-hidden shadow-sm",children:(0,o.jsxs)("div",{className:"relative",children:[(0,o.jsx)(r.p4,{...t}),(0,o.jsxs)("div",{className:"px-5 pr-14 pt-5 pb-4 border-b border-stone-100 bg-stone-50/50",children:[(0,o.jsxs)("div",{className:"flex items-center gap-3 text-xs font-mono text-stone-500 mb-1 tracking-wider",children:[(0,o.jsxs)("span",{children:["STEP ",s+1," / ",n.length]}),(0,o.jsx)("span",{className:"text-stone-300",children:"\xb7"}),(0,o.jsx)("span",{children:"MCP LIFECYCLE"})]}),(0,o.jsx)("div",{className:"font-mono text-sm text-stone-800 font-medium",children:a.label})]}),(0,o.jsxs)("div",{className:"grid grid-cols-[1fr_56px_1fr] items-stretch",children:[(0,o.jsxs)("div",{className:`px-5 py-4 border-r border-stone-100 ${i||d?"bg-blue-50/40":"bg-white"}`,children:[(0,o.jsx)("div",{className:"text-[10px] font-mono text-blue-700 tracking-widest mb-1",children:"CLIENT (HOST)"}),(0,o.jsx)("div",{className:"text-xs text-stone-500",children:"Claude Code, Cursor, your app…"})]}),(0,o.jsx)("div",{className:"flex items-center justify-center bg-stone-50/40",children:(0,o.jsx)("div",{className:`text-2xl font-mono transition-all duration-500 ${i?"text-blue-600 translate-x-1":l?"text-violet-600 -translate-x-1":"text-stone-400"}`,style:{transitionTimingFunction:"cubic-bezier(0.4, 0, 0.2, 1)"},children:a.dir})}),(0,o.jsxs)("div",{className:`px-5 py-4 border-l border-stone-100 ${l?"bg-violet-50/40":"bg-white"}`,children:[(0,o.jsx)("div",{className:"text-[10px] font-mono text-violet-700 tracking-widest mb-1",children:"SERVER"}),(0,o.jsx)("div",{className:"text-xs text-stone-500",children:"Filesystem, GitHub, calc…"})]})]}),(0,o.jsx)("div",{className:"px-5 pt-4 pb-3 border-t border-stone-100 bg-[#faf8f1]",children:(0,o.jsx)("pre",{className:"text-[11.5px] leading-[1.55] font-mono text-stone-800 whitespace-pre-wrap [overflow-wrap:anywhere] min-h-[240px]",style:{transition:"opacity 900ms ease-in-out",opacity:1},children:a.body},s)}),(0,o.jsx)("div",{className:"px-5 py-3 border-t border-stone-100 bg-stone-50/40 text-xs text-stone-600 italic min-h-[38px]",children:a.note}),(0,o.jsx)("div",{className:"px-5 py-3 border-t border-stone-100 flex items-center gap-1.5",children:n.map((e,t)=>(0,o.jsx)("div",{className:`h-1 rounded-full transition-all duration-500 ${t===s?"w-8 bg-stone-700":t<s?"w-3 bg-stone-400":"w-3 bg-stone-200"}`},t))})]})})})}}},e=>{e.O(0,[5846,8500,3050,4263,8441,3794,7358],()=>e(e.s=21413)),_N_E=e.O()}]);