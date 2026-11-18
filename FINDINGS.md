# More Dragon Eggs — upstream findings

Read-only analysis of the original Java mod for the Horizon rewrite. No code
was copied; the port re-implements the behaviour in Rust.

## Repo and version

| | |
|---|---|
| Repo | <https://github.com/Darkere/moredragoneggs> |
| Pack version | **5.0**, `gradle.properties` `mod_version=5.0`, `minecraft_version=1.21.1`, NeoForge `21.1.57` |
| Commit analysed | **`450678c`** ("update to 1.21.1"), the 5.0 / 1.21.1 tree the ATM10 jar is built from |
| Branch tip (read for comparison) | `4b132ae` ("port to 26.1.2"), `mod_version=6.0`, NeoForge `26.1.2.5-beta` |
| Licence | `gradle.properties` declares `mod_license=MIT`; the repo ships **no LICENSE file** (checked `git ls-tree` at 450678c and 4b132ae). The campaign brief rules ARR because no licence file ships; the port is local-only either way. |

The clone was `--depth 1 --branch 1.21`, then `git fetch --unshallow` to reach
450678c (`git log -S'mod_version=5.0'`). It lived at
`ports/_shared/upstream/moredragoneggs` and was **deleted** after this file was
written (the findings below are the record).

## Files read

- `src/main/java/com/darkere/moredragoneggs/mixin/DragonEggMixin.java` (450678c: 41 lines)
- `src/main/java/com/darkere/moredragoneggs/MoreDragonEggs.java` (58 lines)
- `src/main/java/com/darkere/moredragoneggs/ServerConfig.java` (21 lines)
- `src/main/resources/moredragoneggs.mixins.json`, `gradle.properties`,
  `src/main/templates/META-INF/neoforge.mods.toml`, `README.md`
- The same files at the branch tip `4b132ae` (6.0) for the naming change.

## Behaviour (5.0, file:line)

`DragonEggMixin.java:18-39` — mixin on `EnderDragonFight`:

- `:21-23` shadows `boolean previouslyKilled`.
- `:25-27` shadows the `ServerLevel level`.
- `:30` shadows the `BlockPos origin` (the fight's exit-portal origin).
- `:32` injects at **HEAD** of `setDragonKilled(Lnet/minecraft/world/entity/boss/enderdragon/EnderDragon;)V`,
  so before vanilla sets `previouslyKilled = true`.
- `:34` `pos = level.getHeightmapPos(Heightmap.Types.MOTION_BLOCKING, EndPodiumFeature.getLocation(origin))`.
- `:35-37` **if `previouslyKilled`** (i.e. this is not the first kill) →
  `level.setBlockAndUpdate(pos, Blocks.DRAGON_EGG.defaultBlockState())`.
- `:38` always `MoreDragonEggs.PlaceDragonHead(pos, level)` (the method gates
  itself on the config).

`MoreDragonEggs.java:25-58` — `PlaceDragonHead`:

- `:26-27` return immediately unless `SERVER_CONFIG.shouldSpawnHead()`.
- `:29-31` walk `pos` down while `block != BEDROCK && pos.getY() > 0`
  (`pos = pos.below()`), i.e. from the heightmap cell to the podium's bedrock.
- `:33` `Entity e = level.getNearestPlayer(pos.x, pos.y, pos.z, -1, null)`
  (any distance, same level).
- `:35-39` with a player:
  `Direction.getNearest((int)(e.getX() - pos.getX()), 0, (int)(e.getZ() - pos.getZ()))`;
  with none: `Direction.NORTH`.
- `:41-57` up to 4 tries: `headPos = pos.relative(dir)`; if that block is
  bedrock, step once more (`:45-47`, "BetterEnd Reforked style end podiums");
  if it is air, schedule `TickTask(0, …)` placing
  `DRAGON_WALL_HEAD.defaultBlockState().setValue(WallSkullBlock.FACING, dir)`
  (`:49-53`); otherwise `dir = dir.getClockWise()` (`:56`).

`ServerConfig.java:8-20` — one server-config key `spawnHead`, comment "Also
Spawn a Dragon head on the Pedestal.", `define("spawnHead", false)`
(`:13`), read by `shouldSpawnHead()` (`:18-20`).

`neoforge.mods.toml:24-26` — "This mod makes it so a dragon egg spawns every
time the dragon dies." (The README is one line + the CurseForge link.)

## Semantics summary

- There is **one egg per dragon kill after the first**, at the heightmap
  position of the End podium (x=z=0). The first kill's egg is vanilla's.
- If the first egg was never collected, a later kill re-writes the same block
  (upstream uses `setBlockAndUpdate`), so no second egg stacks there.
- `spawnHead=true` places a dragon **wall** head (not a floor head) on every
  kill, first included: beside the podium's central bedrock, on the side
  facing the nearest player, one block out; four clockwise tries at most.
- The head placement is deferred by one tick (`TickTask(0)`); the egg is immediate.

## Differences at the 6.0 branch tip (comparison only)

- Field renamed `previouslyKilled` → `hasPreviouslyKilledDragon`
  (DragonEggMixin at 4b132ae:23).
- `Direction.getNearest` is called with a `NORTH` fallback explicitly;
  `EndPodiumFeature.getLocation` still exists.
- Otherwise the same two placements.

## Where the Horizon rewrite differs

- **Trigger:** no `mob_killed` for the dragon at the pin. `native_mob_killed`
  is only the acting player's melee kill of a regular mob
  (`game.rs:11060` → `attack_mob`); `attack_dragon` never calls it, and the
  dragon is `game.dragon`, not a mob. The port watches `Fight.killed`'s
  false → true edge at `level_tick_post` (the pin sets it in
  `dragon_killed`, `dragon.rs:2259-2260`) — the same point in the tick as the
  mixin's `setDragonKilled` HEAD, one level pass earlier.
- **The "already killed" flag:** the pin has `Fight.previously_killed`
  (`dragon.rs:40-41`, `pub`), the same flag. The port keeps its own per-world
  counter in `<world>/moredragoneggs/state.tsv` (authoritative; re-read at each
  kill) and seeds it from `previously_killed` the first time it sees a loaded
  fight, so a world that already had its first kill still gets eggs.
- **Heightmap:** the port scans (0,0) top-down for the first
  `full_collision || legacy_solid` block and places at +1, exactly like the
  pin's own first-kill egg (`dragon.rs:2250-2257`), instead of a
  `MOTION_BLOCKING` heightmap (which also counts fluids). On the vanilla
  podium the two agree; recorded as a simplification.
- **Config:** `config/moredragoneggs.conf` with `spawn_head` (the NeoForge
  `spawnHead` spelling is also accepted) and default false.
- **Head:** immediate `native_set_block` at the end of the level pass instead
  of the 0-tick `TickTask`; the placement point is the same.
