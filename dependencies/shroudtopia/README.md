# Bundled Shroudtopia loader

Upstream: https://github.com/s0t7x/shroudtopia (MIT, Miguel Oppermann).

These x64 binaries match the official Shroudtopia 0.1.1 release published on
2026-02-16. Both SHA-256 hashes were verified against the downloaded official
archive on 2026-11-20:
https://github.com/s0t7x/shroudtopia/releases/tag/0.1.1

They are pinned third-party binaries, not compiled by the Guardian workflow.

| File | SHA-256 |
| --- | --- |
| shroudtopia.dll | 53CEEE2DD51C12B7DB1371092EDDCD1118ABEAEABA6E3B482986A071AB9B1A11 |
| winmm.dll | 2519AB074597C39F9241456711370CAC907E86957833A7AE16705714CE6D7297 |

The package script verifies these hashes and includes the upstream MIT license.
Only Guardian and Shroudtopia files are included in the archive. Shroudtopia
requires Microsoft Visual C++ Redistributable x64, installed separately on the
server. Microsoft runtime DLLs are not bundled.

No Flight Mod, basics mod, server executable, credentials, player saves or
existing server configuration is distributed.
