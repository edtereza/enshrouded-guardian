const {test}=require('node:test'), assert=require('node:assert/strict'), fs=require('node:fs'), os=require('node:os'), path=require('node:path'), vm=require('node:vm');
const {render}=require('../scripts/viewer-assets.cjs');
test('shared assets, local/HTTP URL resolution, safe report data and reuse',()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'guardian-viewer-'));
 try {
  const report={steam_id_64:'123',steam_account_name:'</script><script>bad</script>',character:{level:1,equipment:[{itemId:3972122124}]}};
  const html=render(root,report),version=fs.readFileSync(path.join(__dirname,'../Cargo.toml'),'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1],dir=path.join(root,'assets',version);
  assert(!html.includes('data:image'));assert(!html.includes('<style>'));assert(html.length<10000);
  const json=html.match(/<script id="guardian-data" type="application\/json">([\s\S]*?)<\/script>/)[1];assert(!json.includes('<'));assert.deepEqual(JSON.parse(json).reports[0].data,report);
  assert.equal(fs.readFileSync(path.join(dir,'images/item-3972122124.webp')).subarray(0,4).toString(),'RIFF');
  const source=fs.readFileSync(path.join(dir,'catalogs.js'),'utf8');assert(!source.includes('data:image'));assert(!source.includes('steam_account_name'));
  for(const url of ['file:///server/guardian/assets/'+version+'/catalogs.js','http://localhost/assets/'+version+'/catalogs.js?key=abc-123']){
   const context={window:{},document:{currentScript:{src:url}},URL};vm.runInNewContext(source,context);
   const image=new URL(context.window.guardianCatalogs.icons['3972122124']);assert(image.pathname.endsWith('/assets/'+version+'/images/item-3972122124.webp'));
   assert.equal(image.searchParams.get('key'),url.startsWith('http')?'abc-123':null);
  }
  for(const match of fs.readFileSync(path.join(dir,'viewer.css'),'utf8').matchAll(/url\((images\/[^)]+)\)/g))assert(fs.existsSync(path.join(dir,match[1])));
  const before=fs.statSync(path.join(dir,'viewer.css')).mtimeMs;render(root,{steam_id_64:'456'});assert.equal(fs.statSync(path.join(dir,'viewer.css')).mtimeMs,before);
 }finally{fs.rmSync(root,{recursive:true,force:true});}
});
