# Working on this Horizon mod (SDK layout, 2026-11-18)

This port is **one flat `.hmod`**: `horizon-mod.toml` + `Cargo.toml` + `src/` (the one
`impl Hooks`) — code only, no `assets/`/`data/`. The `.hnmod` container, `mixins/<id>/`, the plan
and injection schema are **retired** (Horizon's mod-API merge; see `notes.md` §"Migration to the
SDK format"). Do not recreate them.

Read `README.md` and `notes.md` first. Keep this repository separate from Horizon, horizon-mixins
and the launcher; the only things fetched are the pinned SDK (`.horizon/sdk/`, gitignored, never
packed) and the cargo cache. Do not commit binaries, the SDK or Minecraft assets.

- **The pure rules stay in `src/runtime.rs`** and must stay byte-identical to the retired file
  (`sha256 4e4c896c…5c2f59`): it has no `mod_api` type and is what `cargo test` exercises. The
  glue (hosts, events, handles, the kill watch) goes in `src/support.rs` / `src/lib.rs`, with its
  own tests there — new rules would belong in `runtime.rs`, but the migration keeps that file
  frozen.
- **Read the surface, do not guess it.** `cargo horizon-mod hooks <text>` searches the catalog
  offline; the generated `Hooks`, every event struct and every handle method are in
  `.horizon/sdk/api/crates/horizon-mod-vocab/generated/mod_api_generated{,_host}.rs`.
- **This mod has no Parts.** Both blocks are vanilla states; the moments are global hooks
  (`register_configs`, `config_loading/reloading/unloading`, `server_started`, `level_tick_post`).
- Run everything through the campaign slot registry (`../tools/build-slot.sh run --job <id>
  --port ports/moredragoneggs --step <step> -- bash -c 'cd ports/moredragoneggs && …'`), which
  blocks until a slot is free. SDK steps use `BUILD_LOCAL=1` (the automatic remote forward does
  not `cd` into the port). Never run a bare `tools/e2e.sh`, and never start a build in the
  background.
- Ladder: `cargo fmt --check` (not `--all`), `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `rm -f Cargo.lock`, `cargo horizon-mod build` (copy
  `target/horizon-mod/moredragoneggs.hmod` to the port root), `cargo horizon-mod check
  moredragoneggs.hmod`, `cargo horizon-mod fits moredragoneggs.hmod --build nightly`. Deleting
  `Cargo.lock` before the build is required: it is packed and changes the artifact's SHA-256.
- An in-game check needs a game the **launcher** built
  (`horizon-launcher native-build . --trust-native-code`); `cargo horizon-mod run` is content-only.
  `tools/e2e.sh` holds the scenario and takes `GAME=`/`CONTROL_GAME=`; it has not been run for
  0.2.0 (recorded gap).
- Keep validation claims and supported revisions honest: the SHA-256 and the `check` line in
  `notes.md` are what reviewers reproduce.
