# More Dragon Eggs — a Horizon port

**More Dragon Eggs** is a Minecraft mod. Small minecraft mod that allows for more dragon eggs. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [Darkere/moredragoneggs](https://github.com/Darkere/moredragoneggs) — licence **MIT**.

| | |
|---|---|
| Upstream mod | More Dragon Eggs ([source](https://github.com/Darkere/moredragoneggs)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `moredragoneggs.hmod` (install it with the Horizon launcher) |
| Licence | **MIT** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- `DragonEggMixin.java:32-38` injects at HEAD of `setDragonKilled`, i.e. **before** vanilla sets
- `:38` always calls `MoreDragonEggs.PlaceDragonHead(pos, level)`.
- `MoreDragonEggs.java:26-27` gates the head on `spawnHead`; `:29-31` walks down from that
- `ServerConfig.java:13` — one key `spawnHead`, default **false**.
- Full analysis, incl. the 6.0 tip comparison: `FINDINGS.md`.

## Not yet ported

No omissions are recorded for this slice; `notes.md` is the authoritative list.

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `moredragoneggs.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

More Dragon Eggs was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MIT**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/Darkere/moredragoneggs

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run moredragoneggs` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/moredragoneggs/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
