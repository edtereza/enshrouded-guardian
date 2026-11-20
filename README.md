# Enshrouded Guardian

Server-side character management for **Enshrouded dedicated servers**.

Guardian stores a reference copy of each player's character on the server, checks incoming characters against that copy, and provides access controls, first-join resets, backups, structured logs, and standalone character reports.

**This project is exclusively intended for dedicated servers running `enshrouded_server.exe`. It is not a client mod and is not intended for single-player or player-hosted co-op sessions.**

Repository: [edtereza/enshrouded-guardian](https://github.com/edtereza/enshrouded-guardian)

## Human and AI contributions

Development of this expanded version combines human and AI contributions: parts of the code have been modified with AI assistance, while other parts have been modified directly by a human. This does not change the original authorship credits below.

## Origin and original author

**All credit for the original creation of Enshrouded Server Guardian belongs to Nicolas Ferreira.**

This project began by recompiling and expanding his original
[Enshrouded Server Guardian](https://github.com/nicolasferreiraxn/Enshrouded-Server-Guardian).
It exists only because Nicolas Ferreira created and shared that project first.
His work provides the foundation; our contribution is to extend its features
and maintain this expanded version. We do not claim authorship of the original
project, and its copyright notices and license are preserved.

The original project addressed a shared-server problem: players could import
progress made elsewhere or roll back their local character. That motivation
comes from [Nicolas Ferreira's original README](https://github.com/nicolasferreiraxn/Enshrouded-Server-Guardian#readme).
This version retains that foundation while adding the features below.

## Features

- **Server-authoritative characters:** restore the stored reference when a regular player joins with a different character.
- **First-join reset:** accounts without a stored reference start at level 1 with zero XP, empty inventories, no purchased skills, and starting pants. Appearance is preserved, and the incoming save is backed up before replacement.
- **Optional SteamID64 whitelist:** control which accounts may join.
- **Admin exceptions:** accept an administrator's incoming character as the new server reference, preserving the previous copy.
- **Connection records:** with the whitelist disabled, record each connected player's SteamID64 and Steam display name.
- **Reset announcements and reconnection:** broadcast the configured reset messages and request a non-ban kick after the timed sequence, waiting for the complete reset save to be sent.
- **Static character reports:** generate `server_save.html` with equipment, backpack, skills, knowledge, and validation results. Images and data are embedded; no web server is needed.
- **English storage:** configuration, technical logs, report fields, and generated filenames use English. The configured in-game announcements remain in Portuguese.
- **Preserved history:** keep previous reference copies, rejected saves, original first-join characters, and administrator replacement backups.

## Dependencies

### Required to run

| Dependency | Purpose |
| --- | --- |
| Enshrouded Dedicated Server, Windows x64 | The server executable Guardian hooks into. |
| Shroudtopia, when using `mods/guardian.dll` | Loads the Guardian module on the dedicated server. |

The alternative `dbghelp.dll` build uses a native proxy-loading mechanism and does not require Shroudtopia. Use one loading method per server.

Players do **not** need EMF, Shroudtopia, or a Guardian client component. The Save Editor application is not required at runtime: the read-only parser, catalogs, and report artwork are bundled into the build. A browser is only needed to open generated HTML reports.

### Required to compile

- A current stable Rust toolchain with the `x86_64-pc-windows-msvc` target and Cargo.
- Microsoft Visual Studio Build Tools with the C++ toolchain.
- A Windows SDK providing the required headers and libraries.
- For the Shroudtopia adapter, the MSVC C++ compiler used by the `cc` build dependency.

Cargo resolves these dependencies from `Cargo.toml` and `Cargo.lock`:

| Crate | Purpose |
| --- | --- |
| `windows-sys` | Windows APIs, memory hooks, and the release-check HTTP client. |
| `ruzstd` | Decoding Zstandard-compressed character blocks. |
| `serde_json` | Configuration, structured records, reports, and migration data. |
| `cc` | Compiling the optional Shroudtopia C++ adapter. |

The project uses the static C runtime configuration in `.cargo/config.toml`.

### Optional tools

- **Node.js:** runs `scripts/migrate-storage.cjs` for an offline storage migration and HTML regeneration. It is not required by the running mod.
- **Git:** cloning and contributing to the repository.

## Installation with Shroudtopia

Download the **guardian** artifact from [GitHub Actions](https://github.com/edtereza/enshrouded-guardian/actions/workflows/build.yml),
then extract the inner `guardian-{version}.zip` beside the server executable with
the server stopped. Back up existing DLLs and JSON settings first. This is the
only distribution channel. The manual layout below explains the included files.

1. Stop the dedicated server.
2. Install Shroudtopia for the dedicated server.
3. Place the Shroudtopia build at `mods/guardian.dll`, relative to the folder containing `enshrouded_server.exe`.
4. Enable Guardian in the existing `shroudtopia.json`, preserving the other modules:

   ```json
   {
     "mods": {
       "Guardian": {
         "active": true
       }
     }
   }
   ```

5. Copy `templates/guardian.example.json` to `guardian.json` beside the server executable and configure it.
6. Start the server and check `guardian/guardian.log` for `Active (hooks installed)`.

Back up the existing DLL and character data before upgrading. Do not load an older `guardiao.dll` alongside `guardian.dll`.

For the native proxy build, place its `dbghelp.dll` beside `enshrouded_server.exe`. Do not overwrite another loader's DLL or install both Guardian variants together.

### Managed hosts and Wine

The [upstream installation guide](https://github.com/nicolasferreiraxn/Enshrouded-Server-Guardian#-installation)
describes the proxy build on Wine and managed hosts. It reports a different
installation location on BisectHosting: `DLLMods/dbghelp.dll`, with loader timing
configured through `DLLMods/chainloader.ini`. Other hosts may reserve or replace
`dbghelp.dll`; confirm their loading method before deployment.

These are upstream deployment reports, not validation of this expanded version
on those hosts. The Shroudtopia instructions above remain a separate loading
method. In this version, configuration and data still belong beside the server
executable: `guardian.json` and `guardian/`.

When the host provides `DLLMods/loader.log`, Guardian can mirror console output
there. Check both that log and `guardian/guardian.log` when diagnosing startup.

Legacy deployment-specific installers have been removed. Use the GitHub Actions package.

## Configuration

Create `guardian.json` beside `enshrouded_server.exe`:

```json
{
  "mode": "enforce",
  "checkUpdates": true,
  "whitelist": {
    "enabled": false,
    "players": []
  },
  "admins": []
}
```

Use complete SteamID64 values **as strings**, for example `"76561199244298543"`.

| Setting | Behavior |
| --- | --- |
| `mode: "enforce"` | Applies character reference and first-join reset rules. |
| `mode: "observe"` | Logs regular character differences without replacing or resetting them. |
| `checkUpdates` | Enables the release notification check. Does not download or install updates. |
| `whitelist.enabled` | When true, only accounts in `whitelist.players` may join, including administrators. Applies in observe mode too. |
| `whitelist.players` | Allowed SteamID64 strings. With the whitelist disabled, connections are recorded without automatically changing this list. |
| `admins` | SteamID64 strings whose incoming character replaces the server reference, without a first-join reset or reset kick. This does not grant native game administrator permissions. |

Whitelist and admin lists are reloaded on each join. Changing the operating mode requires a restart. Invalid access configuration refuses new joins and records the reason.

The updater checks the latest published public release of
[edtereza/enshrouded-guardian](https://github.com/edtereza/enshrouded-guardian/releases/latest)
at startup and every 24 hours. This legacy notification checks Releases, not
Actions artifacts; builds distributed exclusively through Actions will not be
discovered by this checker. Once a newer release is found, Guardian repeats the console notice
every hour. It never installs the update automatically and logs unavailable checks.

For an established server, start with `mode: "observe"` and review the recorded
differences before enabling enforcement. Observe mode still writes server
reference copies as players progress. Access restrictions and the explicit admin
override remain independent of regular character enforcement.

## How the server reference works

The native proxy forwards `MiniDumpWriteDump` to the Windows implementation.
The Shroudtopia adapter uses its loader instead. Both builds use the same core
character-management logic:

1. A session hook associates the connection handle with the SteamID64.
2. A save-read hook checks the character submitted during entry and applies the
   access, admin and reference rules.
3. Save-write hooks capture the character and knowledge blocks produced by the
   game and refresh the account's server reference.

Before applying a reference, Guardian preserves the incoming character and a
snapshot of the reference used for that decision. After a replacement it reads
the data back and checks the result, recording a failure if the values differ.

The disk copy takes precedence over the cached reference on the next join.
An external edit to the stored character can therefore affect a regular player's
next entry. Admin-list accounts follow the opposite rule: their incoming
character becomes the new reference.

### Reading join decisions

Look for records with `kind: "join"` in `guardian/events.jsonl`. Common `decision`
values are:

| Decision | Meaning in this version |
| --- | --- |
| `first_join` | No server reference exists; enforce mode initializes eligible incoming characters before establishing the reference. |
| `matching` | Incoming character and knowledge blocks match the reference. |
| `replaced` | The stored copy was applied and verified. |
| `difference_observed` | A difference was recorded without applying the regular replacement rule. |
| `replacement_failed` | The replacement could not be verified; inspect the accompanying diagnostics. |
| `outdated_copy` | Continuity protection allows the incoming character because the reference predates a period without Guardian. |
| `whitelist_denied` | The account is not allowed by the enabled whitelist. |
| `admin_client_authoritative` | The admin's incoming character became the server reference. |
| `reset_failed` | Guardian could not safely complete the first-entry reset. |

These English decision names replace the original project's Portuguese names.
The first-join behavior also differs from upstream: enforce mode can reset an
imported character rather than accepting its existing progression as a baseline.

## Storage layout

```text
EnshroudedServer/
├── enshrouded_server.exe
├── guardian.json
├── mods/
│   └── guardian.dll
└── guardian/
    ├── guardian.log
    ├── events.jsonl
    ├── heartbeat.json
    └── <steam_id_64>/
        ├── A.bin
        ├── K.bin
        ├── meta.json
        ├── client_save.json
        ├── server_save.json
        ├── server_save.html
        ├── history/
        ├── admin-history/
        ├── original-first-join/
        └── rejected/
```

`A.bin` and `K.bin` hold the binary character and knowledge data. JSON reports expose the fields Guardian can decode. `server_save.html` is refreshed when the server save is captured; reload the file in your browser to see the latest generated report.

Storage is keyed by **Steam account**, not by a unique character UUID. Different characters from the same account share that account's reference. Names are display information, not identity keys.

> **Important: switching characters on the same Steam account**
>
> In `enforce` mode, if another character joins using the same SteamID64,
> Guardian applies the character save stored for that account on the server and
> sends it back to the game client. **The local save of the character used to
> join will be overwritten based on the server's reference**, replacing that
> character's own progression. Use a separate Steam account if you need an
> independent server reference. This replacement rule does not apply in
> `observe` mode or to accounts in `admins`, whose incoming character instead
> becomes the new server reference. Copies marked outdated by continuity
> protection also follow the separate refresh rule described below.

### Evidence and recovery

`rejected/<timestamp>_replaced.bin` preserves the incoming character rejected by
the reference rule; `<timestamp>_reference.bin` preserves the reference used at
that moment. Compare that pair, rather than comparing an old rejection against
a reference that has since changed through normal gameplay.

`history/` retains up to ten recent join-time reference snapshots. Admin
replacements and first-entry resets have their own backup directories. To
restore a reference manually, stop the server, back up the current account
directory, and restore a consistent `A.bin`, `K.bin`, and `meta.json` set.
An admin-list account will still replace that restored reference on its next join.

Do not delete an account directory as a routine recovery step: an account with
no reference is subject to this version's first-join reset rules.

## Building and testing

Run from a Windows developer environment with Rust and the MSVC tools available:

```powershell
# Shroudtopia adapter
cargo test --locked --target x86_64-pc-windows-msvc --features shroudtopia
cargo build --locked --release --target x86_64-pc-windows-msvc --features shroudtopia
```

Rename the resulting `target/x86_64-pc-windows-msvc/release/dbghelp.dll` to `guardian.dll` when installing it in `mods/`. The Rust library output retains the name `dbghelp` for compatibility with the alternative proxy build.

```powershell
# Native proxy variant
cargo build --locked --release --target x86_64-pc-windows-msvc
```

The optional `scripts/build-shroudtopia.cmd` runs these commands in an MSVC developer environment.

## Templates and migration

- `templates/save-viewer.html`: the report UI embedded into the DLL at compile time.
- `templates/guardian.example.json`: an example configuration.
- `templates/initial-character/`: the initial character model used for resets.
- `data/save-editor/`: catalogs and embedded item icons.

Changing an embedded template requires recompiling the DLL. The server does not need a separate copy of `save-viewer.html`.

Startup migration handles supported legacy storage names and schemas. Original structured files and historical logs are archived instead of being rewritten as purported translations. Conflicting destinations stop activation to avoid overwriting saves.

For an offline migration and HTML regeneration, with the server stopped:

```powershell
node scripts/migrate-storage.cjs "D:\path\to\EnshroudedServer"
```

The helper preserves a complete backup outside the active `guardian/` directory.

## Compatibility and validation

Guardian locates core hooks by byte patterns. Optional chat, name-reading, and kick functionality is enabled only on the verified server build currently identified by PE timestamp `6aeb143e`. A future game update may require new patterns or layout checks; inspect the startup log after updating.

On a divergent build, Guardian may install core hooks only after every required
pattern and call-site check succeeds. It writes a divergence warning to the
native server log, Guardian log and console; extra fixed-layout features stay
disabled. A pattern match alone does not certify the entire new engine layout.

Restoring a copy from an older game build does not itself convert the save. Migration to the current game format depends on the game successfully loading and saving that character. Copies marked outdated after a period without Guardian retain separate protection against stale restoration.

The current automated suite covers parsing, access rules, reset models, message targeting and timing, name lookup, persistence, and migrations. Isolated startup has been checked. The full reset/announcement/kick sequence still requires verification with a connected player; automated tests alone do not establish end-to-end compatibility.

After an update, inspect the latest startup entries rather than assuming that a
running server means Guardian is active. Missing or ambiguous required patterns
disable its enforcement; original game calls remain in place. A divergence
warning means core pattern checks passed or are being attempted, while optional
fixed-layout features remain unavailable.

Guardian uses `heartbeat.json` and world-save timestamps to detect periods when
the server ran without it. Copies marked `OUTDATED` are refreshed from the
incoming character instead of being applied as stale references. Keep the data
directory when upgrading so this continuity information is preserved.

## Scope and limitations

- **Not a general-purpose anti-cheat:** the reference mechanism does not monitor
  or prevent every modification made during a connected session. Changes the
  game accepts and saves can become part of its next reference.
- **Character data, not world rollback:** Guardian's reference files cover the
  character and knowledge blocks. They do not restore terrain, buildings or
  chest contents from a previous world state.
- **No save uploads:** the release checker contacts GitHub to request release
  metadata. It does not upload character or world files and does not install
  downloaded binaries. Set `checkUpdates: false` to disable that check.
- **Engine-dependent:** byte-pattern discovery and compatibility guards reduce
  unsupported access; they do not establish that every future game build will
  behave identically.

## License and credits

Distributed under the [MIT license](LICENSE). **Nicolas Ferreira is the original creator** of
[Enshrouded Server Guardian](https://github.com/nicolasferreiraxn/Enshrouded-Server-Guardian).
This repository is a recompilation and feature expansion of his work, not a claim
to its original creation. Existing copyright notices are preserved. Save Editor
parser and asset provenance are documented in [data/save-editor/README.md](data/save-editor/README.md).

The [upstream credits](https://github.com/nicolasferreiraxn/Enshrouded-Server-Guardian#-credits)
also acknowledge brabb3l's [kfc-parser / Enshrouded Mod Loader](https://github.com/Brabb3l/kfc-parser)
and BisectHosting's loader collaboration. Related original-author tools include
the [Enshrouded Save Editor](https://github.com/nicolasferreiraxn/Enshrouded-Save-Editor)
and [Enshrouded Interactive Map](https://github.com/nicolasferreiraxn/Enshrouded-Interactive-Map).

This is an unofficial community project, with no affiliation or official support
from Keen Games.

See the [changelog](CHANGELOG.md) for version history.

## GitHub builds and complete server package

GitHub Actions builds **only the Shroudtopia adapter** on pushes to main, pull
requests and manual dispatch. Distribution is exclusively through the guardian
artifact in Actions → successful run → Artifacts. It contains guardian.dll,
guardian-{version}.zip and SHA256SUMS.txt. No automatic Release upload is performed;
artifacts are retained for 90 days. Tags v<version> must match Cargo.toml.
Automated tests run before packaging; they do not replace in-game testing.

Extract guardian-<version>.zip directly beside
enshrouded_server.exe. It includes mods/guardian.dll, the pinned Shroudtopia
loader, app-local x64 Visual C++ runtime, clean JSON defaults, installation notes,
licenses and a hash manifest. Stop the server and back up DLLs and JSON settings
before extracting. Restore/merge your existing settings afterward. No player
saves or game files are included. The loader binaries are from the development
server and verified byte-for-byte against the official Shroudtopia 0.1.1 release,
not compiled by Guardian CI.
Their provenance and hashes are recorded in dependencies/shroudtopia/README.md.

The workflow invokes scripts/package-shroudtopia.ps1; distribution packaging
is restricted to GitHub Actions. Local builds are for development/testing only.
The script discovers Visual Studio's x64 redistributable CRT directory.
Microsoft's redistribution documentation describes these app-local runtime files:
https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution

The embedded HTML template contains no example player save; runtime reports
receive the current account's data from Guardian.
