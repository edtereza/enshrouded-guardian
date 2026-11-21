These catalogs are copied from the local Enshrouded Save Editor's data/items.json,
data/skills.json and data/knowledge.json. The corresponding read-only Rust
parser is in src/save.rs. The source parser is shared/character.mjs,
shared/knowledge.mjs, src/treecheck.js and checkModel in src/tauri-api.js.

Copyright (c) 2026 Nicolas Ferreira. MIT license, reproduced in the repository
LICENSE. Catalog updates must be checked against tests/fixtures/save-editor.json,
whose expected models were obtained with the source editor parser.

icons.json contains the editor's item icons as embedded WebP data URIs. The viewer
generator decodes these into versioned shared WebP files under guardian/assets/.
All sheets and Live Map use the same assets instead of embedding images per page.
The UI artwork and catalogs in templates/assets/ are from the same editor.

templates/initial-character/A.bin and K.bin are a fresh level-1/zero-XP server save captured
locally (empty inventories, no purchased skills, starting pants, three default
knowledge entries). First-join resets copy only the incoming appearance/scales
prefix into this model. They are not taken from a progressed character.
