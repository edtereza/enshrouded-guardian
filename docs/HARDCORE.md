# Integrated hardcore backend

Guardian 4.1.0 embeds its native hardcore implementation in guardian.dll. No companion mod or client component is required. Shroudtopia must activate it before native world programs are finalized (bootDelay=0). Unsupported executables or failed pattern/callsite verification refuse native activation.

The integrated backend owns program registration, simulation completion, death-callsite interception, respawn checks, native inventory transactions, level/XP and skill reset, and native StartLocation teleport. The Rust coordinator owns SteamID/session identity, durable reference backups, pending resets and character-save authority. Native logs go to Guardian's logger.

Optional integrations may query GuardianNativeBackendVersion (ABI 2) and forward manager/death events using GuardianNativeManager and GuardianNativeDeath. Forwarded deaths are deduplicated against native observations and are subject to Guardian's own identity checks. Updated integrations must not install their own hardcore simulation/program hooks. Older companions that installed those same hooks must be updated or disabled; a pattern failure refuses overlapping activation.

## Equipment in the death tombstone

The backend observes the game's native take/put/commit transaction from a dead player's inventory into a tombstone. Only an exact, committed source-player/destination-container witness links a grave. Root ownership, session identity, item and persistent entity identity are checked; name/proximity associations never authorize writes. Rollback/free discard uncommitted witnesses.

At respawn, selected equipment and equipment/customization inventory stacks move into empty slots of that verified tombstone using moveStack. This keeps the item's original persistent entity (level, durability, perks and gem data), instead of deleting and recreating it. Equipment moves and remaining character-inventory removal share one native commit/rollback. Full destination stacks and persistent identities are verified after commit before progression reset and teleport proceed.

If a grave is unavailable, collected/removing, unverified or full, equipment stays with the character and the reset remains pending. The mod does not guess a nearby grave or create one when vanilla creates none. After a restart, lost transient linkage is not guessed: equipped references remain intact pending a new verified native death transfer. Knowledge is preserved. A pending join restores the server reference before the native workflow, rather than deleting equipment ahead of a confirmed transfer.

Native inventory ABI research: https://github.com/matenm/emf-enshrouded/blob/main/docs/engine_parts/re_take_items.md (MIT). Dependency and research licenses: dependencies/native/THIRD_PARTY_NOTICES.txt.
