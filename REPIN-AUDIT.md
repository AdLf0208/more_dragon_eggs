# More Dragon Eggs — re-pin audit (Horizon `83ad384f`)

Read-only audit. **No port code, content, manifest or campaign document changed.**
Nothing was built or run: every claim below is a source read of the two pins.

| | Pin | Checkout |
|---|---|---|
| New | Horizon `83ad384f48d0742746d518737511882680aab5b7`, mixins `e8cd0aa…` | `ports/_shared/pins/horizon-83ad384f` |
| Old (shipped) | Horizon `c9d474dd…` (108 hooks) | `ports/artifacts/target/mixins/original/32051c454db029bf4671` |

Sources read: `notes.md`, `FINDINGS.md`, `ARCHITECTURE.md` item 23,
`ports/_shared/repin-83ad384f.md`; then `native-hooks.json` and the Rust
sources of both pins. Paths are relative to the **new** pin unless marked
“c9d474d”.

## Summary

- **Deferred items audited: 5. Newly realizable: 2. Still blocked: 1.
  Non-issues at this pin: 2.**
- The behaviour itself was already complete; this pin closes the one recorded
  *fidelity* gap (egg position) two different ways, both exact:
  `native_dragon_egg_form` (API 208, new) reports the position where the fight
  itself places the first egg, and `native_surface_heights(..., MotionBlocking)`
  (API 127, new) is upstream’s own `getHeightmapPos(MOTION_BLOCKING, …)` rule.
- **Highest-value next step:** capture the first egg’s position from
  `dragon_egg_form` into `state.tsv` and reuse it for every later kill; fall
  back to `native_surface_heights(MotionBlocking)` when the world’s first kill
  predates the port. That removes the recomputed `full_collision || legacy_solid`
  rule (and its fluid divergence) and makes the e2e able to assert a
  fluid-topped podium.

## Deferred matrix

| # | Item (source) | Pin capability | Contract | At `c9d474d` | Verdict | Exact next step |
|---|---|---|---|---|---|---|
| 1 | Egg position uses the pin’s `full_collision \|\| legacy_solid` scan, not upstream’s `getHeightmapPos(MOTION_BLOCKING, …)`; they differ on fluids (`notes.md` §Gaps; ARCHITECTURE item 23 “heightmap rule differs on fluids”) | `native_surface_heights(dimension, columns, NativeHeightmap)` (`crates/horizon-client/src/game/native_world_columns.rs:22`) with `NativeHeightmap::MotionBlocking` (`native_world_edit.rs:7-12`) — **API 127, new** | helper | **no** | **realizable now** | Read `(0,0)` in `the_end` with `MotionBlocking` at the `level_tick_post` edge (the hook’s level is in place) and use `y`; matches upstream `DragonEggMixin:34` |
| 2 | The first egg’s exact position is recomputed every kill (`notes.md` §Gaps) | `native_dragon_egg_form(dimension, pos, cancel)` (`native_hooks.rs:1302`, dispatched at `game/dragon.rs:2489` inside `if !previously_killed`, `:2471-2496`) — **API 208, new** | cancel / listen | **no** | **realizable now** | Store `pos` in the state file on the first kill (needs `state.tsv` version bump or a 3rd column); later kills place the egg there. This is *exactly* vanilla’s first-egg cell, so no rule can diverge |
| 3 | `Fight.previously_killed` has no public API; the port reads it in-crate (`notes.md` §State; `game/dragon.rs:113`, seeded at `:1025`) | No hook or helper exposes the fight flag. `native_dragon_egg_form` implicitly reveals “first kill” only while the fight is happening | absent | absent | **still blocked as a public API** | Keep the in-crate read (campaign practice for another port’s internals) or derive it from `state.tsv` + `end_portal_active()`; do not remove the seed, or old worlds lose their first-kill memory |
| 4 | Head is placed immediately instead of via upstream’s 0-tick `TickTask` (`notes.md` §Gaps) | A scheduled block tick needs the target block to be the block scheduled (`native_tick_block` `native_hooks.rs:1687`, `native_schedule_block_tick` `:8328`, and `Ticks` only exists in `random_tick`/`tick_block`/`update_shape`) — the head is not there before placement | n/a | n/a | **not a pin gap** | Keep the deviation; there is no general task scheduler and none is needed (the cell/facing are identical) |
| 5 | Dedicated-server fights with no player in the End (`notes.md` §Gaps; ARCHITECTURE item 23) | `native_server_players` (`game/native_server.rs:204`) carries `dimension` + `position` (`NativeServerPlayer`, `:19-35`), the local player is `Game.player` | helper | yes | **non-issue** | Keep `nearest_end_player` (local player + guests in `the_end`); no change |

Supporting facts (unchanged, re-verified at this pin):

- `mob_killed` still cannot see the dragon: `native_kill_event` is dispatched
  only from the player’s melee/mace/spear paths (`game.rs:12027`,
  `game/mace.rs:607`, `game/spear.rs:226`); `dragon_killed` (`dragon.rs:2459`)
  never calls it. The new `living_death` hook (API 392) is dispatched from
  `creatures.rs:1324` and `lan_actions.rs:3861` only — not for the dragon.
  The `Fight.killed` false→true edge at `level_tick_post` stays the faithful
  equivalent of upstream’s `setDragonKilled` HEAD injection.
- The fight’s egg cell is `[0, y, 0]` with `y` from the scan at
  `dragon.rs:2478-2485`; `native_dragon_egg_form` is told that same `BlockPos`
  at `:2489`.

## The four spot checks

| Check | Result |
|---|---|
| **Block-state rotate/mirror** | Not used by this port (both blocks are vanilla states). See `../morered/REPIN-AUDIT.md`. |
| **Client world-space rendering** | Not used; no client half. |
| **Heightmap** | **The relevant unblock.** `native_surface_heights` + `MotionBlocking` is new at `83ad384f` and is upstream’s exact rule (API 127, `docs/native-mods.md:1109`). Note `native_level_height` (`native_level_refs.rs:84`) is the world’s min-Y/height, not a column heightmap. |
| **Fluid / entity hooks** | No fluid needed. Entity hooks: `dragon_egg_form` (new) is the useful one; `entity_change_block` irrelevant. |

## Corrections to the campaign re-pin note

`repin-83ad384f.md` lists “heightmap” as still absent in the “Biome read,
force-random-tick helper, client world-space box renderer …” line. Column
heightmaps are present at this pin (API 127) and were absent at `c9d474d`; the
only absent part of that line is the biome *id* read at a position and the
world-space box (which exists — see the measurements audit).

## Evidence (commands)

```
python3 -c '…json…'   # API 127/208 present in 83ad384f, absent at c9d474d
grep -rn "fn native_surface_heights" -A 20 game/native_world_columns.rs
grep -n "enum NativeHeightmap" -A 8 game/native_world_edit.rs
grep -rn "fn native_dragon_egg_form" -B 22 game/native_hooks.rs
grep -n "native_dragon_egg_form\|previously_killed" game/dragon.rs
grep -rn "native_kill_event" game.rs game/mace.rs game/spear.rs game/dragon.rs
grep -rn "native_living_death" game/creatures.rs game/lan_actions.rs game/dragon.rs
grep -rn "fn native_server_players" -A 22 game/native_server.rs
```
