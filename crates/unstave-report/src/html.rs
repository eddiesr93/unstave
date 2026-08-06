//! Self-contained interactive HTML renderer.

use serde_json::{json, Value};

use crate::visualization::{project, GraphInput};
use crate::AnalysisReport;

const CYTOSCAPE: &str = include_str!("../assets/vendor/cytoscape-3.31.2/cytoscape.min.js");

/// Node budget matching the other graph renderers.
pub const DEFAULT_MAX_NODES: usize = 150;

/// Render one portable HTML file with the report and graph library embedded.
pub fn render(report: &AnalysisReport, max_nodes: usize) -> serde_json::Result<String> {
    let value = serde_json::to_value(report)?;
    render_value(&value, max_nodes)
}

/// Render a report that already crossed a JSON-compatible API boundary.
pub fn render_value(report: &Value, max_nodes: usize) -> serde_json::Result<String> {
    let json = serde_json::to_string(&view_model(report, max_nodes)?)?;
    // JSON inside a script element must not be able to spell a closing script tag.
    let safe_json = json
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");

    let mut out = String::with_capacity(CYTOSCAPE.len() + safe_json.len() + 24_000);
    out.push_str(HTML_HEAD);
    out.push_str("<script>");
    out.push_str(CYTOSCAPE);
    out.push_str("</script></head>");
    out.push_str(HTML_BODY);
    out.push_str("<script id=\"unstave-data\" type=\"application/json\">");
    out.push_str(&safe_json);
    out.push_str("</script><script>");
    out.push_str(APP_SCRIPT);
    out.push_str("</script></body></html>\n");
    Ok(out)
}

/// Everything the page draws, and nothing else.
///
/// The full report carries every module, edge, dead export and cycle path. Embedding
/// that made a 4 MB page for a 5,000-module workspace and handed the layout engine
/// ~20,000 elements, which pins the main thread. The page only draws the projected
/// graph, the summary counters and the barrel table, so only those are serialized.
fn view_model(report: &Value, max_nodes: usize) -> serde_json::Result<Value> {
    let projected = project(&GraphInput::from_value(report)?, max_nodes);

    let nodes: Vec<Value> = projected
        .nodes
        .iter()
        .map(|node| {
            json!({
                "id": node.id,
                "label": node.label,
                "directory": node.directory,
                "moduleCount": node.module_count,
                "isBarrel": node.is_barrel,
                "inCycle": node.in_cycle,
                "members": node.members,
            })
        })
        .collect();
    let edges: Vec<Value> = projected
        .edges
        .iter()
        .map(|edge| json!({ "source": edge.source, "target": edge.target, "kind": edge.kind }))
        .collect();

    Ok(json!({
        "schemaVersion": report.get("schemaVersion").cloned().unwrap_or(Value::Null),
        "summary": report.get("summary").cloned().unwrap_or(Value::Null),
        "amplification": {
            "barrels": report
                .pointer("/amplification/barrels")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        },
        "graph": {
            "nodes": nodes,
            "edges": edges,
            "collapsed": projected.collapsed,
            "maxNodes": max_nodes,
        },
    }))
}

const HTML_HEAD: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>unstave analysis</title>
<style>
:root{color-scheme:dark;--space-1:4px;--space-2:8px;--space-3:12px;--space-4:16px;--space-6:24px;--space-8:32px;--bg:oklch(13% .012 125);--panel:oklch(17% .014 125);--panel-2:oklch(21% .018 125);--line:oklch(31% .025 125);--text:oklch(93% .018 115);--muted:oklch(70% .025 125);--acid:oklch(88% .21 126);--amber:oklch(79% .16 73);--red:oklch(68% .23 24)}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.52 "Avenir Next",Avenir,"Century Gothic",sans-serif;overflow:hidden}button,select,input{font:inherit}button,select{color:var(--text);background:var(--panel-2);border:1px solid var(--line);border-radius:2px;padding:var(--space-2) var(--space-3)}button{cursor:pointer;text-transform:uppercase;letter-spacing:.09em;font-size:10px;font-weight:700}button:hover{border-color:var(--acid);color:var(--acid)}button:focus-visible,select:focus-visible,input:focus-visible{outline:2px solid var(--acid);outline-offset:2px}
.app{display:grid;grid-template-columns:344px minmax(0,1fr);height:100vh}.sidebar{position:relative;padding:var(--space-6);border-right:1px solid var(--line);background:var(--panel);overflow:auto}.sidebar::before{content:"";display:block;width:76px;height:5px;background:var(--acid);margin-bottom:var(--space-6)}.brand{display:block}.mark{display:none}.brand h1{font-family:"DIN Alternate","Arial Narrow",sans-serif;font-size:31px;line-height:1;letter-spacing:-.06em;margin:0;text-transform:uppercase}.sub{color:var(--muted);font-size:10px;font-weight:700;letter-spacing:.16em;text-transform:uppercase;margin:var(--space-2) 0 var(--space-6)}.stats{display:grid;grid-template-columns:1fr 1fr;border-top:1px solid var(--line);border-bottom:1px solid var(--line);margin-bottom:var(--space-8)}.stat{padding:var(--space-3) 0}.stat:nth-child(odd){border-right:1px solid var(--line)}.stat:nth-child(even){padding-left:var(--space-3)}.stat:nth-child(n+3){border-top:1px solid var(--line)}.stat b{display:block;font-family:"DIN Alternate","Arial Narrow",sans-serif;font-size:24px;line-height:1.1}.stat span{color:var(--muted);font-size:9px;text-transform:uppercase;letter-spacing:.14em}
.section{border-top:1px solid var(--line);padding-top:var(--space-4);margin-top:var(--space-6)}.section h2{font-size:9px;text-transform:uppercase;letter-spacing:.18em;color:var(--acid);margin:0 0 var(--space-3)}.field{display:grid;gap:var(--space-1);margin:var(--space-3) 0}.field label{color:var(--muted);font-size:11px}.field select{width:100%}.checks{display:grid;grid-template-columns:1fr 1fr;gap:var(--space-2)}.check{display:flex;align-items:center;gap:var(--space-2);color:var(--muted);font-size:11px}.check input{accent-color:var(--acid)}.legend{display:grid;gap:var(--space-2);color:var(--muted);font-size:11px}.swatch{display:inline-block;width:17px;height:3px;margin-right:var(--space-2);vertical-align:middle;background:var(--muted)}.swatch.barrel{background:var(--amber)}.swatch.cycle{background:var(--red)}.details{font-size:11px}.details h3{margin:0 0 var(--space-2);font:700 13px/1.35 "DIN Alternate","Arial Narrow",sans-serif;overflow-wrap:anywhere}.details p{color:var(--muted);margin:var(--space-1) 0}.details ul{padding-left:18px;margin:var(--space-2) 0;max-height:135px;overflow:auto}.details li{overflow-wrap:anywhere;margin:2px 0}
.main{min-width:0;display:grid;grid-template-rows:minmax(320px,62vh) minmax(220px,38vh)}.graph-wrap{position:relative;border-bottom:1px solid var(--line);background:var(--bg)}#cy{position:absolute;inset:0}.graph-title{position:absolute;z-index:2;left:var(--space-6);top:var(--space-4);border-top:3px solid var(--acid);padding-top:var(--space-2);color:var(--muted);pointer-events:none;text-transform:uppercase;font-size:9px;letter-spacing:.14em}.graph-title b{display:block;color:var(--text);font:700 17px/1.1 "DIN Alternate","Arial Narrow",sans-serif;letter-spacing:-.01em}.graph-title span{letter-spacing:.06em;text-transform:none}
.table-wrap{overflow:auto;padding:var(--space-4) var(--space-6) var(--space-8);background:var(--panel)}.table-head{display:flex;justify-content:space-between;align-items:baseline;gap:var(--space-6);margin-bottom:var(--space-3)}.table-head h2{margin:0;font:700 18px/1.2 "DIN Alternate","Arial Narrow",sans-serif;text-transform:uppercase;letter-spacing:.02em}.table-head span{color:var(--muted);font-size:10px;text-transform:uppercase;letter-spacing:.1em}table{border-collapse:collapse;width:100%;font-size:11px}th{text-align:left;position:sticky;top:-16px;background:var(--panel);padding:0;border-top:1px solid var(--line);border-bottom:1px solid var(--line)}.sort-button{width:100%;padding:var(--space-2);border:0;background:transparent;color:var(--muted);text-align:left;font-size:9px}.sort-button:hover,.sort-button:focus-visible{color:var(--acid)}td{padding:var(--space-2);border-bottom:1px solid color-mix(in oklch,var(--line),transparent 45%)}td:first-child{font-family:"SFMono-Regular",Consolas,monospace}tbody tr:hover{background:color-mix(in oklch,var(--acid),transparent 94%)}.empty{color:var(--muted);padding:var(--space-6) 0}.warn{color:var(--amber)}
@media(max-width:800px){body{overflow:auto}.app{display:block;height:auto}.sidebar{border-right:0;border-bottom:1px solid var(--line)}.main{height:900px;grid-template-rows:540px 360px}}
</style>
"##;

const HTML_BODY: &str = r##"<body>
<div class="app">
  <aside class="sidebar" aria-label="Analysis controls">
    <div class="brand"><span class="mark"></span><h1>UN/STAVE</h1></div>
    <p class="sub">module graph / disassembled</p>
    <div class="stats" id="stats"></div>
    <section class="section">
      <h2>Graph filters</h2>
      <div class="field"><label for="directory">Directory</label><select id="directory"><option value="">All directories</option></select></div>
      <div class="checks" id="edge-kinds"></div>
      <label class="check" style="margin-top:10px"><input id="cycles" type="checkbox" checked> Highlight cycles</label>
      <button id="fit" type="button" style="margin-top:12px">Fit graph</button>
    </section>
    <section class="section"><h2>Legend</h2><div class="legend"><span><i class="swatch"></i>module</span><span><i class="swatch barrel"></i>barrel</span><span><i class="swatch cycle"></i>cycle member</span></div></section>
    <section class="section"><h2>Selection</h2><div class="details" id="details"><p>Click a node to inspect its importers and importees.</p></div></section>
  </aside>
  <main class="main">
    <section class="graph-wrap" aria-label="Dependency graph"><div class="graph-title"><b>Topology // live</b><span id="visible-count"></span><span id="graph-status"></span></div><div id="cy"></div></section>
    <section class="table-wrap"><div class="table-head"><h2>Barrel amplification</h2><span>Click a heading to sort</span></div><div id="barrels"></div></section>
  </main>
</div>
"##;

const APP_SCRIPT: &str = r##"
(() => {
  'use strict';
  const report = JSON.parse(document.getElementById('unstave-data').textContent);
  const graph = report.graph;
  const modules = new Map(graph.nodes.map(n => [n.id, n]));
  const kinds = [...new Set(graph.edges.map(e => e.kind))].sort();
  const labels = {static:'Static',dynamic:'Dynamic',typeOnly:'Type only',reExport:'Re-export',sideEffectOnly:'Side effect'};

  const stats = [
    [report.summary.modules, 'modules'], [report.summary.edges, 'edges'],
    [report.summary.classifiedBarrels, 'barrels'], [report.summary.cycles, 'cycles']
  ];
  const statsEl = document.getElementById('stats');
  stats.forEach(([value,label]) => { const el=document.createElement('div'); el.className='stat'; const b=document.createElement('b'); b.textContent=value; const s=document.createElement('span'); s.textContent=label; el.append(b,s); statsEl.append(el); });

  const directory = document.getElementById('directory');
  [...new Set(graph.nodes.map(n => n.directory))].sort().forEach(dir => { const option=document.createElement('option'); option.value=dir; option.textContent=dir; directory.append(option); });
  const edgeKinds = document.getElementById('edge-kinds');
  kinds.forEach(kind => { const label=document.createElement('label'); label.className='check'; const input=document.createElement('input'); input.type='checkbox'; input.value=kind; input.checked=true; label.append(input, document.createTextNode(labels[kind] || kind)); edgeKinds.append(label); });

  const elements = [
    ...graph.nodes.map(n => ({data:{id:n.id,label:n.label,directory:n.directory,moduleCount:n.moduleCount,inCycle:n.inCycle},classes:[n.isBarrel?'barrel':'',n.inCycle?'cycle':'',n.moduleCount>1?'group':''].filter(Boolean).join(' ')})),
    ...graph.edges.map((e,i) => ({data:{id:`e${i}`,source:e.source,target:e.target,kind:e.kind,label:labels[e.kind] || e.kind},classes:e.kind}))
  ];
  const cy = cytoscape({
    container: document.getElementById('cy'), elements,
    style:[
      {selector:'node',style:{'background-color':'#34412f','border-color':'#71806c','border-width':1,'label':'data(label)','color':'#d9e4d4','font-size':9,'text-wrap':'ellipsis','text-max-width':150,'text-valign':'bottom','text-margin-y':8,'width':18,'height':18}},
      {selector:'node.barrel',style:{'shape':'diamond','background-color':'#5c4814','border-color':'#ffb000','border-width':2,'width':25,'height':25}},
      {selector:'node.cycle',style:{'border-color':'#ff5c67','border-width':4}},
      {selector:'node:selected',style:{'background-color':'#b7f34a','border-color':'#edfbd6','border-width':3}},
      {selector:'edge',style:{'width':1,'line-color':'#687762','target-arrow-color':'#687762','target-arrow-shape':'triangle','curve-style':'bezier','arrow-scale':.7,'opacity':.72}},
      {selector:'edge.dynamic',style:{'line-style':'dashed','line-color':'#b7f34a','target-arrow-color':'#b7f34a'}},
      {selector:'edge.typeOnly',style:{'line-style':'dotted','line-color':'#687762','target-arrow-color':'#687762'}},
      {selector:'edge.reExport',style:{'width':2,'line-color':'#ffb000','target-arrow-color':'#ffb000'}},
      {selector:'edge.sideEffectOnly',style:{'width':2,'line-color':'#ff5c67','target-arrow-color':'#ff5c67'}},
      {selector:'node.group',style:{'shape':'round-rectangle','width':46,'height':26,'font-size':8}},
      {selector:'.hidden',style:{'display':'none'}}
    ],
    // A layout run in the constructor blocks first paint. Start from a cheap preset so
    // the sidebar and table are usable immediately, then lay out on the next frame.
    layout:{name:'grid',fit:true}
  });

  const visibleCount = document.getElementById('visible-count');
  const status = document.getElementById('graph-status');
  function runLayout(){
    const count = cy.nodes().length;
    // cose is force-directed and superlinear; past a few hundred nodes it stops being
    // worth the wait, and a deterministic layered layout reads better anyway.
    const layout = count <= 260
      ? {name:'cose',animate:false,nodeRepulsion:7000,idealEdgeLength:65,edgeElasticity:80,gravity:.4,numIter:400,randomize:true}
      : {name:'breadthfirst',animate:false,directed:true,spacingFactor:1.1};
    status.textContent='laying out…';
    requestAnimationFrame(() => {
      cy.layout(layout).run();
      cy.fit(cy.elements(':visible'),38);
      status.textContent = graph.collapsed
        ? `directories collapsed at --max-nodes ${graph.maxNodes}`
        : '';
    });
  }
  runLayout();
  function applyFilters(){
    const dir=directory.value;
    const enabled=new Set([...edgeKinds.querySelectorAll('input:checked')].map(i=>i.value));
    cy.batch(() => {
      cy.nodes().forEach(node => { const item=modules.get(node.id()); node.toggleClass('hidden', !!dir && item.directory!==dir && !item.members.some(path => path.startsWith(`${dir}/`))); });
      cy.edges().forEach(edge => edge.toggleClass('hidden', !enabled.has(edge.data('kind'))));
    });
    const shown=cy.nodes(':visible');
    const moduleCount=shown.reduce((sum,node)=>sum+(node.data('moduleCount')||1),0);
    visibleCount.textContent=`${moduleCount} modules in ${shown.length} nodes · ${cy.edges(':visible').length} edges`;
  }
  directory.addEventListener('change',applyFilters);
  edgeKinds.addEventListener('change',applyFilters);
  document.getElementById('cycles').addEventListener('change',event => { graph.nodes.filter(n=>n.inCycle).forEach(n=>cy.getElementById(n.id).toggleClass('cycle',event.target.checked)); });
  document.getElementById('fit').addEventListener('click',()=>cy.fit(cy.elements(':visible'),38));
  applyFilters();

  const details=document.getElementById('details');
  const addList=(title,items) => { const p=document.createElement('p'); p.textContent=`${title} (${items.length})`; details.append(p); const ul=document.createElement('ul'); [...items].sort((a,b)=>a.localeCompare(b)).forEach(text=>{const li=document.createElement('li');li.textContent=text;ul.append(li)}); details.append(ul); };
  cy.on('tap','node',event => {
    const node=event.target, item=modules.get(node.id());
    details.replaceChildren();
    const h=document.createElement('h3'); h.textContent=item.moduleCount>1?item.label:item.members[0]; details.append(h);
    const meta=document.createElement('p'); meta.textContent=[item.moduleCount>1?`${item.moduleCount} modules`:null,item.isBarrel?(item.moduleCount>1?'contains barrels':'barrel'):null,item.inCycle?'cycle member':null].filter(Boolean).join(' · ') || 'module'; details.append(meta);
    addList('Imported by',node.incomers('node').map(n=>modules.get(n.id()).label));
    addList('Imports',node.outgoers('node').map(n=>modules.get(n.id()).label));
    if(item.moduleCount>1) addList('Modules',item.members);
  });

  let sortKey='totalExcess', sortDirection=-1;
  const columns=[['barrel','Barrel'],['importSites','Sites'],['actualCost','Cost'],['totalExcess','Excess'],['maxAmplification','Amplification'],['rewritableSymbols','Rewritable']];
  function renderTable(){
    const host=document.getElementById('barrels'); host.replaceChildren();
    if(!report.amplification.barrels.length){const p=document.createElement('p');p.className='empty';p.textContent='No imported barrels found.';host.append(p);return;}
    const rows=[...report.amplification.barrels].sort((a,b)=>{const av=a[sortKey]??Number.POSITIVE_INFINITY,bv=b[sortKey]??Number.POSITIVE_INFINITY;return (typeof av==='string'?av.localeCompare(bv):av-bv)*sortDirection});
    const table=document.createElement('table'),thead=document.createElement('thead'),tr=document.createElement('tr');
    columns.forEach(([key,label])=>{const th=document.createElement('th');const button=document.createElement('button');button.type='button';button.className='sort-button';button.textContent=label+(sortKey===key?(sortDirection>0?' ↑':' ↓'):'');button.addEventListener('click',()=>{if(sortKey===key)sortDirection*=-1;else{sortKey=key;sortDirection=key==='barrel'?1:-1}renderTable()});th.append(button);tr.append(th)});thead.append(tr);table.append(thead);
    const tbody=document.createElement('tbody'); rows.forEach(row=>{const tr=document.createElement('tr');columns.forEach(([key])=>{const td=document.createElement('td');let value=row[key];if(key==='maxAmplification')value=value===null?'∞':`${value.toFixed(1)}×`;if(key==='rewritableSymbols')value=`${row.rewritableSymbols}/${row.rewritableSymbols+row.skippedSymbols}`;td.textContent=value;if(row.hasSideEffects&&key==='barrel'){td.textContent+= ' ⚠';td.className='warn'}tr.append(td)});tbody.append(tr)});table.append(tbody);host.append(table);
  }
  renderTable();
})();
"##;
