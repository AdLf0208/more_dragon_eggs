//! More Dragon Eggs for Horizon — the one `impl mod_api::Hooks` at the crate root.
//!
//! Vanilla places a dragon egg on the first ender-dragon kill only; this mod places one at the
//! podium on **every** kill after the first, and — with `spawn_head = true` — a dragon wall head
//! beside the podium facing the nearest player. Ported from More Dragon Eggs 5.0
//! (`Darkere/moredragoneggs`, commit `450678c`, MIT); `FINDINGS.md` has the behaviour read from the
//! jar and the repository, `notes.md` the hook map, the gaps and the migration to the SDK format.
//!
//! The moments, all in [`support`]: the config is a registered `Server` config; the kill is the
//! End-pass edge the level itself shows (a dragon under a watching player is gone); the two blocks
//! are vanilla 26.3 states written when the kill is counted.
//!
//! The rules and their numbers live in [`runtime`], which has no `mod_api` type and is unit-tested
//! by `cargo test` alone — byte-identical to the retired bundle's `runtime.rs`.
pub mod runtime;
mod support;

use std::sync::{Mutex, MutexGuard};

use mod_api::{
    self, ConfigLoadingEvent, ConfigReloadingEvent, ConfigUnloadingEvent, Hooks, HostSide,
    LevelTickPostEvent, RegisterConfigsEvent, ServerHost, ServerStartedEvent,
};

#[derive(Default)]
struct Mod {
    core: Mutex<support::Core>,
}

impl Mod {
    fn core(&self) -> MutexGuard<'_, support::Core> {
        self.core
            .lock()
            .expect("moredragoneggs state lock is held for one short step")
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// The config file upstream registers with NeoForge (`ServerConfig`): written when missing, and
    /// read again for each world.
    fn register_configs(&self, _host: HostSide<'_>, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(support::Core::config_file());
    }

    /// A registered config was read: keep `spawn_head`.
    fn config_loading(&self, _host: HostSide<'_>, event: &mut ConfigLoadingEvent<'_>) {
        self.core().config_loaded(event.config);
    }

    /// A loaded config's file changed on disk: re-read it, as upstream's `ModConfigEvent.Reloading`.
    fn config_reloading(&self, _host: HostSide<'_>, event: &mut ConfigReloadingEvent<'_>) {
        self.core().config_loaded(event.config);
    }

    /// A `Server` config was dropped as its server stopped.
    fn config_unloading(&self, _host: &mut dyn ServerHost, event: &mut ConfigUnloadingEvent<'_>) {
        self.core().config_unloaded(event.config);
    }

    /// A world opened (the phase the retired bundle read as `WorldPhase::WorldOpenReady`): the
    /// watch and the counter start over.
    fn server_started(&self, _host: &mut dyn ServerHost, _event: &mut ServerStartedEvent) {
        self.core().new_world();
    }

    /// After a level's tick pass: the End's pass is where a completed kill is seen and counted, and
    /// where the egg and the head are written. Reads happen under the state lock, the two writes
    /// after it is dropped.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let plan = {
            let mut core = self.core();
            core.pass(&mut *host, event.level)
        };
        if let Some(plan) = plan {
            support::place(&mut *host, event.level, plan);
        }
    }
}
