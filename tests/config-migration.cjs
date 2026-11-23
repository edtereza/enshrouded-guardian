const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { migrate } = require('../scripts/migrate-storage.cjs');

for (const [oldTime, newTime, oldWins] of [[10, 20, true], [20, 10, false], [10, 10, true]]) {
  test(`offline merge precedence ${oldTime}/${newTime}, original backups and repeat`, () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'guardian-merge-'));
    try {
      fs.mkdirSync(path.join(root, 'guardian'));
      const old = '\uFEFF{"modo":"observar","admins":[],"whitelist":{"enabled":true,"steamIds":["76561198000000001"]},"legacyOnly":7}';
      const current = '{"mode":"enforce","admins":["76561198000000002"],"whitelist":{"enabled":false,"players":[],"note":"kept"},"customSetting":false}';
      for (const [name, content, time] of [['guardiao.json', old, oldTime], ['guardian.json', current, newTime]]) {
        fs.writeFileSync(path.join(root, name), content);
        fs.utimesSync(path.join(root, name), time, time);
      }
      const backup = migrate(root);
      const merged = JSON.parse(fs.readFileSync(path.join(root, 'guardian.json'), 'utf8'));
      assert.equal(merged.mode, oldWins ? 'observe' : 'enforce');
      assert.deepEqual(merged.admins, oldWins ? [] : ['76561198000000002']);
      assert.equal(merged.whitelist.enabled, oldWins);
      assert.deepEqual(merged.whitelist.players, oldWins ? ['76561198000000001'] : []);
      assert.equal(merged.whitelist.steamIds, undefined);
      assert.equal(merged.whitelist.note, 'kept');
      assert.equal(merged.legacyOnly, 7);
      assert.equal(merged.customSetting, false);
      assert.equal(fs.readFileSync(path.join(backup, 'guardiao.json.original'), 'utf8'), old);
      assert.equal(fs.readFileSync(path.join(backup, 'guardian.json.original'), 'utf8'), current);
      assert.equal(fs.existsSync(path.join(root, 'guardiao.json')), false);
      // A later standalone migration preserves the resolved configuration.
      const bytes = fs.readFileSync(path.join(root, 'guardian.json'));
      // Separate timestamp avoids the helper's intentionally exclusive backup name.
      fs.renameSync(backup, backup + '-previous');
      migrate(root);
      assert.deepEqual(fs.readFileSync(path.join(root, 'guardian.json')), bytes);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
}

test('offline migration rejects invalid roots/JSON before modifying either config', () => {
  for (const invalid of ['{', '[]', 'null']) for (const invalidOld of [true, false]) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'guardian-invalid-'));
    try {
      fs.mkdirSync(path.join(root, 'guardian'));
      const old = invalidOld ? invalid : '{}', current = invalidOld ? '{}' : invalid;
      fs.writeFileSync(path.join(root, 'guardiao.json'), old);
      fs.writeFileSync(path.join(root, 'guardian.json'), current);
      assert.throws(() => migrate(root));
      assert.equal(fs.readFileSync(path.join(root, 'guardiao.json'), 'utf8'), old);
      assert.equal(fs.readFileSync(path.join(root, 'guardian.json'), 'utf8'), current);
      assert.deepEqual(fs.readdirSync(root).sort(), ['guardiao.json', 'guardian', 'guardian.json'].sort());
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  }
});
