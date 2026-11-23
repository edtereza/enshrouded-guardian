# Changelog

*Leia em [português](CHANGELOG.pt-BR.md).*

## 4.0.0 — 2026-11-23

- Remove the update-check configuration toggle and its legacy alias: version checks always run at startup and periodically in the background. Old toggle fields no longer disable checking.
- Checks remain informational only: announce newer releases; never download or install update binaries or upload saves. This retires the previous opt-out behavior.
- Remove the setting from both READMEs and retain unknown-field coverage in migration tests. Automated validation: Rust/Node tests and local Shroudtopia DLL compilation; no in-game test.

## 3.3.1 — 2026-11-23

- Store new hardcore backups and completion records under guardian/<SteamID64>/hardcore/history/<token>/; reconnect reference snapshots use the same history folder. Keep legacy hardcore-history backups readable without moving or overwriting them.
- Update both READMEs. Automated validation covers the new layout, legacy fallback and preservation; no in-game test.

## 3.3.0 — 2026-11-23

- Send first-join and imported-character reset announcements in Portuguese and English in the same chat line. Preserve message targeting and the existing notification/kick timing; translate the generic English character label too.
- Move hardcore documentation below Features, include the full configuration and hardcore setting descriptions, remove the English-storage feature note and named map references from both READMEs, and document bilingual announcements.
- Validation: automated Rust message/targeting/timing tests and local Shroudtopia DLL compilation; no new in-game test.

## 3.2.0 — 2026-11-23

- Add hardcore boolean configuration (default false), exact account/session checks, background reference backups and persisted pending resets.
- Integrate Live Map 1.14.0's verified native respawn backend for level/XP, inventory and skill resets with knowledge preserved and native start-point teleport, without a kick.
- Retain interrupted resets for reconnect recovery and serialize completion/death markers to avoid retiring a newer death.
- Automated validation: 30 Rust tests, 5 Node tests, 18 Live Map C++ tests, DLL export inspection and offline pattern/RVA probe (verified server accepted, unrelated executable rejected); no in-game validation. Requires the updated Live Map backend.

## 3.1.0 — 2026-11-20

- Character sheets now reference shared, versioned guardian/assets/<version>/ files for CSS, JavaScript, catalogs, UI artwork and item images. HTML retains only the current character data and works over file:// without fetch.
- Assets are generated once per version on the save worker; offline regeneration uses the same structure. Keep the assets folder when copying sheets. Existing generated standalone sheets remain valid until regenerated.
- Shared catalog paths support authenticated Live Map serving. Local validation: 27 Rust tests, 5 Node tests, 12 Live Map C++ tests and isolated HTTP checks passed (sheet, CSS, JS, images, key enforcement, caching and traversal). No in-game test.

## 3.0.9 — 2026-11-20

- Resolve guardian.json/guardiao.json coexistence by recursively merging normalized configuration keys. Older last-modified values win (legacy file wins ties); arrays remain whole.
- Back up both originals separately before replacing guardian.json and retiring guardiao.json. Existing runtime migration backups are not overwritten. Accept UTF-8 BOMs; malformed JSON/non-object roots leave both originals unchanged.
- Offline migration follows the same precedence and no longer overwrites one config backup with the other. Local automated validation: 26 Rust tests and 4 Node tests passed, including precedence, aliases, backups, idempotence and invalid inputs. No in-game test.

## 3.0.8 — 2026-11-20

- GitHub builds/releases provide guardian-{version}.zip (Guardian/Shroudtopia only) and guardian-{version}-full.zip (also includes app-local Visual C++ x64 runtime and notice).
- Both archives have independently validated file lists, hashes and manifests; SHA256SUMS.txt covers both and guardian.dll.
- Automated tests/package checks run in GitHub Actions; no gameplay change or new in-game test.

## 3.0.7 — 2026-11-20

- Removed Microsoft Visual C++ runtime DLLs and their notice from the server ZIP. Only Guardian/Shroudtopia files, base configs, installation notes, licenses and hash manifest are packaged.
- Packaging validates an explicit file allowlist and ZIP entries/hashes. Documented Visual C++ Redistributable x64 as a separately installed Shroudtopia prerequisite.
- GitHub Actions performs automated tests and package validation; no in-game change or new in-game testing.

## 3.0.6 — 2026-11-20

- Artifact name is guardian; the complete server archive is named guardian-{version}.zip. Packages are built exclusively in GitHub Actions, retained as artifacts for 90 days and automatically published to Releases for matching version tags.
- Removed obsolete deployment installers and machine-specific build paths. Verified bundled loader hashes against official Shroudtopia 0.1.1.

- GitHub Actions now tests/builds only the Shroudtopia adapter and packages guardian.dll, a root-extractable server ZIP and SHA-256 checksums. Version tags are checked against Cargo.toml.
- ZIP includes pinned existing Shroudtopia loader binaries, app-local Visual C++ runtime, clean base configuration, notices and a verified file manifest. Loader provenance is documented.
- Removed the example player snapshot from the embedded HTML template.
- Local automated tests and archive hash verification; no new in-game or GitHub-hosted run claimed.

## 3.0.5 — 2026-11-20

- Server HTML reports are now named server_save.html. Updated generation, offline helper, documentation and migration of previous character.html/personagem.html reports.
- Automated persistence and migration checks cover the new filename; no gameplay change.

## 3.0.4 — 2026-11-20

- Separated core pattern validation from extra fixed-layout compatibility. A divergent server build may install core hooks only when all required patterns and call-site checks pass.
- Divergent builds emit a warning in the server's own log, Guardian log and console. Fixed-layout chat, character-name and kick features stay disabled on unverified builds.
- Callback guards bypass Guardian memory access when hooks are disabled or installation fails, retaining original game calls. Automated guard/disabled-pointer tests; no unverified-build in-game validation is claimed.

## 3.0.3 — 2026-11-20

- Update checks and release download notices now target edtereza/enshrouded-guardian. Startup branding and issue links use the same repository; upstream credits remain intact.
- Parse release responses as JSON, ignore draft/prerelease payloads and report when no release can be read instead of silently implying success. Existing checkUpdates switch and polling intervals remain unchanged.
- Automated repository/response/version-comparison coverage. No GitHub release is created or published by this change.

## 3.0.2 — 2026-11-20

- Grouped models under templates/: save-viewer.html, guardian.example.json and initial-character/A.bin and K.bin. Updated build, migration and documentation references without changing model contents.
- Rebuilt and ran automated checks for the new paths; gameplay behavior unchanged.

## 3.0.1 — 2026-11-20

- Fixed remaining Portuguese source diagnostics, including hook scanner errors. Removed obsolete translations and the translation selector that already returned only English.
- Renamed modules to access.rs, scan.rs, updates.rs, continuity.rs, state.rs, install.rs and logging.rs; updated references and imports. User-requested Portuguese chat announcements and legacy migration names are preserved.
- Follow-up correction to 3.0.0 English storage standardization. Automated checks run for this delivery; no additional connected-player validation.

## 3.0.0 — 2026-11-20

- English storage names and schemas: guardian.json, guardian.log, events.jsonl, heartbeat.json, client_save.json and server_save.json, character.html, history, admin-history, original-first-join, rejected and OUTDATED.
- English structured fields, decisions, events and technical diagnostics. Startup migration preserves original structured data and historical logs under legacy-backup. Binary saves and player-provided names remain unchanged.
- English standalone save-viewer.html. Shroudtopia module renamed Guardian and installed as guardian.dll. User-requested Portuguese chat announcements remain unchanged; technical logs record the announcement event in English without repeating its text.
- Updated scripts and documentation. Breaking path/schema changes with data-preserving migration and conflict refusal.

## 2.3.0 — 2026-11-20

- Whitelist now uses `players`; legacy `steamIds` remains a fallback when `players` is absent.
- Storage now uses `guardian/`. Startup renames the legacy directory without discarding files; if both directories exist, activation stops to avoid overwriting saves.
- Updated examples, documentation and Windows scripts. Automated migration preservation/conflict coverage; in-game validation pending.

## 2.2.0 — 2026-11-20

- With the whitelist disabled, every accepted connection records SteamID64 and Steam display name in the console, guardiao.log and the `conexao_sem_whitelist` event in registro.jsonl, including reconnects and admins. Lists are not modified.
- Names come from the native join event on the supported build. Unreadable names are marked unavailable and control characters are removed. Automated validation; an in-game check is still pending.

## 2.1.0 — 2026-11-20

- Optional SteamID64 whitelist in `guardiao.json`, reloaded on each join. Disabled by default; when enabled, applies to everyone including admins and observe mode. Invalid configuration refuses new joins and logs the reason.
- Admin list makes the incoming character authoritative without first-join resets or reset kicks. Previous files are preserved in `admin-historico/`; write failures refuse entry and attempt to restore the previous files.
- Empty-by-default configuration example and documentation. Automated access-policy and admin persistence/backup tests; real in-game joins still need verification.

## 2.0.0 — 2026-11-18

- **Behavior change:** accounts without a stored server copy receive a level-1, zero-XP character, preserving appearance, with empty inventories, no purchased skills and only starting pants. The received character is backed up before replacement; failures refuse entry and log the reason.
- Global reset announcements at 5 seconds after login and 1 second later, followed 5 seconds later by a native kick without a ban, waiting for the complete reset save to be sent.
- Real character-name lookup with a generic fallback, connection-generation checks and timer cancellation. These hooks are enabled only on the verified server build.
- **Behavior change:** previous-build copies can be restored again; migration depends on the game loading and saving them. Copies predating periods without Guardian retain their exception.
- Read-only Save Editor parsing, separate client/server JSON reports and automatic per-Steam-account static `personagem.html`, with embedded item/UI artwork, equipment, backpack, skills and knowledge.
- Shroudtopia loader integration and a global first-join announcement.
- Consolidates local changes that still reported 1.1.0. Validation: 14 automated tests and isolated server startup; the full player reset/announcement/kick sequence still needs an in-game check.
- Future delivered changes increment the version using SemVer; identical rebuilds keep their version.

## 1.1.0 — 2026-11-04

### Game updates, without losing progress

- **Works on the November 2026 game update (build 1285601)** with no rebuild: the hooks are found by
  byte pattern, as in October.
- **Never rolls a player back.** If the hooks can't be installed, the guardian says so and marks every
  stored copy as outdated; when it works again, it refreshes each player's copy instead of restoring an old one.
- **Notices when the server ran without it.** It writes a heartbeat every minute (`guardiao-dados/ativo.json`);
  if the world was saved long after the last one (DLL removed, hooks missing), copies are marked the same way.
- **A copy from another game version is never used to replace a character.** Each copy now records the
  server build that wrote it; on the first join after a game update, the current character is kept and the
  copy is refreshed. Copies made by 1.0.0 have no build, so each player's first diverging join after
  upgrading is accepted once.
- **History:** the last 10 copies of each player are kept in `historico/`, ready to be put back by hand.

### A guardian you can see

- **Console messages for everything:** load, mode, players connecting, each join decision, saved copies,
  warnings and errors — with the character size before and after when something differs.
- **Boot banner** shown once the world has loaded, so it doesn't get lost among the game's boot lines.
  Hook addresses go only to `guardiao.log`.
- **BisectHosting:** its panel shows `DLLMods/loader.log` but not the server output, so the guardian now
  also appends its lines there. Installing on BisectHosting (upload to `DLLMods/`) is documented.
- **English by default**, Portuguese with `"idioma": "pt-BR"` (or `"language"`) in `guardiao.json`, which
  also accepts `{"mode": "observe"}` besides `{"modo": "observar"}`.
- **When it can't hook in,** it says clearly that it's OFF, that nothing is changed, and how to report it.

### New version notice

- Checks GitHub for a newer guardian on boot and then every 24 hours. When there is one, a framed notice
  appears in the console and is repeated every hour; the checks stop, since the guardian already knows.
- It never downloads anything and sends nothing about your server. Turn it off with `"checkUpdates": false`.
  This is the guardian's only network request.

## 1.0.0 — 2026-10-24

First release: the dedicated server keeps its own copy of each player's character and knowledge and puts it
back when someone joins with a different one, with the rejected save kept as evidence, an observe mode,
verified replacements and hooks found by byte pattern.
