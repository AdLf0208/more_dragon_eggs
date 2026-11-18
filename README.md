# More Dragon Eggs — Horizon port

A clean-room Rust rewrite of **More Dragon Eggs 5.0** (`Darkere/moredragoneggs`, commit
`450678c`, Minecraft 1.21.1 NeoForge) for Horizon at the SDK nightly
`0b14f8acfd97fa2c750e42795249dcc981178ca4` (26.3), mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.

One flat SDK package (mod API **0.2**) — code only, no carried content: the egg and the wall
head are vanilla 26.3 states.

## What it does

- On every ender-dragon kill **after the first**, a `minecraft:dragon_egg` appears at the End
  exit podium. The first kill stays vanilla's (vanilla already places its egg there).
- With `spawn_head = true` in `config/moredragoneggs.conf` (default `false`), a
  `minecraft:dragon_wall_head` is also placed beside the podium, one block out from its
  bedrock, facing the nearest End player (north when there is none), trying up to four
  clockwise directions for an air cell.
- The egg's cell is the engine's own `MOTION_BLOCKING` height at the podium's origin, which is
  upstream's `getHeightmapPos(MOTION_BLOCKING, …)`.
- The port keeps a per-world kill counter in
  `<world>/horizon-mod-data/moredragoneggs/state.tsv` (`kills<TAB><n>`). It is the source of
  truth for "a dragon died here before", so a world the port is added to after a first kill
  still gets eggs; a world whose counter is deleted behaves like a world with no previous kill.

**Licence: MIT** (the jar's own `neoforge.mods.toml` says `license="MIT"`, author Darkere;
the repository ships no LICENSE file). Rank 0, permissive. Local-only until the maintainer
approves publishing.

## Layout

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and identity: `id`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]`, `[native]` pinned to the exact game/horizon/mixins commits |
| `Cargo.toml` | one crate, `edition = "2024"`; its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`: the config hooks, `server_started`, `level_tick_post` |
| `src/support.rs` | the glue: the config entry, the per-world kill watch, the podium/head planning, the per-world counter; 8 unit tests |
| `src/runtime.rs` | the pure rules, byte-identical to the retired bundle's file: config, TSV counter, the first-kill rule, the direction and head planning; 13 unit tests |
| `tools/e2e.sh` | the fail-capable in-game scenario (`MODE=default\|head`) |
| `FINDINGS.md`, `REPIN-AUDIT.md` | the upstream analysis and the re-pin audit (historical) |
| `notes.md` | the hook map, deviations, gaps and the migration record |

## Build and check

```sh
cd ports/moredragoneggs
CARGO_INCREMENTAL=0 cargo test                 # 21 tests (13 runtime + 8 glue)
CARGO_INCREMENTAL=0 cargo clippy --all-targets -- -D warnings
cargo fmt --check                              # not --all: that checks the fetched SDK
rm -f Cargo.lock                               # a packed lock changes the artifact
cargo horizon-mod build                        # -> target/horizon-mod/moredragoneggs.hmod
cp target/horizon-mod/moredragoneggs.hmod .    # the deliverable lives in the port
cargo horizon-mod check moredragoneggs.hmod
cargo horizon-mod fits moredragoneggs.hmod --build nightly
```

Run the compile steps through `ports/tools/build-slot.sh` (see `notes.md`); the in-game leg
needs a **launcher-built** game (`horizon-launcher native-build . --trust-native-code`), which
this environment does not have, so `tools/e2e.sh` takes `GAME=` and has not been run for 0.2.0.
