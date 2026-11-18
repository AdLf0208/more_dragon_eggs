# More Dragon Eggs — port notes

Ported from **More Dragon Eggs 5.0** (mod id `moredragoneggs`, `Darkere/moredragoneggs`, commit
`450678c`, Minecraft 1.21.1 NeoForge). The port targets Horizon 26.3 at the mod SDK
**`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`,
`api = "0.2"`, package format 6). Clean-room Rust rewrite from the mod's behaviour; what the mod
does and its evidence are in [`FINDINGS.md`](FINDINGS.md).

**Licence: MIT** — the jar's own `META-INF/neoforge.mods.toml` declares `license="MIT"` and
`authors="Darkere"` (as read from `moredragoneggs-5.0.jar`, 6,389 B, sha256
`d70d9560…73d6bf`); the analysed repository commit declares `mod_license=MIT` in
`gradle.properties` and ships no LICENSE file. `precheck-mod.py` reads the same value
(`licence=MIT rank=0 (permissive)`). **The 0.1.0 note below said "all rights reserved" because the
repository has no LICENSE file; the artefact wins — the jar is the licence (ARCHITECTURE.md §8,
"the jar wins over every catalogue").** Rank 0, permissive; the port stays **local-only until the
maintainer approves publishing**. `LICENSE` (MIT) and `LICENSE-NOTICE.md` travel inside the
package.

**Format: the SDK package (2026-11-18).** This port was migrated from the retired `.hnmod`
container to the flat SDK package — see [`## Migration to the SDK format`](#migration-to-the-sdk-format).
`src/runtime.rs` is **byte-identical** to the pre-migration `mixins/moredragoneggs/runtime.rs`
(sha256 `4e4c896c720a63ea89188c04a6ced3f5a383ae8b9733c6424f7e98c5365c2f59`), including the file
inside the built package.

Status: `ported-partial` for 0.2.0. The behaviour is complete and unit-verified (21 tests), the
package passes the registry `check` and `fits`; **the in-game e2e has not been re-run at this
pin** because it needs a launcher-built game and `horizon-launcher` is not installed here (see
[Gaps](#gaps)). The 0.1.0 in-game run recorded at the bottom is historical evidence for the
behaviour, not for the 0.2.0 package.

## Approach (from the pin's `docs/modpacks.md`)

- **Class `content`, tier `logic`**, core, P2 (`ports/_shared/source-map/atm10-sources.csv`;
  `ports/ledger.csv`). The jar holds three classes, one mixin config and a `pack.mcmeta` — no
  blocks, items, models, recipes or tags, so **nothing is carried**; the egg and the wall head
  are vanilla 26.3 states.
- **Carry-over vs agent port:** the agent port is the whole job (there is nothing to carry).
- **Why now:** rank 0, core, mechanics, and a behaviour the engine itself cannot express: the
  vanilla fight places its egg on the first kill only, and this is the one-mod-per-line addition
  that makes later kills do the same.

## Behaviour (upstream, file:line)

- `DragonEggMixin.java:32-38` injects at HEAD of `setDragonKilled`, i.e. **before** vanilla sets
  `previouslyKilled`: it computes
  `pos = getHeightmapPos(MOTION_BLOCKING, EndPodiumFeature.getLocation(origin))` (`:34`), and
  **if `previouslyKilled`** (`:35`) sets a dragon egg there (`:36`). So vanilla's first-kill egg
  stays vanilla's; this mod adds the egg on every later kill (at the same block, replacing an
  uncollected one).
- `:38` always calls `MoreDragonEggs.PlaceDragonHead(pos, level)`.
- `MoreDragonEggs.java:26-27` gates the head on `spawnHead`; `:29-31` walks down from that
  heightmap cell to the podium's bedrock; `:33-39` takes the nearest player's horizontal direction
  (`Direction.getNearest`, north when none); `:41-57` tries four clockwise directions, skipping one
  bedrock block, and places `minecraft:dragon_wall_head[facing=<dir>]` in the first air cell.
- `ServerConfig.java:13` — one key `spawnHead`, default **false**.
- Full analysis, incl. the 6.0 tip comparison: [`FINDINGS.md`](FINDINGS.md).

## Hooks (0.2.0)

| Moment | SDK method | What the glue does |
|---|---|---|
| A config is registered / read / changed / dropped | `Hooks::register_configs`, `config_loading`, `config_reloading`, `config_unloading` | Declares `moredragoneggs.conf` as a `Server` config (`config/moredragoneggs.conf`, default written when missing; a world's `serverconfig/` copy is read when present, as NeoForge's `ServerConfig` is), and keeps `spawn_head` from the text. |
| A world opened | `Hooks::server_started` | Resets the per-world watch (the retired bundle's `WorldPhase::WorldOpenReady`; the SDK moved that phase here, `hooks/notes.rs:2770`). |
| After a level's tick pass | `Hooks::level_tick_post` (server side only) | On the End's pass: the kill edge (see below), the counter, and the two `LevelRef::set_block` writes. |

**Why the kill is not a hook.** The SDK has no dragon-death moment: `living_death` /
`living_death_post` say so themselves ("The ender dragon uses a separate death sequence and fires
neither living death hook"), and `mob_killed` is only "the acting player's melee blow killed the
mob" (its call site is `attack_mob`). `dragon_egg_form` is the fight's **first** egg
(`dragon.rs:2841-2862`, guarded by `!previously_killed`), so it cannot be this mod's trigger. What
the API does expose is the fight's own memory (`DragonRef::previously_killed` =
`EnderDragonFight.hasPreviouslyKilledDragon`) and the level's occupant list, so the glue watches
the level, which is the faithful equivalent of the injection point: Horizon's `dragon_killed`
(= `setDragonKilled`) runs only from the dragon's removal — `tick_death`'s 200th tick and `/kill`
— and the arena's chunk ticket is held whenever a player watches the fight (`Fight::arena_watched`,
`dragon.rs:150-158`).

This is recorded as a **deviation with its evidence**, not a gap: the behaviour (an egg on every
kill after the first, the head on every kill) is complete.

## Runtime (`runtime.rs`, 13 tests)

Pure and unit-tested, unchanged from 0.1.0: config parsing/formatting (`spawn_head` and upstream's
`spawnHead`, default false), the TSV kill counter (`kills\t<n>`), the first-kill/no-egg rule
(`places_egg`), the world-seeding rule (`adopt`), the direction math (`Direction.getNearest` with
enum-order ties and north fallback, `getClockWise`), the head's descent to bedrock, and the
four-try head planner (air, skip one bedrock, blocked → clockwise, none). The glue only reads the
level and writes two blocks.

Glue tests live in `src/support.rs` (8): the first sight seeds the counter, a world whose first
kill predates the port seeds one, a fight that cannot answer still counts as presence, a dragon
that stays is not a kill, a dragon that leaves under a player is exactly one kill, a dragon that
leaves with nobody watching is not a kill, a dragon seen before the level emptied is not one
either, and a respawned dragon arms the next kill.

## State and config

- `config/moredragoneggs.conf` — registered as a `Server` config, so the game writes it with
  `spawn_head = false` when it is missing and reads it again for each world (a world's
  `serverconfig/moredragoneggs.conf`, when someone made one, wins for that world — as NeoForge's
  `ServerConfig` behaves). `spawnHead = true` (upstream's spelling) is accepted too.
- `<world>/horizon-mod-data/moredragoneggs/state.tsv` — one row `kills\t<N>`, written through the
  SDK's `world_data_files()` (atomic replace, copied and backed up with the save). It is re-read
  before each kill, so it is the source of truth (and the e2e can reset it); a missing or corrupt
  file counts as zero. The first time the port sees a world's dragon, it seeds the counter from
  the fight's `previously_killed` (`DragonRef::previously_killed`), so a world whose first kill
  predates the port still gets eggs.
- The counter has moved from 0.1.0's `<world>/moredragoneggs/state.tsv` to the SDK world-data
  folder. Same format, same semantics; **a world migrated from 0.1.0 starts its counter from the
  fight's own flag** (which is exactly right: `previously_killed` is true iff a dragon died there).

## Deviations (deliberate)

1. **The kill edge is the level's observation, not `Fight.killed`.** 0.1.0 read the pin-internal
   `Game.fight.killed` false → true edge. The SDK may not reach into the game, so the edge is "a
   dragon stood in the End at the last pass and is gone at this one, with a player still in the
   level". Same moment (the dragon's removal is where `dragon_killed` runs), same outcome; a
   level closing with nobody in it is explicitly not a kill.
2. **The counters are correlated per world open, not per process.** `server_started` resets the
   watch; the file itself is the memory, so switching worlds never carries a kill across.
3. **The head's candidate players are the End's own list** (`LevelRef::online_players`), where
   0.1.0 also considered the window's own body unconditionally. In single player the body is in
   that list, so the nearest-player answer is unchanged; in a dedicated server it is the same set.
4. **The egg cell is the engine's `MOTION_BLOCKING` height** (`surface_heights`), which is
   upstream's `getHeightmapPos(MOTION_BLOCKING, …)`; it is not rebuilt from
   `full_collision || legacy_solid` (0.1.0's first version). The engine also counts thin blocks
   such as lanterns; the e2e's lantern differential asserts this.
5. **No logging.** 0.1.0 wrote `log::info!/warn!` lines for the config, the world and each kill.
   A native mod may not add a crate and the SDK exports no logger, so those diagnostics are
   dropped (recorded under Gaps).
6. **The head is placed in the same pass** as the egg rather than via upstream's 0-tick
   `TickTask`; the cell and the facing are the same.
7. **A `/summon`ed dragon counts as a kill** where the engine's `dragon_killed` would not run for
   it (`is_fight_dragon`, `dragon.rs:4307-4315`): the SDK names no "is this the fight's dragon"
   predicate. See Gaps; the vanilla fight's own dragon and its respawns are exact.

## Gaps

| Gap | What it costs | Shape of the fix |
|---|---|---|
| **No in-game run for 0.2.0.** The scenario needs a launcher-built game (`horizon-launcher native-build . --trust-native-code`) and the launcher is not installed here. | The package's behaviour is verified by 21 unit tests and the registry checks only; the 0.1.0 in-game run below is historical. | Run `tools/e2e.sh` with `GAME=` (+ `CONTROL_GAME=`) and record the transcript before claiming `ported`. |
| **No logger.** Config, world and kill lines of 0.1.0 have no route to the player. | A misconfigured file is silent; the counter is the only observable state. | Read the state file, or a future SDK logging API. |
| **`acquire`-level error detail is invisible.** `set_block` returning `Ok(false)`/`Err` is ignored. | A refused write (unloaded chunk, out-of-height) leaves no egg and no message. | The same: a logger, or a readout command. |
| **The counter's write is best-effort.** A failed `world_data_files().write` is not retried. | A world whose data folder is read-only counts kills but loses them on restart. | Retry at the next kill; the fight's own flag still seeds a fresh world correctly. |
| **Config reload is file-driven only.** `config_reloading` fires when the game polls the file (once a second); there is no `/reload` hook for a mod's own file. | Upstream reloads on a datapack reload; here a second's delay is possible. | Nothing to do; it is the SDK's documented behaviour. |
| **`spawn_head` cannot be toggled per player.** It is a server config, as upstream's `ServerConfig` is. | — | Not a gap; noted so it is not "fixed" into a client option. |
| **A dragon summoned with `/summon` in the End counts as a kill.** The engine ends the fight only for its own dragon (`is_fight_dragon` compares the live dragon's uuid with `fight.dragon_uuid`, `dragon.rs:4307-4315`), and 0.1.0's `Fight.killed` read inherited that precision. No SDK method names the fight's dragon: `DragonRef::previously_killed` / `alive_crystals` answer for the *fight* whenever it is loaded, so a summoned dragon's removal looks the same. | A player who summons and kills a dragon gets an extra egg (and +1 on the counter); the real first kill then also gets the port's egg on the same cell vanilla uses, so play is unaffected. | "A `DragonRef` predicate that answers whether this dragon is the fight's own" — until then the deviation is recorded, not faked. |

## Upstream defects fixed

None found in this pass. The artefact is internally consistent: the mixin's injection point
(`setDragonKilled` HEAD) matches the flag it tests (`previouslyKilled`, still false on the first
kill), the config key it reads (`ServerConfig.spawnHead`) is the one the class uses, and the block
it places (`dragon_wall_head[facing=…]`) has exactly the four horizontal `facing` values
`Direction.getNearest` can produce. Nothing was rebalanced or dropped.

## Migration to the SDK format (0.2.0, 2026-11-18)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70) retired `.hnmod`: a native mod is now one flat
`.hmod` — `horizon-mod.toml` (`api = "0.2"`) + `Cargo.toml` + `src/lib.rs` with one
`#[mod_api::horizon_mod] impl Hooks` + `assets/`/`data/` when the mod carries content. **Version
0.2.0 is the same behaviour as an SDK package, and this is a first publication** (there is no
closed submission to supersede).

Pins: SDK nightly `0b14f8acfd97fa2c750e42795249dcc981178ca4` (2026-11-18, package format 6,
`content_api` "0.2"), mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`; `cargo-horizon-mod` from
`horizon-rust/horizon@main`. The pinned SDK is cached in-tree at `../_shared/sdk/0b14f8a/` and
fetched into `.horizon/sdk/` (gitignored) by the build.

### File map (0.1.0 → 0.2.0)

| 0.1.0 path | Now | Evidence |
|---|---|---|
| `mixins/moredragoneggs/runtime.rs` | `src/runtime.rs`, **copied with `cp`, `cmp`-checked** | `retired-hnmod-0.1.0/mixins/moredragoneggs/runtime.rs` |
| `mixins/moredragoneggs/support.rs` (glue) | `src/support.rs` (the same responsibilities, typed: config, kill watch, head planner, per-world counter) + `src/lib.rs` (the one `Hooks`) | `retired-hnmod-0.1.0/mixins/moredragoneggs/support.rs` |
| `mixins/moredragoneggs/{about,world,level_tick}.rs` (one-line helpers) | gone: one `Hooks` method each, no helper files | same folder |
| `mixins/moredragoneggs/horizon-mixin.toml` (2 `[[module]]`, 3 `[[injection]]`) | no plan: the manifest's `[native]` pins and typed methods | same folder |
| `compatibility.json`, `rust-toolchain.toml` | `[native]` in `horizon-mod.toml`; the pinned commit decides the toolchain | `retired-hnmod-0.1.0/` |
| `tools/horizon.py`, `tools/package.py`, `tools/audit/`, `tools/__pycache__`, `Cargo.toml` (old), `AGENTS.md` (old), `README.md` (old), `Cargo.lock` | deleted/moved: staging, `.hnmod` packaging and the source-audit crate belong to the retired format; the lock is never packed | `retired-hnmod-0.1.0/` (the lock is **not** kept) |
| — | `horizon-mod.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `.gitignore` (`/target`, `/.horizon`, `/Cargo.lock`) | new |

**Kept unchanged:** `tools/e2e.sh` (the scenario; two launch-line edits and the SDK paths, see the
traps), `notes.md`, `FINDINGS.md`, `REPIN-AUDIT.md`.

### Helper → hook table (every retired injection and module)

| 0.1.0 injection (hook, API) | 0.2.0 home |
|---|---|
| `about-to-start` (`server_about_to_start` v1, HEAD; `about.rs`) — read/create `config/moredragoneggs.conf` with `Game::native_config_dir()` + `std::fs`, `log::info!` | `Hooks::register_configs` + `config_loading` + `config_reloading` + `config_unloading` → `support::{Core::config_file, config_loaded, config_unloaded}`. The SDK's own NeoForge equivalent (`registerConfig` / `ModConfigEvent.*`) writes the default file, reads a world's `serverconfig/` copy, and re-reads on change. |
| `world-lifecycle` (`world_lifecycle` v1, HEAD; `world.rs`) — remember the world dir, read `<world>/moredragoneggs/state.tsv`, reset the caches | `Hooks::server_started` → `support::Core::new_world` (the SDK moved `WorldPhase::WorldOpenReady` to `server_started`); the counter is read on demand through `world_data_files()`, so no path has to be remembered. |
| `end-level-tick` (`level_tick_post` v1, HEAD; `level_tick.rs`) — the `Fight.killed` edge, the counter, the egg and the head | `Hooks::level_tick_post` (server side) → `support::Core::pass` (reads, under the lock) + `support::place` (the two writes, after it is dropped). |
| `[[module]] runtime` (`runtime.rs`, injected as `crate::game::moredragoneggs`) | `src/runtime.rs`, an ordinary `pub mod runtime;` — byte-identical. |
| `[[module]] state` (`support.rs`, injected as `crate::game::moredragoneggs_state`) | `src/support.rs`, a private module of this crate: no `//!`-free constraint, no game module. |

**Pin-internal reads and the SDK home each one got** (the wire that had to be re-routed):

| 0.1.0 (pin internals) | 0.2.0 (typed API) |
|---|---|
| `game.fight.killed` (the false → true edge) | the level's dragon observation (`LevelRef::all_entities`, `EntityRef::kind`), see *Deviations* 1 |
| `game.fight.previously_killed` | `DragonRef::previously_killed` (the fight's `hasPreviouslyKilledDragon`) |
| `game.dragon.is_none()` (the "arena not loaded yet" guard) | the same presence check |
| `game.native_surface_heights(…, MotionBlocking)` | `WorldEditServerHost::surface_heights` |
| `game.pack.assets.registry` state lookup (`state_of`: default state + `descriptor.properties.insert`) | `BlockStatesHost::block_state_id("minecraft:dragon_wall_head[facing=south]")` — one `/setblock`-syntax string |
| `game.world.loaded(x, z)` + `game.world.block(…)` + the registry's state names | `LevelRef::block` (`None` when unloaded) + `BlockKey::id` |
| `game.native_set_block(dimension, pos, state, flags::UPDATE_ALL)` | `LevelRef::set_block(host, pos, state, BlockUpdateFlags::NEIGHBORS \| CLIENTS)` |
| `game.player` + `game.native_server_players()` (filtered to the End) | `LevelRef::online_players` (the End's own player list) |
| `Game::native_config_dir()` + `std::fs` | `register_configs` / `ConfigFiles` |
| `game.native_world_dir()` + `std::fs` + hand-rolled `state.tsv.tmp` + rename | `ConfigServerHost::world_data_files()` (`ConfigFiles::{read, write}`, atomic replace) |
| `static CORE: Mutex<Option<Core>>` in the injected module | a `Mutex<Core>` field on the mod struct (the macro boxes one `Default` instance per session; `Hooks` is `Send + Sync`) |
| `log::info!/warn!` | dropped — no logger, no crate (see Gaps) |

### The ladder (all inside one build slot, `CARGO_INCREMENTAL=0`, no `--target-dir`)

Evidence: `../_shared/evidence/moredragoneggs/sdk-*.log`.

| Step | Command | Result |
|---|---|---|
| runtime only, before any glue | `cargo test --offline` with `src/lib.rs` = `pub mod runtime;` | **13 passed**, 0 failed (`sdk-test-runtime-only.log`) |
| glue red | same, `observe` = `todo!()` | **13 passed; 8 failed**, all `not yet implemented` (`sdk-test-red-glue.log`) |
| glue green | `cargo test --offline` | **21 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| format | `cargo fmt --check` (**not** `--all`: that checks the fetched SDK) | exit 0 (`sdk-fmt.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | `SDK 0b14f8a … in ./.horizon/sdk` / `Built ./target/horizon-mod/moredragoneggs.hmod` (`sdk-build.log`) |
| registry check | `cargo horizon-mod check moredragoneggs.hmod` | `moredragoneggs 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)` / `18549 bytes, SHA-256 2438ada0…7345f3, unsigned`, **EXIT 0** (`sdk-check.log`) |
| fits the pinned build | `cargo horizon-mod fits moredragoneggs.hmod --build nightly` | `More Dragon Eggs:` / `0b14f8a  made for`, **EXIT 0** (`sdk-fits.log`) |
| reproducible | a second `rm -f Cargo.lock && cargo horizon-mod build` | same 18,549 B, same sha256, `cmp` identical (`sdk-build-second.log`, `sdk-sha256.txt`) |
| `runtime.rs` untouched | `cmp` against the retired file, and `unzip -p moredragoneggs.hmod src/runtime.rs \| sha256sum` | `4e4c896c…5c2f59` three times (`sdk-runtime-identical.txt`) |
| package contents | `unzip -l moredragoneggs.hmod` | 9 entries: `horizon-mod.toml`, `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `src/lib.rs`, `src/runtime.rs`, `src/support.rs`, `uses.json`; **no `Cargo.lock`** (`sdk-entries.txt`) |
| what it uses | `unzip -p moredragoneggs.hmod uses.json` | 119 entries: 6 hooks (+ their fields), 18 methods, 31 handles, 14 traits, 41 types, `capability:registries`, `macro:horizon_mod` (`sdk-uses-summary.txt`, `sdk-uses-from-package.json`) |

### Package

`moredragoneggs.hmod` — **18,549 B**, sha256
`2438ada0a407e713683e4d377796b066954762e0ebc64acf1046d4e2e27345f3`, 9 entries, reproducible
(two builds `cmp`-identical). `check` and `fits` both exit 0.

### Traps this migration hit (new to this port)

1. **`WorldPhase::WorldOpenReady` does not exist in the SDK.** At `0b14f8a` the enum has only
   `WorldOpenFailed`, `WorldCloseFinal` and `WorldCloseFailed` (`world/types.rs:1124-1137`), and
   the open phase moved to `Hooks::server_started` (`hooks/notes.rs:2770`: "a mod that matched
   `WorldOpenReady` in `world_lifecycle` implements `server_started` instead, fired at the same
   moment"). Porting the old `match event.phase` verbatim would compile only for the failure arms
   and reset nothing per world.
2. **There is no dragon-death hook, and the docs say so.** `living_death_post` ends with "The
   ender dragon uses a separate death sequence and fires neither living death hook"; `mob_killed`
   is the player's melee blow on a regular mob. Do not spend a run looking for the event — plan
   the level observation instead.
3. **`dragon_egg_form` is the FIRST egg only** (`dragon.rs:2847-2862`, `!previously_killed`), so it
   cannot be the mod's per-kill trigger; it is, however, the exact cell the fight uses, which is
   why `MOTION_BLOCKING` at the origin agrees with it.
4. **The kill moment is the dragon's removal, not the killing blow.** `dragon_killed` runs on
   `tick_death`'s 200th tick (`dragon.rs:2792-2800`, ten seconds after the health ran out) and
   immediately on `/kill` (`:3944-3960`). A port that keys on `DragonPhase::Dying` never sees
   `/kill` — and `/kill` is exactly how the e2e ends a fight.
5. **"The dragon is gone" is not a kill on its own.** A level closing with nobody in it removes
   the dragon too (`entity_leave_level`'s own doc lists it), so the edge needs a player in the
   level; the arena's chunk ticket is held only while someone watches (`dragon.rs:150-158`), which
   is what makes that guard sound rather than lucky.
6. **`LevelRef::all_entities` reports the dragon's eight parts as entities of the dragon's kind**,
   so the question is "was *any* entity of that kind present", never a count.
7. **`block_state_id` reads `/setblock` syntax** (`BlockStateParser.parseForBlock`,
   `host/block_states.rs:14-27`), so the head is one string
   (`minecraft:dragon_wall_head[facing=south]`) and 0.1.0's registry-descriptor plumbing is dead
   code, not a rewrite.
8. **`Config::new(..).file("moredragoneggs.conf")` is what keeps upstream's file name.** Without
   `.file(..)` the SDK writes `moredragoneggs-server.toml` and the documented config is never
   read.
9. **The per-world counter moved** from `<world>/moredragoneggs/state.tsv` to
   `<world>/horizon-mod-data/moredragoneggs/state.tsv` (`world_data_files()`). The e2e's `STATE`
   had to move with it: a stale path reads "missing", which looks exactly like "the port never
   counted".
10. **A lock guard must not span the world writes.** The reads (`all_entities`, `surface_heights`,
    `block`) run under the state lock; the two `set_block` calls run after it is dropped, because a
    world write dispatches the game's other hooks and could re-enter ours (`accelerateddecay` hit
    the same shape).
11. **The automatic remote build forward cannot run the documented command.** `build-slot.sh run`
    without `BUILD_LOCAL=1` forwarded to the second box, where the brief's `cd ports/moredragoneggs`
    fails (`cd: ports/moredragoneggs: No such file or directory`) because the runner already
    `cd`s into the synced port. Every SDK step here therefore ran with `BUILD_LOCAL=1`.
12. **`cargo test`/`clippy` re-create `Cargo.lock`.** The delete must be in the *same* shell line
    as `horizon-mod build`; a lock left from a test run is packed and changes the sha256 (the
    brief's trap 5). `build` was run twice to prove the artifact is stable.

### Changelog (for the submission PR)

> **First publication.** More Dragon Eggs 5.0 (`Darkere/moredragoneggs`, MIT) for Horizon 26.3,
> built against the mod SDK `0b14f8acfd97fa2c750e42795249dcc981178ca4` (`api = "0.2"`, mixins
> `e59da909…`). One flat `.hmod`: `horizon-mod.toml` + `Cargo.toml` + `src/lib.rs` (the one
> `impl mod_api::Hooks`) + `src/runtime.rs` (the pure rules, 13 tests) + `src/support.rs` (the
> glue, 8 tests) — no `assets/`/`data/`, because the egg and the wall head are vanilla 26.3
> states.
>
> A dragon egg appears at the End exit podium on every ender-dragon kill **after the first**
> (vanilla's first-kill egg is untouched), and with `spawn_head = true` in
> `config/moredragoneggs.conf` (default `false`) a `dragon_wall_head` is placed beside the podium
> facing the nearest player, on every kill. The egg's cell is the engine's `MOTION_BLOCKING`
> height at the podium origin, which is upstream's `getHeightmapPos(MOTION_BLOCKING, …)`.
>
> What changed for `api = "0.2"`: the `.hnmod` container and its three hook helpers became typed
> `Hooks` methods (`register_configs`, `config_loading`, `config_reloading`, `config_unloading`,
> `server_started`, `level_tick_post`); the pin-internal `Game.fight` read became the level's own
> dragon observation plus the SDK's `DragonRef::previously_killed`; block reads/writes, surface
> heights, players and the per-world counter go through the typed host API
> (`LevelRef::{block,set_block,online_players}`, `WorldEditServerHost::surface_heights`,
> `world_data_files()`); and the counter moved to
> `<world>/horizon-mod-data/moredragoneggs/state.tsv` (same `kills<TAB><n>` format).
> `src/runtime.rs` is byte-identical; the config file name and its one key are unchanged.
> Local-only until the maintainer approves publishing.

## Verification (0.2.0)

| Check | Command | Result |
|---|---|---|
| Runtime unit tests (SDK crate) | `cargo test --offline` | **21 passed**, 0 failed (13 `runtime::`, 8 `support::`) — `evidence/moredragoneggs/sdk-test.log` |
| Lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings (`sdk-clippy.log`) |
| Formatting | `cargo fmt --check` | clean (`sdk-fmt.log`) |
| Package | `rm -f Cargo.lock && cargo horizon-mod build` | `target/horizon-mod/moredragoneggs.hmod`, EXIT 0 (`sdk-build.log`) |
| Registry check | `cargo horizon-mod check moredragoneggs.hmod` | `moredragoneggs 0.2.0 (native-mod, format native-1/0b14f8a…/e59da90…, for 26.3)`, 18,549 B, sha256 `2438ada0…7345f3`, **EXIT 0** (`sdk-check.log`) |
| Fits the pinned build | `cargo horizon-mod fits moredragoneggs.hmod --build nightly` | `0b14f8a  made for`, EXIT 0 (`sdk-fits.log`) |
| `runtime.rs` untouched | `cmp` + `unzip -p … src/runtime.rs \| sha256sum` | `4e4c896c…5c2f59` (`sdk-runtime-identical.txt`) |
| Skills line | `python3 ports/tools/check-skills.py --notes ports/moredragoneggs --strict` | PASS |

### Not re-run in this migration

- **The in-game e2e** (`tools/e2e.sh`, `MODE=default|head`, the state-reset control, the optional
  unmodified-game control leg) — it needs a game the launcher built
  (`horizon-launcher native-build . --trust-native-code`) and `horizon-launcher` is not installed
  in this environment. The retired script's 0.1.0 run below stands as the behaviour's evidence; the
  0.2.0 transport (typed host reads and writes) is what changed, and the rules are byte-identical
  and tested.
- **The staged ladder** (`tools/horizon.py stage/check/test/build`) — those tools are deleted with
  the retired format.

## 0.1.0 verification (historical, `.hnmod`, pin `c9d474d` / `83ad384f`)

Run 2026-11-04 against pin `c9d474dd9116e84c243f8992d75c11314df56f06`
(`nightly-source/2026-11-01-c9d474d`), horizon-mixins
`d9c83d7096b7ea5b83678da2efdfd6161f74997e`, rustc 1.95.0. Every command ran under
`flock /tmp/opencode/campaign-build.lock` with `CARGO_INCREMENTAL=0`.

### 1. Standalone crate — pass

```
$ cargo fmt --all --check                 # exit 0
$ cargo test --locked --offline           # 13 passed; 0 failed
$ cargo clippy --workspace --all-targets --locked --offline -- -D warnings   # exit 0
```

### 2. Staged ladder — pass, 0 warnings

```
$ python3 tools/horizon.py stage --horizon <campaign>/horizon   # stage_changed: true, 63.3 s
$ python3 tools/horizon.py check                                # cargo_exit: 0, 0 warnings
$ python3 tools/horizon.py test                                 # 13 passed; 531 filtered out
$ python3 tools/horizon.py build                                # cargo_exit: 0
$ python3 tools/package.py
  target/moredragoneggs-0.1.0.hnmod, 10,204 B, sha256 69326ecd…3583f
```

### 3. In-game e2e — pass, both modes (`tools/e2e.sh`, exit 0)

The scenario kills the dragon with `/kill @e[type=minecraft:ender_dragon]` (the pin's
`kill_dragon` → `setDragonKilled`), removes the vanilla first egg, respawns the dragon with four
summoned end crystals plus one real right-click on an obsidian post, and kills again.

| Phase | `kills` in state.tsv | egg at (0,66,0) | wall heads | verdict |
|---|---|---|---|---|
| kill 1 (default) | 1 | vanilla egg (yes) | 0 | no port egg, no head |
| kill 2 (default) | 2 | **port egg (yes)** | 0 | port adds what vanilla no longer does |
| kill 3, state reset to 0 (default) | 1 | **none** | 0 | control: no counter → no egg |
| kill 1 (head) | 1 | vanilla egg (yes) | **1** at (0,65,1) `facing=south` | head on the first kill, player direction |
| kill 2 (head) | 2 | port egg (yes) | 1 | egg + head |
| kill 3, state reset to 0 (head) | 1 | **none** | 1 | counter still drives the egg |

Why it can fail: the second-kill egg is not vanilla's (vanilla places no egg after the first
kill; the first egg was removed), and the state-reset phase turns the port's own egg off by
resetting the file — an implementation that ignored the counter, or one that never placed the
block, fails one of the two assertions. The first-kill *vanilla* egg cannot discriminate the port
(both write the same cell), so the run removes it before the differential.

### 4. Harness findings (0.1.0, kept for the next fight/dragon port)

- `ctl state`'s `fight` section is the **window client's** copy; in single player the fight is
  simulated on the integrated server thread, so `dragon`, `killed` and `exit_portal` read `null`
  there even while the fight runs. Use `execute if entity @e[type=…]`, `/kill` feedback and
  `ctl blocks`.
- `/kill @e[type=minecraft:ender_dragon]` reaches the fight's `setDragonKilled` immediately (no
  death animation), exactly the upstream injection point.
- The dragon respawn ritual works headless: `/summon minecraft:end_crystal` at
  `(.5, origin+1, ±3.5)` / `(±2.5, origin+1, .5)`, then one real right-click with an end-crystal
  stack on any bedrock/obsidian block with air above. The respawn completes in ~520–600 ticks.
- `ctl give` + `ctl use-item` place the end crystal even when `ctl select minecraft:end_crystal`
  reports "no minecraft:end_crystal in the inventory".

## Files

```
horizon-mod.toml          the only manifest: id, version 0.2.0, api 0.2, side both, [native] pins
Cargo.toml                one crate; the only dependency is horizon-mod-api from .horizon/sdk/
src/lib.rs                the one #[mod_api::horizon_mod] impl Hooks: 6 methods
src/support.rs            the glue + 8 tests
src/runtime.rs            the pure engine + 13 tests, byte-identical to the retired bundle
LICENSE, LICENSE-NOTICE.md, README.md, AGENTS.md, .gitignore
FINDINGS.md, REPIN-AUDIT.md, notes.md
tools/e2e.sh              fail-capable scenario (GAME=, MODE=default|head, SELFTEST=1,
                          CONTROL_GAME=); not run for 0.2.0
moredragoneggs.hmod       the deliverable: 18,549 B, sha256 2438ada0…7345f3, 9 entries
```

No `assets/` or `data/`: the egg and the wall head are vanilla 26.3 states.

## Skills / references used

- Superpowers: `using-superpowers` (gate), `brainstorming` (choosing the kill edge before writing
  it), `writing-plans` (the slice order: move → carry → red → green → ladder), `test-driven-development`
  (the red `todo!()` run above), `systematic-debugging` (reading `dragon.rs` to find where
  `setDragonKilled` actually runs on both kill paths), `verification-before-completion` (every
  claim in this file is a quoted command and its output).
- Beautiful Rust `rust-best-practices` (expressive types: `Edge`/`Fight` instead of booleans,
  `Option` for "the fight cannot answer", a small `Plan` resolved under the lock; no `unwrap` in
  glue; `cargo fmt --check` + `clippy -D warnings`), `rust-port` (reading the Java behaviour
  rather than its structure).

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices, rust-port
