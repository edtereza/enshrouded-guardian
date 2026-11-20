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

Includes Shroudtopia loader DLLs, app-local Microsoft Visual C++ x64 runtime,
base JSON files, license notices and package-manifest.json with SHA-256 hashes.
Requires Windows 10/Server 2016 or newer and an installed dedicated server.
The game, Windows system DLLs and player saves are not included.
Guardian templates, initial-character model and catalogs are embedded in its
DLL. guardian/ storage is created at runtime. No Node.js or Save Editor needed.
Other existing Shroudtopia mods are not enabled by this base config.

Loader provenance and hashes: licenses/shroudtopia-README.md.
Runtime terms: https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution
Project: https://github.com/edtereza/enshrouded-guardian
Original Guardian creator: Nicolas Ferreira.
