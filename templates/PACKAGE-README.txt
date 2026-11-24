GUARDIAN - SHROUDTOPIA DEDICATED SERVER PACKAGE

For Windows x64 dedicated servers only. Players need no client mod.

1. Stop the dedicated server before installing.
2. Back up existing mods/guardian.dll, winmm.dll, shroudtopia.dll,
   guardian.json and shroudtopia.json before extracting an update.
3. Extract the ZIP contents directly beside enshrouded_server.exe.
4. On an existing installation, restore/merge your backed-up JSON settings.
   The packaged JSON files are clean defaults, not your existing configuration.
5. Start the server and check guardian/guardian.log for activation/compatibility.

The archive has no enclosing directory. mods/guardian.dll is the Guardian
loading method in this package. Disable any previous Guardian dbghelp.dll proxy
installation before using it. Do not delete another mod's DLL by name alone.

Two archive options are published from the same GitHub build:
- guardian-{version}.zip: only Guardian and Shroudtopia files.
- guardian-{version}-full.zip: the same files plus app-local Visual C++ x64 DLLs.
Install ONE archive. Both extract directly into the server root.

Both include Guardian/Shroudtopia DLLs,
base JSON files, license notices and package-manifest.json with SHA-256 hashes.
Requires Windows 10/Server 2016 or newer, an installed dedicated server and
Microsoft Visual C++ Redistributable x64 (required by Shroudtopia).
Standard ZIP: install Microsoft Visual C++ Redistributable x64 separately.
Full ZIP: includes the app-local Visual C++ runtime and its license notice.
The game, Windows system DLLs and player saves are not included.
Guardian templates, initial-character model and catalogs are embedded in its
DLL. guardian/ storage is created at runtime. Shared viewer assets are exported
to guardian/assets/{version}/ once per version. When copying character sheets,
copy assets/ too and keep it beside the SteamID directories. Live Map 1.9.0
serves these shared files with its existing access-key protection.
No Node.js or Save Editor needed.
Other existing Shroudtopia mods are not enabled by this base config.

Loader provenance and hashes: licenses/shroudtopia-README.md.

Project: https://github.com/edtereza/enshrouded-guardian
Original Guardian creator: Nicolas Ferreira.
