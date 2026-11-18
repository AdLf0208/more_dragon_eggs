//! Game glue for More Dragon Eggs: the config, the per-world kill watch and the two blocks.
//!
//! Upstream injects at the head of `EnderDragonFight.setDragonKilled` (`DragonEggMixin.java:32-38`),
//! where vanilla's `previouslyKilled` flag is still false on the first kill: it places an egg when
//! the flag is true (every kill after the first) and always calls `PlaceDragonHead`.
//!
//! The SDK has no dragon-death moment — `living_death` and `living_death_post` say so themselves
//! ("The ender dragon uses a separate death sequence and fires neither living death hook") — and
//! `dragon_egg_form` only reports the *first* egg (`dragon.rs:2841-2862` places it inside
//! `dragon_killed`, guarded by `!previously_killed`). What the level does show is the dragon
//! itself, so the kill is the edge "a dragon stood in the End at the last pass and is gone at this
//! one, with a player still in the level". Horizon ends the fight (`dragon_killed`, the engine's
//! `setDragonKilled`) only from a dragon's removal, on both paths — the death flight's 200th tick
//! and `/kill` — and the arena's chunk ticket is held while a player watches (`Fight::arena_watched`,
//! `dragon.rs:150-158`), so a dragon that disappears under a watching player was killed. A level
//! that closes with nobody in it removes the dragon too, which is why a pass with no player is not
//! a kill.
use mod_api::{
    BlockUpdateFlags, Config, ConfigFile, ConfigKind, ConfigPath, EntityRef, Heightmap, Host,
    LevelRef, PlayerRef, ServerHost,
};

use crate::runtime;

/// The mod's id: the manifest's, the config owner's and the bulk-data folder's.
const ID: &str = "moredragoneggs";
/// The ender dragon's entity type id (`EntityType.ENDER_DRAGON`).
const ENDER_DRAGON: &str = "minecraft:ender_dragon";

/// The whole session state: the config in force and the per-world watch.
#[derive(Default)]
pub struct Core {
    config: runtime::Config,
    fight: Fight,
}

/// What one End pass decides, from the level and the watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Nothing to do.
    Nothing,
    /// The first sight of this world's dragon: seed the kill counter.
    Seed { previously_killed: bool },
    /// A dragon under a watching player is gone: one kill completed.
    Killed,
}

/// The per-world watch: whether a dragon stood in the End at the last pass, and whether the kill
/// counter has been seeded for this world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fight {
    dragon: bool,
    seeded: bool,
}

impl Fight {
    /// One End pass, from what the level shows. `previously_killed` is
    /// `EnderDragonFight.hasPreviouslyKilledDragon` for a dragon in the level (`None` without one),
    /// and `players` is how many players the End holds now.
    pub fn observe(
        &mut self,
        present: bool,
        previously_killed: Option<bool>,
        players: usize,
    ) -> Edge {
        if present {
            self.dragon = true;
            if self.seeded {
                return Edge::Nothing;
            }
            self.seeded = true;
            return Edge::Seed {
                previously_killed: previously_killed.unwrap_or(false),
            };
        }
        let killed = self.dragon && players > 0;
        self.dragon = false;
        if killed { Edge::Killed } else { Edge::Nothing }
    }
}

/// The writes one completed kill makes, resolved while the state lock is held.
pub struct Plan {
    egg: Option<[i32; 3]>,
    head: Option<([i32; 3], &'static str)>,
}

impl Core {
    /// The config file the game writes when it is missing (`Server` kind: `config/moredragoneggs.conf`,
    /// or a world's `serverconfig/` copy, as NeoForge's `ServerConfig` is read).
    pub fn config_file() -> Config {
        Config::new(
            ID,
            ConfigKind::Server,
            &runtime::format_config(&runtime::Config::default()),
        )
        .file(runtime::CONFIG_FILE)
    }

    /// A registered config was read. Every mod hears every config, so the owner is checked.
    pub fn config_loaded(&mut self, file: &ConfigFile) {
        if file.mod_id != ID {
            return;
        }
        self.config = runtime::parse_config(&file.text);
    }

    /// A `Server` config was dropped as its server stopped: the next world reads it again.
    pub fn config_unloaded(&mut self, file: &ConfigFile) {
        if file.mod_id == ID {
            self.config = runtime::Config::default();
        }
    }

    /// A world opened: the watch starts over (a new dragon, a new counter).
    pub fn new_world(&mut self) {
        self.fight = Fight::default();
    }

    /// One End pass: seed the counter on the first sight of the dragon, and on the kill edge resolve
    /// the writes. Reads only; the caller drops the lock before [`place`].
    pub fn pass(&mut self, host: &mut dyn ServerHost, level: LevelRef) -> Option<Plan> {
        let dimension = level.dimension().id(&*host)?;
        if dimension != runtime::THE_END {
            return None;
        }
        let players = level.online_players(&*host).unwrap_or_default();
        let (present, previously_killed) = scan(host, level)?;
        match self
            .fight
            .observe(present, previously_killed, players.len())
        {
            Edge::Nothing => None,
            Edge::Seed { previously_killed } => {
                let count = read_store(host).unwrap_or_else(|| runtime::adopt(previously_killed));
                write_store(host, count);
                None
            }
            Edge::Killed => self.kill(host, level, &dimension, &players),
        }
    }

    /// A kill completed: count it, then plan the egg and, when configured, the wall head.
    fn kill(
        &mut self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        dimension: &str,
        players: &[PlayerRef],
    ) -> Option<Plan> {
        // The file stays the source of truth: it is re-read here, so a counter reset between
        // kills is honoured, and a missing file counts as zero.
        let count = read_store(host).unwrap_or(0);
        let egg = runtime::places_egg(count);
        write_store(host, runtime::after_kill(count));
        let top = podium_top(host, dimension)?;
        let head = if self.config.spawn_head {
            head_spot(host, level, players, top)
        } else {
            None
        };
        if !egg && head.is_none() {
            return None;
        }
        Some(Plan {
            egg: egg.then_some(top),
            head,
        })
    }
}

/// Writes a resolved plan with `setBlockAndUpdate`'s flags (`UPDATE_ALL`).
pub fn place(host: &mut dyn ServerHost, level: LevelRef, plan: Plan) {
    let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
    if let Some(pos) = plan.egg
        && let Some(state) = host.block_state_id(runtime::DRAGON_EGG)
    {
        let _ = level.set_block(&mut *host, pos, state, flags);
    }
    if let Some((pos, facing)) = plan.head {
        let name = format!("{}[facing={facing}]", runtime::DRAGON_WALL_HEAD);
        if let Some(state) = host.block_state_id(&name) {
            let _ = level.set_block(&mut *host, pos, state, flags);
        }
    }
}

/// Whether a dragon stands in the level, and what the fight remembers about its previous kills.
/// `None` when the level's entity list cannot be read (a stale handle), so a pass never invents a
/// kill from a failed read.
fn scan(host: &dyn ServerHost, level: LevelRef) -> Option<(bool, Option<bool>)> {
    let entities = level.all_entities(host)?;
    let mut present = false;
    let mut previously_killed = None;
    for entity in entities {
        if !is_dragon(host, entity) {
            continue;
        }
        present = true;
        if let Some(flag) = host
            .dragon(entity)
            .and_then(|dragon| dragon.previously_killed(host))
        {
            previously_killed = Some(flag);
        }
    }
    Some((present, previously_killed))
}

fn is_dragon(host: &dyn Host, entity: EntityRef) -> bool {
    entity
        .kind(host)
        .is_some_and(|kind| kind.id(host) == ENDER_DRAGON)
}

/// `getHeightmapPos(MOTION_BLOCKING, EndPodiumFeature.getLocation(origin))`: the engine's own
/// motion-blocking column height at the End's origin (x=z=0), where the podium and the exit portal
/// are, plus nothing (the engine already answers the first free cell above the column).
fn podium_top(host: &mut dyn ServerHost, dimension: &str) -> Option<[i32; 3]> {
    let heights = host
        .surface_heights(dimension, &[(0, 0)], Heightmap::MotionBlocking)
        .ok()?;
    let height = heights.into_iter().next().flatten()?;
    Some([0, height, 0])
}

/// `MoreDragonEggs.PlaceDragonHead`: the nearest player sets the facing, then up to four clockwise
/// tries find an air cell beside the podium's bedrock.
fn head_spot(
    host: &dyn Host,
    level: LevelRef,
    players: &[PlayerRef],
    top: [i32; 3],
) -> Option<([i32; 3], &'static str)> {
    let base = runtime::bedrock_base(top, |pos| {
        block_name(host, level, pos).is_some_and(|name| name == runtime::BEDROCK)
    });
    let (dx, dz) = match nearest_player(host, players, base) {
        Some([x, _, z]) => (x as i32, z as i32),
        None => (0, 0),
    };
    let direction = runtime::facing(dx, dz);
    runtime::head_spot(base, direction, |pos| cell(host, level, pos))
}

/// `Level.getNearestPlayer(base, -1, null)` among the End's players; upstream keeps the horizontal
/// offsets from the podium, which stands at the origin, so the player's block coordinates are them.
fn nearest_player(host: &dyn Host, players: &[PlayerRef], base: [i32; 3]) -> Option<[f64; 3]> {
    let base = [f64::from(base[0]), f64::from(base[1]), f64::from(base[2])];
    let mut best: Option<(f64, [f64; 3])> = None;
    for player in players {
        let Some(position) = player.entity().position(host) else {
            continue;
        };
        let distance = (position[0] - base[0]).powi(2)
            + (position[1] - base[1]).powi(2)
            + (position[2] - base[2]).powi(2);
        if best.is_none_or(|(closest, _)| distance < closest) {
            best = Some((distance, position));
        }
    }
    best.map(|(_, position)| position)
}

/// The block at `pos` in the level's own read, by registry name.
fn block_name(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|block| block.id(host))
}

fn cell(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> runtime::Cell {
    match block_name(host, level, pos).as_deref() {
        Some(runtime::BEDROCK) => runtime::Cell::Bedrock,
        Some(runtime::AIR) => runtime::Cell::Air,
        _ => runtime::Cell::Other,
    }
}

/// The world's kill counter, `<world>/horizon-mod-data/moredragoneggs/state.tsv`: the SDK's own
/// per-world data folder, so a world's backups carry it.
fn store_path() -> Option<ConfigPath> {
    ConfigPath::new(runtime::STORE_FILE).ok()
}

fn read_store(host: &dyn ServerHost) -> Option<u32> {
    let path = store_path()?;
    let bytes = host.world_data_files().read(&path).ok()?;
    runtime::parse_store(&String::from_utf8_lossy(&bytes))
}

fn write_store(host: &dyn ServerHost, count: u32) {
    let Some(path) = store_path() else {
        return;
    };
    let _ = host
        .world_data_files()
        .write(&path, runtime::format_store(count).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{Edge, Fight};

    #[test]
    fn the_first_sight_seeds_the_counter() {
        let mut fight = Fight::default();
        assert_eq!(
            fight.observe(true, Some(false), 1),
            Edge::Seed {
                previously_killed: false
            }
        );
    }

    #[test]
    fn a_world_whose_first_kill_predates_the_port_seeds_one() {
        let mut fight = Fight::default();
        assert_eq!(
            fight.observe(true, Some(true), 1),
            Edge::Seed {
                previously_killed: true
            }
        );
    }

    #[test]
    fn a_fight_that_cannot_answer_still_counts_as_presence() {
        let mut fight = Fight::default();
        assert_eq!(
            fight.observe(true, None, 1),
            Edge::Seed {
                previously_killed: false
            }
        );
        assert_eq!(fight.observe(true, None, 1), Edge::Nothing);
        assert_eq!(fight.observe(false, None, 1), Edge::Killed);
    }

    #[test]
    fn a_dragon_that_stays_is_not_a_kill() {
        let mut fight = Fight::default();
        let _ = fight.observe(true, Some(false), 1);
        for _ in 0..200 {
            assert_eq!(fight.observe(true, Some(false), 1), Edge::Nothing);
        }
    }

    #[test]
    fn a_dragon_that_leaves_under_a_player_is_one_kill() {
        let mut fight = Fight::default();
        let _ = fight.observe(true, Some(false), 1);
        assert_eq!(fight.observe(false, None, 1), Edge::Killed);
        // The second pass with no dragon is the same kill, not another one.
        assert_eq!(fight.observe(false, None, 1), Edge::Nothing);
    }

    #[test]
    fn a_dragon_that_leaves_with_nobody_watching_is_not_a_kill() {
        // A level closing with nobody in it removes the dragon; that is not a death.
        let mut fight = Fight::default();
        let _ = fight.observe(true, Some(false), 1);
        assert_eq!(fight.observe(false, None, 0), Edge::Nothing);
    }

    #[test]
    fn a_dragon_seen_before_the_level_emptied_is_not_a_kill_either() {
        let mut fight = Fight::default();
        let _ = fight.observe(true, Some(false), 2);
        assert_eq!(fight.observe(false, None, 1), Edge::Killed);
        assert_eq!(fight.observe(false, None, 0), Edge::Nothing);
    }

    #[test]
    fn a_respawned_dragon_arms_the_next_kill() {
        let mut fight = Fight::default();
        let _ = fight.observe(true, Some(false), 1);
        assert_eq!(fight.observe(false, None, 1), Edge::Killed);
        // The respawn: a dragon again, and the counter is seeded already.
        assert_eq!(fight.observe(true, Some(true), 1), Edge::Nothing);
        assert_eq!(fight.observe(false, None, 1), Edge::Killed);
    }
}
