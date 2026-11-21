const fs = require('node:fs');
const path = require('node:path');
const version = fs.readFileSync(path.join(__dirname, '../Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
function writeAssets(root) {
  const directory = path.join(root, 'assets', version);
  if (fs.existsSync(path.join(directory, 'ready')) && fs.readFileSync(path.join(directory, 'ready'), 'utf8') === version) return;
  fs.mkdirSync(path.join(directory, 'images'), { recursive: true });
  const catalogs = JSON.parse(fs.readFileSync(path.join(__dirname, '../templates/assets/catalogs.json'), 'utf8'));
  const images = require('../data/save-editor/icons.json');
  const write = (name, content) => { const file = path.join(directory, name); fs.writeFileSync(file + '.tmp', content); fs.renameSync(file + '.tmp', file); };
  function image(name, uri) {
    if (!/^data:image\/webp;base64,[A-Za-z0-9+/]*={0,2}$/.test(uri)) throw new Error('Invalid image');
    write(name, Buffer.from(uri.split(',')[1], 'base64'));
    return name;
  }
  catalogs.icons = {};
  for (const [id, uri] of Object.entries(images)) {
    if (!/^\d+$/.test(id)) throw new Error('Invalid item ID');
    catalogs.icons[id] = image(`images/item-${id}.webp`, uri);
  }
  for (const [key, uri] of Object.entries(catalogs.ui)) catalogs.ui[key] = image(`images/ui-${Buffer.from(key).toString('hex')}.webp`, uri);
  for (const [key, uri] of Object.entries(catalogs.cssImages)) {
    if (!/^[A-Za-z0-9-]+$/.test(key)) throw new Error('Invalid CSS image');
    image(`images/${key}.webp`, uri);
  }
  delete catalogs.cssImages;
  for (const name of ['viewer.js', 'viewer.css']) write(name, fs.readFileSync(path.join(__dirname, '../templates/assets', name)));
  write('catalogs.js', 'window.guardianCatalogs=' + JSON.stringify(catalogs).replace(/</g, '\\u003c') + ';\n' + fs.readFileSync(path.join(__dirname, '../templates/assets/catalog-paths.js'), 'utf8'));
  write('ready', version);
}
function render(root, report) {
  writeAssets(root);
  const template = fs.readFileSync(path.join(__dirname, '../templates/save-viewer.html'), 'utf8');
  return template.replace(/<script id="guardian-data" type="application\/json">[\s\S]*?<\/script>/,
    () => '<script id="guardian-data" type="application/json">' + JSON.stringify({ reports: [{ label: 'Server save', data: report }] }).replace(/</g, '\\u003c').replace(/\u2028/g, '\\u2028').replace(/\u2029/g, '\\u2029') + '</script>')
    .replaceAll('__GUARDIAN_ASSET_BASE__', '../assets/' + version);
}
module.exports = { render, writeAssets };
