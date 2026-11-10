"use strict";(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[9363],{14503:(e,t,a)=>{a.d(t,{Q:()=>n});var r=a(95155);function n({text:e="the",className:t=""}){return(0,r.jsxs)("span",{className:`cfb-the-slam ${t}`.trim(),children:[(0,r.jsx)("span",{className:"cfb-the-slam-word",children:e}),(0,r.jsx)("style",{children:`
        .cfb-the-slam {
          position: relative;
          display: inline-block;
          margin-right: 0.08em;
          font-style: italic;
          font-weight: 700;
          vertical-align: baseline;
        }
        .cfb-the-slam-word {
          display: inline-block;
          transform-origin: 50% 85%;
          will-change: transform;
          animation: cfb-the-slam-pulse 6s cubic-bezier(0.22, 1, 0.36, 1) infinite;
        }
        .cfb-the-slam::after {
          content: "";
          position: absolute;
          left: -4%;
          right: -4%;
          bottom: 0.04em;
          height: 2px;
          border-radius: 999px;
          background: #991b1b;
          opacity: 0;
          transform: scaleX(0.4);
          transform-origin: center;
          pointer-events: none;
          animation: cfb-the-slam-wave 6s ease-out infinite;
        }
        @keyframes cfb-the-slam-pulse {
          0%, 100% {
            transform: translateY(0) scale(1);
            color: inherit;
            text-shadow: none;
          }
          3% {
            transform: translateY(-0.16em) scale(1.26);
            color: #991b1b;
            text-shadow: 0 4px 10px rgba(153, 27, 27, 0.22);
          }
          5.5% {
            transform: translateY(0.04em) scale(1.24, 0.88);
            color: #991b1b;
            text-shadow: 0 1px 2px rgba(153, 27, 27, 0.35);
          }
          8% {
            transform: translateY(-0.05em) scale(1.18);
            color: #991b1b;
            text-shadow: 0 2px 6px rgba(153, 27, 27, 0.2);
          }
          10.5% {
            transform: translateY(0) scale(1.05);
            color: #991b1b;
          }
          13% {
            transform: translateY(-0.03em) scale(1.13);
            color: #991b1b;
            text-shadow: 0 2px 5px rgba(153, 27, 27, 0.16);
          }
          15.5% {
            transform: translateY(0) scale(1.03);
            color: #7f1d1d;
          }
          18% {
            transform: translateY(-0.015em) scale(1.07);
            color: inherit;
            text-shadow: none;
          }
          21% {
            transform: translateY(0) scale(1);
            color: inherit;
            text-shadow: none;
          }
        }
        @keyframes cfb-the-slam-wave {
          0%, 4.5%, 22%, 100% {
            opacity: 0;
            transform: scaleX(0.35);
          }
          5.5% {
            opacity: 0.85;
            transform: scaleX(1.08);
          }
          13% {
            opacity: 0.45;
            transform: scaleX(1.18);
          }
          20% {
            opacity: 0;
            transform: scaleX(1.28);
          }
        }
        @media (prefers-reduced-motion: reduce) {
          .cfb-the-slam-word,
          .cfb-the-slam::after {
            animation: none !important;
          }
        }
      `})]})}},20524:(e,t,a)=>{a.d(t,{$:()=>d});var r=a(95155),n=a(12115),s=a(98500),o=a.n(s),i=a(73321),l=a(45265),c=a(67466);let p={SEC:"SEC","Big Ten":"B1G","Big 12":"B12",ACC:"ACC","Pac-12":"P12","American Athletic":"AAC","Mountain West":"MWC","Sun Belt":"SBC","Mid-American":"MAC","Conference USA":"CUSA","FBS Independents":"Ind"},m=[{id:"fbs",label:"FBS"},{id:"SEC",label:"SEC"},{id:"Big Ten",label:"Big Ten"},{id:"Big 12",label:"Big 12"},{id:"ACC",label:"ACC"},{id:"Pac-12",label:"Pac-12"},{id:"g5",label:"G5"},{id:"fcs",label:"FCS"},{id:"all",label:"All"}],f=new Set(["American Athletic","Mountain West","Sun Belt","Mid-American","Conference USA"]);function d({rankings:e}){let t=(0,i.useRouter)(),[a,s]=(0,n.useState)("fbs"),[h,u]=(0,n.useState)(""),[_,x]=(0,n.useState)("power_rating"),[b,w]=(0,n.useState)("desc"),g=(0,n.useMemo)(()=>{let t=h.trim().toLowerCase(),r=e.filter(e=>{if("fbs"===a){if("fbs"!==e.division)return!1}else if("fcs"===a){if("fcs"!==e.division)return!1}else if("g5"===a){if(!f.has(e.conference))return!1}else if("all"!==a&&e.conference!==a)return!1;return(!(t.length>0)||!!`${e.team_name} ${e.abbreviation} ${e.conference}`.toLowerCase().includes(t))&&!0});return r.sort((e,t)=>{let a=Number(e[_]??("desc"===b?-9999:9999)),r=Number(t[_]??("desc"===b?-9999:9999));return a===r?e.rank-t.rank:"desc"===b?r-a:a-r}),r},[e,a,h,_,b]),y=(e,t,a,n="py-2.5 px-1.5 sm:px-2 text-right font-normal whitespace-nowrap")=>{let s=_===t;return(0,r.jsx)("th",{className:n,title:a,children:(0,r.jsx)("button",{type:"button",onClick:()=>{_===t?w(e=>"desc"===e?"asc":"desc"):(x(t),w("rank"===t?"asc":"desc"))},className:`cursor-pointer uppercase tracking-wider whitespace-nowrap ${s?"font-semibold text-stone-900 underline underline-offset-3":"hover:text-stone-700"}`,children:e})})};return(0,r.jsxs)("div",{className:"not-prose cfb-leaderboard-wrap my-6",children:[(0,r.jsxs)("div",{className:"mb-3 flex flex-col sm:flex-row sm:flex-wrap sm:items-center sm:justify-between gap-2.5 sm:gap-3 font-mono text-[13px]",children:[(0,r.jsx)("div",{className:"flex flex-wrap items-center gap-x-3.5 gap-y-1",role:"group","aria-label":"Filter conference or division",children:m.map(e=>{let t=a===e.id;return(0,r.jsx)("button",{type:"button",onClick:()=>s(e.id),className:`cursor-pointer py-1 transition-colors ${t?"font-semibold text-stone-900 underline underline-offset-3":"text-stone-400 hover:text-stone-700"}`,children:e.label},e.id)})}),(0,r.jsx)("input",{type:"search",value:h,onChange:e=>u(e.target.value),placeholder:"Filter teams...","aria-label":"Filter teams",className:"w-full sm:w-52 border-b border-stone-300 bg-transparent px-1 py-1 font-mono text-[13px] text-stone-800 placeholder:text-stone-400 focus:border-stone-800 focus:outline-none"})]}),(0,r.jsx)("div",{className:"border-y border-stone-200 overflow-x-auto",role:"region","aria-label":"College Football Power Index Leaderboard",tabIndex:0,children:(0,r.jsxs)("table",{className:"w-full min-w-[700px] sm:min-w-[820px] max-w-none border-collapse text-left font-mono text-[13px] whitespace-nowrap",style:{width:"100%",maxWidth:"none",display:"table"},children:[(0,r.jsx)("thead",{children:(0,r.jsxs)("tr",{className:"border-b border-stone-200 text-[11px] uppercase tracking-wider text-stone-400 whitespace-nowrap",children:[y("#","rank","Overall Rank","w-6 sm:w-8 py-2.5 pr-1 sm:pr-2 font-normal text-left whitespace-nowrap"),(0,r.jsx)("th",{className:"py-2.5 pr-2 sm:pr-3 font-normal sm:w-[215px] whitespace-nowrap",children:"Team"}),y("Rec","wins","Current Win-Loss Record","py-2.5 px-1.5 sm:px-2 text-left font-normal whitespace-nowrap"),y("Power","power_rating","Spread points above an average FBS team on a neutral field"),y("Off","off_rating","Offensive points above FBS average"),y("Def","def_rating","Defensive points prevented above FBS average"),y("ST","st_rating","Special teams net points"),y("Prior","prior_rating","Preseason roster & coaching prior"),y("Lift","inseason_lift","In-season performance lift vs preseason prior"),y("SOS","sos_full","Full-season strength of schedule"),y("Proj","proj_wins","Projected regular-season record"),y("Playoff","cfp_playoff_prob","12-team CFP probability"),y("Title","natl_champ_prob","National championship probability","py-2.5 pl-1.5 sm:pl-2 text-right font-normal whitespace-nowrap")]})}),(0,r.jsx)("tbody",{className:"divide-y divide-stone-200/70",children:g.map(e=>{let a="fbs"===e.division,n=p[e.conference]??("fcs"===e.division?"FCS":e.conference);return(0,r.jsxs)("tr",{onClick:r=>{!a||r.target.closest("a, button")||t.push(`/cfb/teams/${e.slug}`)},className:`group transition-colors hover:bg-stone-100/70 whitespace-nowrap ${a?"cursor-pointer":""}`,children:[(0,r.jsx)("td",{className:"py-2.5 pr-1.5 sm:pr-2 text-xs tabular-nums text-stone-400 whitespace-nowrap",children:e.rank}),(0,r.jsx)("td",{className:"py-2.5 pr-2 sm:pr-3 whitespace-nowrap",children:(0,r.jsxs)("div",{className:"inline-flex items-center gap-1.5 sm:gap-2 whitespace-nowrap",children:[(0,r.jsx)(l.i,{espnId:e.espn_id,logoPath:e.logo_path,teamId:e.team_id,teamName:e.team_name,abbreviation:e.abbreviation,color:e.color,altColor:e.alt_color,size:18}),a?(0,r.jsx)(o(),{href:`/cfb/teams/${e.slug}`,className:"font-sans text-[13.5px] font-medium text-stone-900 underline decoration-stone-300 underline-offset-3 transition-colors group-hover:text-black group-hover:decoration-stone-600 whitespace-nowrap",children:e.team_name}):(0,r.jsx)("span",{className:"font-sans text-[13.5px] text-stone-700 whitespace-nowrap",children:e.team_name}),(0,r.jsx)("span",{className:"text-[11px] text-stone-400 whitespace-nowrap",children:n})]})}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-left tabular-nums text-stone-600 whitespace-nowrap",children:e.record.replace("-","‑")}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right font-semibold tabular-nums text-stone-900 whitespace-nowrap",children:(0,c.Gp)(e.power_rating,1)}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-700 whitespace-nowrap",children:(0,c.Gp)(e.off_rating,1)}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-700 whitespace-nowrap",children:(0,c.Gp)(e.def_rating,1)}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-500 whitespace-nowrap",children:(0,c.Gp)(e.st_rating,1)}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-500 whitespace-nowrap",children:(0,c.Gp)(e.prior_rating,1)}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-600 whitespace-nowrap",children:(0,c.Gp)(e.inseason_lift,1)}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-600 whitespace-nowrap",children:null!==e.sos_full?(0,c.Gp)(e.sos_full,1):"—"}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-600 whitespace-nowrap",children:null!==e.proj_wins&&null!==e.proj_losses?`${e.proj_wins.toFixed(1)}\u2011${e.proj_losses.toFixed(1)}`:"—"}),(0,r.jsx)("td",{className:"py-2.5 px-1.5 sm:px-2 text-right tabular-nums text-stone-700 whitespace-nowrap",children:null!==e.cfp_playoff_prob&&e.cfp_playoff_prob>=.001?`${(100*e.cfp_playoff_prob).toFixed(1)}%`:"—"}),(0,r.jsx)("td",{className:"py-2.5 pl-1.5 sm:pl-2 text-right tabular-nums text-stone-500 whitespace-nowrap",children:null!==e.natl_champ_prob&&e.natl_champ_prob>=.001?`${(100*e.natl_champ_prob).toFixed(1)}%`:"—"})]},e.team_id)})})]})})]})}},45265:(e,t,a)=>{a.d(t,{i:()=>o});var r=a(95155),n=a(12115),s=a(67466);function o({espnId:e,logoPath:t,teamId:a,teamName:i,abbreviation:l,color:c,altColor:p,size:m=18,className:f=""}){let[d,h]=(0,n.useState)(!1),u=e??(a&&a.startsWith("cfbd:")?Number(a.slice(5)):void 0),_=t||(u&&!Number.isNaN(u)?`/images/cfb-logos/${u}.png`:null),x=(0,s.d3)(c,p),b=(l||i.replace(/[^a-zA-Z0-9 ]/g,"").split(/\s+/).map(e=>e[0]).join("").slice(0,3)).slice(0,4).toUpperCase(),w=m<=18?8:m<=24?9:m<=32?10.5:12;return(0,r.jsx)("span",{className:`cfb-team-logo inline-flex shrink-0 items-center justify-center select-none ${f}`,style:{width:`${m}px`,height:`${m}px`},"aria-hidden":"true",children:_&&!d?(0,r.jsx)("img",{src:_,alt:"",width:m,height:m,decoding:"async",onError:()=>h(!0),className:"object-contain",style:{width:`${m}px`,height:`${m}px`}}):(0,r.jsx)("span",{className:"flex h-full w-full items-center justify-center rounded-xs font-mono font-semibold tracking-tight",style:{backgroundColor:x.softBg,color:x.primary,fontSize:`${w}px`},children:b})})}},67466:(e,t,a)=>{a.d(t,{Gp:()=>h,OU:()=>i,R8:()=>u,d3:()=>m,qG:()=>r,ry:()=>o,uw:()=>d});let r=9.5;function n(e,t,a){return Math.min(a,Math.max(t,e))}function s(e){return Math.round(100*e)/100}function o(e){if(e>=0)return 1/(1+Math.exp(-e));let t=Math.exp(e);return t/(1+t)}function i(e,t,a="regular_home",l=null,c=null){let p="homeTeam"in e,m=p?e.homeTeam:e,f=p?e.awayTeam:t,d=p?e.venue??"regular_home":a,h=p?e.homeQbVorOverride??null:l,u=p?e.awayQbVorOverride??null:c,_=function(e="regular_home"){return"neutral"===e?0:"cfp_first_round"===e||"cfp_first_round_home"===e?2.5:2.3}(d),x=null!=h?h-m.active_qb_vor:0,b=null!=u?u-f.active_qb_vor:0,w=m.off_pass_rating+x,g=m.off_rating+x,y=m.power_rating+x,v=f.off_pass_rating+b,j=f.off_rating+b,N=f.power_rating+b,M=y-N+_,C=1/(1+Math.exp(-M/r)),$=n(27.2+g-f.def_rating+.5*(m.st_rating-f.st_rating+_),3,70),S=n(27.2+j-m.def_rating-.5*(m.st_rating-f.st_rating+_),3,70),k=M>=0?`${m.team_name} -${Math.abs(M).toFixed(1)}`:`${f.team_name} -${Math.abs(M).toFixed(1)}`,F={home_pass_vs_away_pass_def:s(w-f.def_pass_rating),home_rush_vs_away_rush_def:s(m.off_rush_rating-f.def_rush_rating),away_pass_vs_home_pass_def:s(v-m.def_pass_rating),away_rush_vs_home_rush_def:s(f.off_rush_rating-m.def_rush_rating),finish_edge_home:s(m.off_finish_rating-f.off_finish_rating),havoc_edge_home:s(m.def_havoc_rating-f.def_havoc_rating),st_edge_home:s(m.st_rating-f.st_rating)},A=Math.round(10*Math.min(100,Math.max(0,45*o((.5*(y+N)*.7+.3*Math.max(y,N)-2)/8.5)+30*Math.exp(-Math.pow(Math.abs(M)/14.5,2))+25*Math.min(1,.55*((m.cfp_playoff_prob??0)+(f.cfp_playoff_prob??0))+.35*((m.conf_champ_prob??0)+(f.conf_champ_prob??0))))))/10,B=Math.round(100*Math.sqrt(169+Math.pow(.55*(m.uncertainty_sd??2.1),2)+Math.pow(.55*(f.uncertainty_sd??2.1),2)))/100;return{home_team_id:m.team_id,home_team_name:m.team_name,away_team_id:f.team_id,away_team_name:f.team_name,venue:d,hfa_points:_,home_qb_delta:s(x),away_qb_delta:s(b),home_effective_power:s(y),away_effective_power:s(N),home_effective_off:s(g),away_effective_off:s(j),home_effective_off_pass:s(w),away_effective_off_pass:s(v),projected_home_margin:M,formatted_spread:k,home_win_prob:C,away_win_prob:1-C,projected_home_points:$,projected_away_points:S,projected_total:$+S,unit_edges:F,watchability_score:A,game_margin_sd:B}}function l(e){let t=(e||"#3b5998").replace(/^#/,"").trim();return/^[0-9a-fA-F]{6}$/.test(t)?[parseInt(t.slice(0,2),16),parseInt(t.slice(2,4),16),parseInt(t.slice(4,6),16)]:[59,89,152]}function c(e,t,a){let r=e/255,n=t/255,s=a/255,o=Math.max(r,n,s),i=Math.min(r,n,s),l=0,c=0,p=(o+i)/2;if(o!==i){let e=o-i;switch(c=p>.5?e/(2-o-i):e/(o+i),o){case r:l=(n-s)/e+6*(n<s);break;case n:l=(s-r)/e+2;break;case s:l=(r-n)/e+4}l/=6}return[Math.round(360*l),Math.round(100*c),Math.round(100*p)]}function p(e,t,a){let r=(e%360+360)%360,s=n(t,0,100)/100,o=n(a,0,100)/100,i=(1-Math.abs(2*o-1))*s,l=i*(1-Math.abs(r/60%2-1)),c=o-i/2,p=0,m=0,f=0;r<60?[p,m,f]=[i,l,0]:r<120?[p,m,f]=[l,i,0]:r<180?[p,m,f]=[0,i,l]:r<240?[p,m,f]=[0,l,i]:r<300?[p,m,f]=[l,0,i]:[p,m,f]=[i,0,l];let d=e=>Math.round((e+c)*255).toString(16).padStart(2,"0");return`#${d(p)}${d(m)}${d(f)}`}function m(e,t){let[a,r,s]=l(e||"#3b5998"),[o,i,m]=c(a,r,s),f=i<10?i:n(i,28,66),d=p(o,f,n(m,24,38)),[h,u,_]=l(t||"#57534e"),[x,b,w]=c(h,u,_);return{primary:d,secondary:p(x,b<10?8:n(b,20,55),n(w,28,44)),softBg:p(o,Math.min(f,42),93),border:p(o,Math.min(f,45),76),textOnPrimary:"#faf8f1",hue:o}}function f(e,t){let a=Math.abs(e-t)%360;return a>180?360-a:a}function d(e,t){let a=m(e.color,e.alt_color),r=m(t.color,t.alt_color);if(f(a.hue,r.hue)>=32)return{away:a,home:r,collisionResolved:!1};let[n,s,o]=l(e.alt_color||"#334155"),[i,p]=c(n,s,o),d=f(i,r.hue);return p>=18&&d>=36?{away:m(e.alt_color,e.color),home:r,collisionResolved:!0}:{away:m(f(215,r.hue)>=45?"#2b4c7e":"#8c5a2b",e.color),home:r,collisionResolved:!0}}function h(e,t=2){return null==e||Number.isNaN(e)?"—":`${e>0?"+":e<0?"−":"+"}${Math.abs(e).toFixed(t)}`}function u(e,t=1){if(null==e||Number.isNaN(e))return"—";let a=100*e;return a>0&&a<.1?"<0.1%":a>99.9&&a<100?">99.9%":`${a.toFixed(t)}%`}}}]);