"use strict";
const bundle={...window.guardianCatalogs,...JSON.parse(document.getElementById("guardian-data").textContent)};
const reports=bundle.reports.map((r,i)=>({label:r.label||"Report "+(i+1),data:r.data||r}));
const state={report:0,view:"equipment",container:"main",query:"",selection:null,page:0};
const $=id=>document.getElementById(id);
const list=value=>Array.isArray(value)?value:[];
const num=value=>value===null||value===undefined?"—":Number(value).toLocaleString("en-US");
const element=(tag,text,className)=>{const e=document.createElement(tag);if(text!==undefined&&text!==null)e.textContent=String(text);if(className)e.className=className;return e};
const add=(parent,...children)=>{for(const child of children)if(child)parent.append(child);return parent};
const report=()=>reports[state.report]?.data||{};
const character=()=>report().character||{};
const catalog=id=>bundle.items[String(id)]||{};
const itemName=e=>e.name||catalog(e.itemId).en||"Item #"+e.itemId;
const skillName=s=>bundle.skills.nodes[String(s.id)]?.en||s.name||"Skill #"+s.id;
const knowledgeName=k=>{const meta=bundle.knowledge[String(k.id)]||{};return meta.i!==undefined?(catalog(meta.i).en||k.name||"#"+k.id):(meta.n||k.name||"#"+k.id)};
const categories={Weapons:"Weapon",Equipment:"Equipment",Materials:"Material",Consumables:"Consumable",Decorations:"Decoration",Tools:"Tool"};
const slots={Armour_Head:"Head",Armour_UpperBody:"Peitoral",Armour_Arms:"Arms",Armour_LowerBody:"Lower Body",Armour_Feet:"Feet",Armour_Ring:"Ring",Shield:"Escudo",HookShot:"Grappling hook",Glider:"Glider",Backpack:"Backpack"};
const rarities={Common:["Common","#a0a99e"],Uncommon:["Uncommon","#8bb982"],Rare:["Rare","#80acd9"],Epic:["Epic","#bb97dc"],Legendary:["Legendary","#d8b66e"]};
const rarity=e=>rarities[catalog(e.itemId).rarity]||["No rarity informada","#6c7a6b"];
const searchMatches=(name,id)=>!state.query||(name+" "+id).toLocaleLowerCase("en-US").includes(state.query);
const fmtDate=value=>{if(!value)return"Time unavailable";const d=new Date(value);return Number.isNaN(d.getTime())?String(value):d.toLocaleString("en-US",{timeZone:"America/Sao_Paulo",dateStyle:"short",timeStyle:"medium"})+" · Sao Paulo"};
function imageFor(id){const source=bundle.icons[String(id)];if(!source)return null;const img=element("img");img.src=source;img.alt="";img.loading="lazy";return img}

// Recursos originais do Save Editor incorporados: nenhuma conexão externa.
function uiImage(name, className) {
  const image = element('img', null, className);
  image.src = bundle.ui[name];
  image.alt = '';
  image.setAttribute('aria-hidden', 'true');
  return image;
}
function applyEditorArtwork() {
  const menuIcons = {
    equipment: 'recipe_category_armor', inventory: 'recipe_category_supplies',
    skills: 'recipe_category_magical', knowledge: 'recipe_category_upgrade',
    validation: 'recipe_category_blocks', raw: 'recipe_category_resources'
  };
  document.querySelectorAll('[data-view]').forEach(button => {
    const count = button.querySelector('.nav-count');
    const label = button.firstChild.textContent.replace(/^[^A-Za-zÀ-ÿ]+/, '').trim();
    button.replaceChildren(uiImage(menuIcons[button.dataset.view], 'menu-icon'), element('span', label, 'menu-label'));
    if (count) button.append(count);
  });
  const brand = document.querySelector('.brand strong');
  brand.replaceChildren(uiImage('tinted/level_badge_gold', 'brand-badge'), element('span', 'GUARDIAN'));
  document.documentElement.style.setProperty('--editor-slot', 'url("' + bundle.ui['tinted/slot_bg'] + '")');
  document.documentElement.style.setProperty('--editor-pattern', 'url("' + bundle.ui.fancy_box_background_pattern + '")');
  document.documentElement.style.setProperty('--editor-frame', 'url("' + bundle.ui.fancy_box_frame_full + '")');
}

function steamName() { return report().steam_account_name || report().nome_conta_steam || 'Steam account'; }
function availableName(){const r=report(),c=character();return c.name||r.nome_personagem||r.character_name||null}
function populateReports() {}
function updateHeader(){
 const r=report(),c=character(),validation=r.validation||{},problems=list(r.issues),name=availableName();
 $("steam-id").textContent=r.steam_id_64||"Not provided";
 $("source-label").textContent=r.origin==="client"?"Client input":r.origin==="server"?"Server save":r.origin==="disk_copy"?"Disk copy":"Report";
 $("report-description").textContent=name?name+" · Steam ID "+(r.steam_id_64||"not provided"):"Steam ID "+(r.steam_id_64||"not provided")+" · Character name unavailable in report";
 $("level").textContent=num(c.level);$("xp").textContent=num(c.xp);$("points").textContent=num(validation.usedSkillPoints);$("known").textContent=Array.isArray(r.knowledge?.entries)?num(r.knowledge.entries.length):"—";
 const readable=validation.characterReadable===true&&validation.knowledgeReadable===true;
 $("status").textContent=problems.length?num(problems.length)+" warning"+(problems.length>1?"s":""):readable?"Validated":"Partial read";
 $("status").className="status"+(problems.length||!readable?" warning":"");
 const counts={equipment:Array.isArray(c.equipment)?c.equipment.length:"—",inventory:Array.isArray(c.main)?c.main.length+list(c.extra).length:"—",skills:Array.isArray(c.skills)?c.skills.length:"—",knowledge:Array.isArray(r.knowledge?.entries)?r.knowledge.entries.length:"—",validation:problems.length};
 document.querySelectorAll("[data-count]").forEach(e=>e.textContent=counts[e.dataset.count]);
 $("metadata").replaceChildren(element("span", "✓ Server copy · Steam " + (r.steam_id_64 || "—")), element("time", "Last save update: " + fmtDate(r.updatedAtUtc)));
}
function detailsList(entries){const dl=element("dl",null,"details-list");for(const [label,value] of entries){if(value===undefined||value===null)continue;const row=element("div");add(row,element("dt",label),element("dd",value));dl.append(row)}return dl}
function empty(text){return element("div",text,"empty-message")}
function lineDetail(parent,label,value){add(parent,add(element("div",null,"item-info-line"),element("span",label),element("span",value)))}
function statName(id){const pair=bundle.stats.names?.[String(id)];return pair?.[1]||pair?.[0]||"#"+id}
function formatStat(l){if(l.e)return"Scales with level";const v=Number(l.v);if(!Number.isFinite(v))return String(l.v??"—");const sign=l.s&&v>=0?"+":"";if(l.f==="%")return sign+num(v)+"%";if(l.f==="s"||l.f==="ms"){if(Math.abs(v)<60000)return sign+(v/1000).toLocaleString("en-US",{maximumFractionDigits:1})+" s";return sign+Math.floor(v/60000)+":"+String(Math.round(v%60000/1000)).padStart(2,"0")+" min"}return sign+v.toLocaleString("en-US",{maximumFractionDigits:2})}
function itemStats(parent,e){
 const stats=bundle.stats.itens?.[String(e.itemId)];if(!stats)return;
 const balance=bundle.stats.balanceamento||{};
 const attrs=list(stats.a).filter(a=>a.t!==2298204525);
 const base=e.level!==undefined&&stats.d&&balance.genericWeaponSeed!==undefined&&balance.powerCreepWeapon!==undefined?balance.genericWeaponSeed*(stats.d.mod??1)*(stats.d.vel??1)*(1+balance.powerCreepWeapon*e.level):null;
 const armor=e.level!==undefined&&stats.r&&balance.armorSetup?.offset!==undefined?balance.armorSetup.offset+balance.armorSetup.gradient*e.level:null;
 if(attrs.length||base!==null||armor!==null){parent.append(element("hr"));parent.append(element("div","Catalog attributes","subhead"))}
 if(base!==null)for(const [type,fraction] of Object.entries(stats.d.dist||{})){if(fraction>0&&fraction<=1&&bundle.stats.damageTypes?.[type])lineDetail(parent,statName(bundle.stats.damageTypes[type]),num(Math.round(base*fraction)))}
 if(armor!==null){lineDetail(parent,"Physical armor",num(Math.round(armor*(stats.r.physical||0))));lineDetail(parent,"Magical armor",num(Math.round(armor*(stats.r.magical||0))))}
 attrs.forEach(a=>lineDetail(parent,statName(a.t),formatStat(a)));
 const upgrades=list(stats.p).map((u,i)=>({u,i})).filter(x=>x.u);
 if(upgrades.length){parent.append(element("hr"));parent.append(element("div","Catalog upgrades","subhead"));for(const {u,i}of upgrades){const box=element("div",null,"upgrade"+(i<(e.upgrades||0)?" applied":""));box.append(element("p","Upgrade "+(i+1)+(i<(e.upgrades||0)?" · applied":" · available")));for(const a of list(u.v))lineDetail(box,statName(a.t),formatStat(a));if(u.rd&&base!==null)for(const [type,fraction]of Object.entries(u.rd))lineDetail(box,statName(u.rn??bundle.stats.damageTypes?.[type]??type),"+"+(base*fraction).toLocaleString("en-US",{maximumFractionDigits:1}));parent.append(box)}}
}
function renderItemDetail(panel,entry,container){
 panel.hidden = !entry;
 if (!entry) { panel.replaceChildren(); return; }
 panel.replaceChildren();panel.setAttribute("aria-label","Selected item details");
 if(!entry){const box=element("div",null,"detail-content");add(box,element("div","Item details","subhead"),element("p","Select an item to view its fields and attributes."));panel.append(box);return}
 const info=catalog(entry.itemId),[label,color]=rarity(entry);const head=element("div",null,"detail-header");const art=element("div",null,"detail-art");art.append(imageFor(entry.itemId)||element("span","◇","item-initial"));add(head,art,element("small",label),element("h3",itemName(entry)));head.style.borderBottomColor=color;
 const body=element("div",null,"detail-content");body.append(detailsList([
  ["Location",container],["Save slot",entry.slot],["Item ID",entry.itemId],["Category",categories[info.category]||info.category],["Equipment slot",slots[info.slot]],["Level",entry.level],["Amount",entry.count],["Catalog maximum stack",info.maxStack],["Durability",entry.durability===undefined?null:num(entry.durability)+" / "+num(entry.maxDurability)],["Liquid",entry.liquid===undefined?null:num(entry.liquid)+" / "+num(entry.maxLiquid)],["Upgrades",entry.upgrades===undefined?null:num(entry.upgrades)+" / "+num(entry.maxUpgrades)],["Appearance / dye",entry.appearance],["Flags",entry.flags===undefined?null:"0x"+entry.flags.toString(16).padStart(2,"0")]
 ]));
 if(typeof entry.socket==="string"&&/^[0-9a-f]{10}$/i.test(entry.socket)){const bytes=entry.socket.match(/../g).map(x=>parseInt(x,16));const id=(bytes[0]+bytes[1]*256+bytes[2]*65536+bytes[3]*16777216)>>>0;body.append(element("hr"));body.append(element("div","Socket","subhead"));lineDetail(body,"Gem",catalog(id).en||"#"+id);lineDetail(body,"Level",bytes[4]);lineDetail(body,"ID",id)}
 itemStats(body,entry);const raw=element("details",null,"raw-toggle");add(raw,element("summary","Original item fields"),element("pre",JSON.stringify(entry,null,2)));body.append(raw);add(panel,head,body);
}
function renderInventory(content) {
  const c = character();
  if (!Array.isArray(c.main)) {
    content.append(empty('Inventory is not available in this save yet.'));
    return;
  }
  const extent = entries => entries.reduce((max, entry) => Math.max(max, entry.slot + 1), 0);
  const backpack = list(c.equipment).find(entry => catalog(entry.itemId).slot === 'Backpack' && entry.count > 0);
  const mainCount = Math.max(40, extent(c.main));
  const extraCount = Math.max(bundle.capacities[catalog(backpack?.itemId).debug] || 0, extent(list(c.extra)));
  const bagKeys = [];
  for (let slot = 0; slot < mainCount; slot++) {
    if (slot < 16 || slot >= 32) bagKeys.push('main:' + slot);
  }
  for (let slot = 0; slot < extraCount; slot++) bagKeys.push('extra:' + slot);
  const entryAt = key => {
    if (typeof key !== 'string') return null;
    const [container, slot] = key.split(':');
    return list(c[container]).find(entry => entry.slot === Number(slot) && entry.count > 0);
  };
  const selected = entryAt(state.selection);
  const visibleSelection = selected && searchMatches(itemName(selected), selected.itemId) ? selected : null;
  const layout = element('div', null, 'backpack-layout');
  const left = element('div', null, 'backpack-left');
  const scroll = element('div', null, 'bag-scroll');
  scroll.setAttribute('aria-label', 'Itens da backpack');
  const grid = element('div', null, 'bag-grid');
  function slotButton(key, badge) {
    const entry = entryAt(key);
    const matches = entry && searchMatches(itemName(entry), entry.itemId);
    const button = element('button', null, 'bag-slot' + (!entry ? ' vacant' : '') + (matches && state.selection === key ? ' selected' : '') + (entry && !matches ? ' filtered' : ''));
    button.type = 'button';
    button.dataset.inventoryKey = key;
    button.setAttribute('aria-pressed', String(Boolean(matches && state.selection === key)));
    button.setAttribute('aria-label', entry ? itemName(entry) + ', count ' + entry.count + ', ' + key : 'Empty slot ' + key);
    button.disabled = !matches;
    if (badge) button.append(element('span', badge, 'action-key'));
    if (entry) {
      button.title = itemName(entry);
      button.append(imageFor(entry.itemId) || element('span', itemName(entry).slice(0, 2), 'item-initial'));
      if (entry.count > 1) button.append(element('span', num(entry.count), 'bag-count'));
      if (entry.level !== undefined) button.append(element('span', num(entry.level), 'bag-level'));
      if (entry.durability !== undefined && entry.maxDurability > 0) {
        const meter = element('div', null, 'bag-durability');
        const fill = element('span');
        fill.style.width = Math.max(0, Math.min(100, entry.durability / entry.maxDurability * 100)) + '%';
        meter.append(fill);
        button.append(meter);
      }
      button.onclick = () => { state.selection = state.selection === key ? null : key; render(); };
    }
    return button;
  }
  for (const key of bagKeys) grid.append(slotButton(key));
  scroll.append(grid);
  left.append(scroll);
  const separator = element('div', null, 'bag-separator');
  left.append(separator);
  for (const [offset, title] of [[16, 'ALT · Swap'], [24, 'Current action bar']]) {
    const row = element('div', null, 'action-row');
    row.append(element('div', title, 'action-label'));
    const bar = element('div', null, 'action-slots' + (offset === 24 ? ' current-action' : ''));
    for (let index = 0; index < 8; index++) bar.append(slotButton('main:' + (offset + index), offset === 24 ? index + 1 : null));
    row.append(bar);
    left.append(row);
  }
  const detail = element('aside', null, 'detail character-detail backpack-detail');
  renderItemDetail(detail, visibleSelection, typeof state.selection === 'string' && state.selection.startsWith('extra:') ? 'Backpack extra' : 'Main inventory');
  layout.append(left, detail);
  content.append(layout);
}
function renderSkills(content){
 const skills=list(character().skills).filter(s=>searchMatches(skillName(s),s.id));
 if(!skills.length){content.append(empty(state.query?"No skills match the search.":"No skills available in this report."));return}
 const layout=element("div",null,"content-layout"),items=element("div",null,"list"),detail=element("aside",null,"detail");let selected=skills.find(s=>s.id===state.selection);
 for(const s of skills){const no=bundle.skills.nodes[String(s.id)]||{};const b=element("button",null,"list-item"+(selected===s?" selected":""));b.type="button";const text=element("span",null,"text");add(text,element("strong",skillName(s)),element("small","ID "+s.id+" · "+(no.class||no.type||"Skill")));add(b,text,element("span",s.level+" / "+(no.maxLevel||1),"rank"));b.onclick=()=>{state.selection=s.id;render()};items.append(b)}
 if(selected){const no=bundle.skills.nodes[String(selected.id)]||{},head=element("div",null,"detail-header");head.append(element("h3",skillName(selected)));const body=element("div",null,"detail-content");body.append(detailsList([["ID",selected.id],["Level",selected.level],["Catalog maximum",no.maxLevel],["Cost per level",no.cost],["Type",no.type],["Class",no.class]]));body.append(element("hr"));body.append(element("p",no.descEn||no.desc||"Description unavailable.","skill-description"));add(detail,head,body)}else{const body=element("div",null,"detail-content");add(body,element("div","Skill details","subhead"),element("p","Select a skill to view its description and catalog limits."));detail.append(body)}
 add(layout,items,detail);content.append(layout)
}
function renderKnowledge(content){
 const entries=list(report().knowledge?.entries).filter(k=>searchMatches(knowledgeName(k),k.id));if(!entries.length){content.append(empty(state.query?"No knowledge matches the search.":"No knowledge available in this report."));return}
 const pageSize=50;state.page=Math.min(state.page,Math.max(0,Math.ceil(entries.length/pageSize)-1));const start=state.page*pageSize,wrap=element("div",null,"table-wrap"),table=element("table"),thead=element("thead"),tr=element("tr");
 ["Knowledge","ID","Type","Estado recebido"].forEach(t=>tr.append(element("th",t)));thead.append(tr);table.append(thead);const body=element("tbody");
 for(const k of entries.slice(start,start+pageSize)){const row=element("tr"),info=bundle.knowledge[String(k.id)]||{},type=k.type||info.t;const name=element("td");add(name,element("span",knowledgeName(k)));const bits=type==="r"?[[1,"Vista"],[2,"Fabricada"]]:[[1,"Obtido"],[2,"Usado"],[4,"Inspecionado"]];const meaning=bits.filter(([b])=>(Number(k.value)&b)!==0).map(([,n])=>n);const flags=element("td",num(k.value));if(meaning.length)flags.append(element("small",meaning.join(" · ")));add(row,name,element("td",k.id,"mono"),element("td",type==="r"?"Receita":type==="i"?"Item":type||"—"),flags);body.append(row)}table.append(body);wrap.append(table);content.append(wrap);
 const pages=element("div",null,"pagination"),prev=element("button","Previous","button"),next=element("button","Next","button");prev.type=next.type="button";prev.disabled=start===0;next.disabled=start+pageSize>=entries.length;prev.onclick=()=>{state.page--;render()};next.onclick=()=>{state.page++;render()};add(pages,element("span",(start+1)+"–"+Math.min(start+pageSize,entries.length)+" of "+num(entries.length)),prev,next);content.append(pages)
}
function renderValidation(content){
 const r=report(),v=r.validation||{};const rows=[["Character decoding",v.characterReadable===true?"Complete":"Unavailable",v.characterReadable===true],["Knowledge decoding",v.knowledgeReadable===true?"Complete":"Unavailable",v.knowledgeReadable===true],["Used skill points",num(v.usedSkillPoints),null],["Total available points",v.totalSkillPoints===null||v.totalSkillPoints===undefined?"Not provided":num(v.totalSkillPoints),null]];
 rows.forEach(([label,value,good])=>{const row=element("div",null,"validation-row");add(row,element("span",label),element("span",value,good===true?"good":good===false?"warning-text":"muted"));content.append(row)});
 if(v.skillPointsNote)content.append(add(element("div",null,"warning-box"),element("p",v.skillPointsNote)));
 if(list(r.issues).length){content.append(element("h3","Recorded issues"));for(const issue of r.issues)content.append(element("div",issue,"warning-box"))}else{const p=element("p","No issues recorded in this report.","good");p.style.marginTop="22px";content.append(p)}
}
function renderRaw(content){
 const r=report();const section=element("div",null,"technical");const fields=[["Identity and capture",{steam_id_64:r.steam_id_64,connectionHandle:r.connectionHandle,origin:r.origin,updatedAtUtc:r.updatedAtUtc,parser:r.parser}],["Block A",r.blockA],["Opaque character fields",character().opaque||{appearanceHex:character().appearanceHex}],["Complete report",r]];
 for(const [label,data]of fields){const d=element("details");add(d,element("summary",label),element("pre",JSON.stringify(data??null,null,2)));section.append(d)}content.append(section)
}
function render(){
 updateHeader();document.querySelectorAll("[data-view]").forEach(b=>{if(b.dataset.view===state.view)b.setAttribute("aria-current","page");else b.removeAttribute("aria-current")});
 const sections={equipment:["Equipment","Worn equipment, accessories and backpack."],inventory:["Inventories","Select a stack to view its data."],skills:["Skills","Nodes and ranks recorded in the character skill tree."],knowledge:["Knowledge","Discoveries and recipes recorded in block K."],validation:["Validation","Validation results recorded by Guardian."],raw:["Received data","Report fields and opaque data preserved by the reader."]};
 const tabName = {equipment:"Character",inventory:"Backpack",skills:"Skills",knowledge:"Knowledge"}[state.view] || sections[state.view][0];
 $("breadcrumb").textContent = steamName() + " / " + tabName;
 $("section-title").textContent=sections[state.view][0];$("section-subtitle").textContent=sections[state.view][1];$("search").hidden=!["equipment","inventory","skills","knowledge"].includes(state.view);
 const content=$("content");content.replaceChildren();if(!reports.length){content.append(empty("Open client_save.json or server_save.json to begin."));return}
 if(state.view==="equipment")renderCharacter(content);else if(state.view==="inventory")renderInventory(content,false);else if(state.view==="skills")renderSkills(content);else if(state.view==="knowledge")renderKnowledge(content);else if(state.view==="validation")renderValidation(content);else renderRaw(content)
}

function renderCharacter(content) {
  const c = character();
  const layout = element('div', null, 'character-layout');
  const equipment = element('section', null, 'equipment-panel');
  equipment.append(element('h2', 'Equipment'));
  const headings = element('div', null, 'equipment-headings');
  headings.append(element('span', ''), element('span', 'ARMOR'), element('span', 'VISUAL'));
  equipment.append(headings);
  const entries = list(c.equipment);
  // A lista do save é densa; a máscara fornece os espaços reais (como no Save Editor).
  const activeSlots = [];
  for (let bit = 0; bit < 32; bit++) if ((c.equipMask >>> bit) & 1) activeSlots.push(bit);
  const byEquipmentSlot = new Map(entries.map(entry => [activeSlots[entry.slot] ?? entry.slot, entry]));
  const types = ['Armour_Head','Armour_UpperBody','Armour_Arms','Armour_LowerBody','Armour_Feet'];
  let selected = entries.find(e => e.slot === state.selection);
  function equipmentButton(entry, texture) {
    const button = element('button', null, 'equipment-slot' + (entry && selected === entry ? ' selected' : ''));
    button.type = 'button';
    button.setAttribute('aria-pressed', String(Boolean(entry && selected === entry)));
    button.setAttribute('aria-label', entry ? itemName(entry) : 'Empty slot');
    button.append(entry ? imageFor(entry.itemId) || uiImage(texture, 'empty-equipment-icon') : uiImage(texture, 'empty-equipment-icon')); 
    button.disabled = !entry;
    if (entry) button.onclick = () => { state.selection = state.selection === entry.slot ? null : entry.slot; render(); };
    return button;
  }
  types.forEach((type, index) => {
    const entry = byEquipmentSlot.get(index + 5);
    const row = element('div', null, 'equipment-row');
    const text = element('div');
    text.append(element('div', slots[type] || type), element('small', entry ? itemName(entry) : 'empty'));
    const visual = byEquipmentSlot.get(index);
    const texture = ['equipmentslot_head','equipmentslot_upperbody','equipmentslot_arms','equipmentslot_legs','equipmentslot_feet'][index];
    row.append(text, equipmentButton(entry, texture), equipmentButton(visual, texture));
    equipment.append(row);
  });
  equipment.append(element('h3', 'ACCESSORIES', 'accessory-heading'));
  const accessories = element('div', null, 'accessories');
  const armor = new Set(types);
  const others = [10, 11, 12, 13, 14, 15].map(slot => byEquipmentSlot.get(slot));
  const labels = ['Ring', 'Ring', 'Shield', 'Grappling hook', 'Glider', 'Backpack'];
  for (let index = 0; index < Math.max(6, others.length); index++) {
    const entry = others[index];
    const cell = element('div');
    cell.append(equipmentButton(entry, ['equipmentslot_ring','equipmentslot_ring','equipmentslot_shield','equipmentslot_hookshot','equipmentslot_glider','equipmentslot_backpack'][index % 6]), element('small', entry ? slots[catalog(entry.itemId).slot] || itemName(entry) : labels[index]));
    accessories.append(cell);
  }
  equipment.append(accessories);
  const summary = element('section', null, 'character-card');
  const crest = element('div', null, 'character-crest');
  crest.append(uiImage('levelup_ornament', 'crest-ornament'), uiImage('attribute_tabIllustration_basics', 'crest-medallion'));
  const badge = element('div', null, 'crest-badge');
  badge.append(uiImage('tinted/level_badge_gold'), element('b', num(c.level)));
  crest.append(badge);
  const steamAccountName = steamName();
  summary.append(crest, element('h1', steamAccountName));
  summary.append(element('p', report().steam_id_64 || 'Steam ID unavailable', 'muted'));
  summary.append(detailsList([['Level', num(c.level)], ['XP towards next level', num(c.xp)], ['Used skill points', num(report().validation?.usedSkillPoints)], ['Steam ID', report().steam_id_64], ['Save format', c.layout]]));
  summary.append(element('p', 'Data read from the server save. Missing attributes are not estimated.', 'character-note'));
  const detail = element('aside', null, 'detail character-detail');
  renderItemDetail(detail, selected, 'Equipment');
  layout.append(equipment, summary, detail);
  content.append(layout);
}
function validateReport(value){if(!value||typeof value!=="object"||Array.isArray(value)||!("character"in value)||!("validation"in value))throw new Error("Select a JSON report generated by Guardian.");for(const field of ["equipment","main","extra","skills"]){const array=value.character?.[field];if(array!==undefined&&!Array.isArray(array))throw new Error("Invalid field: character."+field)}for(const field of ["equipment","main","extra"])for(const item of list(value.character?.[field]))if(!item||!Number.isInteger(item.itemId)||!Number.isInteger(item.slot)||item.slot<0||item.slot>255)throw new Error("Invalid item in "+field);for(const s of list(value.character?.skills))if(!s||!Number.isInteger(s.id)||!Number.isInteger(s.level))throw new Error("Invalid skill.");if(value.knowledge?.entries!==undefined&&!Array.isArray(value.knowledge.entries))throw new Error("Invalid knowledge list.");for(const k of list(value.knowledge?.entries))if(!k||!Number.isInteger(k.id)||!Number.isInteger(k.value))throw new Error("Invalid knowledge entry.");if(value.issues!==undefined&&!Array.isArray(value.issues))throw new Error("Invalid issue list.");return value}
async function loadFiles(files){const errors=[];let loaded=false;for(const file of files){try{const data=validateReport(JSON.parse(await file.text()));reports.push({label:file.name+" · "+(data.origin||"report")+" · "+(data.steam_id_64||"account not provided"),data});state.report=reports.length-1;loaded=true}catch(error){errors.push(file.name+" · "+error.message)}}$("file-error").textContent=errors.join(" | ");$("file-error").hidden=!errors.length;if(loaded){state.selection=null;state.page=0;state.query="";$("search").value="";populateReports();render()}}
document.querySelectorAll("[data-view]").forEach(b=>b.onclick=()=>{state.view=b.dataset.view;state.selection=null;state.query="";state.page=0;$("search").value="";render()});
$("search").addEventListener("input",e=>{state.query=e.target.value.trim().toLocaleLowerCase("en-US");state.page=0;render()});
applyEditorArtwork();populateReports();render();
