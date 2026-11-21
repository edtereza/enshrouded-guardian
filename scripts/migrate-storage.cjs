// Offline migration helper. Run with the server stopped and its directory as argument.
const fs = require('node:fs');
const path = require('node:path');
const dictionary = require('../data/storage-english.json');

function convert(value, field = '') {
  if (Array.isArray(value)) return value.map(item => convert(item, field));
  if (value && typeof value === 'object') {
    const result = {};
    for (const [original, item] of Object.entries(value)) {
      const key = dictionary.keys[original] || original;
      if (key !== original && Object.hasOwn(value, key)) continue;
      result[key] = convert(item, key);
    }
    return result;
  }
  if (typeof value === 'string') {
    if (['kind', 'decision', 'origin', 'reference', 'layout', 'format', 'mode'].includes(field)) {
      return dictionary.keys[value] || value;
    }
    if (['issues', 'skillPointsNote', 'parser'].includes(field)) {
      return dictionary.messages[value] || (/[^\x00-\x7f]/.test(value)
        ? 'Legacy diagnostic preserved in the original migration backup.' : value);
    }
  }
  return value;
}

function renamed(name) {
  if (dictionary.files[name]) return dictionary.files[name];
  for (const [old, replacement] of Object.entries({
    _primeira_entrada: '_first_join', _referencia: '_reference',
    _copia_desatualizada: '_outdated_copy', _divergente_observado: '_difference_observed',
    _substituido: '_replaced', _versao_do_jogo_mudou: '_game_build_changed'
  })) name = name.replaceAll(old, replacement);
  return name;
}

function write(file, value) {
  fs.writeFileSync(file + '.tmp', typeof value === 'string' ? value : JSON.stringify(value, null, 2));
  fs.renameSync(file + '.tmp', file);
}

function readConfig(file) {
  const value = convert(JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '')));
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Configuration must be a JSON object');
  if (value.whitelist && typeof value.whitelist === 'object' && !Array.isArray(value.whitelist)) {
    if (Object.hasOwn(value.whitelist, 'steamIds')) {
      if (!Object.hasOwn(value.whitelist, 'players')) value.whitelist.players = value.whitelist.steamIds;
      delete value.whitelist.steamIds;
    }
  }
  return value;
}

function mergeConfig(newer, older) {
  const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  if (!object(newer) || !object(older)) return older;
  const result = { ...newer };
  for (const [key, value] of Object.entries(older)) {
    Object.defineProperty(result, key, { value: Object.hasOwn(newer, key) ? mergeConfig(newer[key], value) : value,
      enumerable: true, configurable: true, writable: true });
  }
  return result;
}

function migrate(root) {
  root = path.resolve(root);
  const data = path.join(root, 'guardian');
  if (!fs.statSync(data).isDirectory()) throw new Error('Expected guardian directory');
  const oldConfig = path.join(root, 'guardiao.json');
  const newConfig = path.join(root, 'guardian.json');
  const hasNew = fs.existsSync(newConfig), hasOld = fs.existsSync(oldConfig);
  let config = readConfig(hasNew ? newConfig : oldConfig);
  if (hasNew && hasOld) {
    const old = readConfig(oldConfig);
    const legacyOlder = fs.statSync(oldConfig, { bigint: true }).mtimeNs <= fs.statSync(newConfig, { bigint: true }).mtimeNs;
    config = legacyOlder ? mergeConfig(config, old) : mergeConfig(old, config);
  }
  const backup = path.join(root, 'guardian-legacy-backup-' + new Date().toISOString().replace(/[:.]/g, '-'));
  fs.cpSync(data, backup, { recursive: true, errorOnExist: true, force: false });
  for (const name of ['guardiao.json', 'guardian.json', 'shroudtopia.json']) {
    const file = path.join(root, name);
    if (fs.existsSync(file)) fs.copyFileSync(file, path.join(backup, name + '.original'));
  }
  write(newConfig, config);
  if (fs.existsSync(oldConfig)) fs.renameSync(oldConfig, path.join(backup, 'legacy-config.original'));

  function walk(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (entry.isSymbolicLink() || entry.name === 'legacy-backup') continue;
      const file = path.join(directory, entry.name);
      const destination = path.join(directory, renamed(entry.name));
      if (entry.isDirectory()) walk(file);
      else if (['guardiao.log', 'registro.jsonl'].includes(entry.name)) {
        const archive = path.join(backup, 'archived-logs', path.relative(data, file).split(path.sep).map(renamed).join(path.sep) + '.original');
        fs.mkdirSync(path.dirname(archive), { recursive: true });
        fs.renameSync(file, archive);
        continue;
      } else if (entry.name.endsWith('.json')) {
        write(file, convert(JSON.parse(fs.readFileSync(file, 'utf8'))));
      }
      if (destination !== file) {
        if (fs.existsSync(destination)) throw new Error('Migration destination already exists: ' + destination);
        fs.renameSync(file, destination);
      }
    }
  }
  walk(data);
  const loaderFile = path.join(root, 'shroudtopia.json');
  if (fs.existsSync(loaderFile)) {
    const loader = JSON.parse(fs.readFileSync(loaderFile, 'utf8'));
    if (loader.mods?.Guardiao) {
      if (loader.mods.Guardian) throw new Error('Both Guardian module entries exist');
      loader.mods.Guardian = loader.mods.Guardiao;
      delete loader.mods.Guardiao;
      write(loaderFile, loader);
    }
  }

  // Regenerate standalone pages with the English template and current report.
  const { render } = require('./viewer-assets.cjs');
  for (const account of fs.readdirSync(data, { withFileTypes: true })) {
    if (!account.isDirectory() || !/^\d+$/.test(account.name)) continue;
    const directory = path.join(data, account.name);
    const reportFile = path.join(directory, 'server_save.json');
    if (!fs.existsSync(reportFile)) continue;
    const report = JSON.parse(fs.readFileSync(reportFile, 'utf8'));
    write(path.join(directory, 'server_save.html'), render(data, report));
  }
  for (const name of ['guardian.log', 'events.jsonl']) fs.closeSync(fs.openSync(path.join(data, name), 'a'));
  const version = fs.readFileSync(path.join(__dirname, '../Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
  write(path.join(data, 'migration.json'), { version, migratedAtUtc: new Date().toISOString(), originalBackup: backup });
  return backup;
}

module.exports = { migrate, convert, mergeConfig };
if (require.main === module) {
  if (!process.argv[2]) throw new Error('Usage: node scripts/migrate-storage.cjs SERVER_DIRECTORY');
  console.log('Original files preserved at:', migrate(process.argv[2]));
}
