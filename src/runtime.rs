// More Dragon Eggs for Horizon — the pure logic engine (no game access).
//
// Ported from More Dragon Eggs 5.0 (mod id `moredragoneggs`,
// `Darkere/moredragoneggs`, commit 450678c, Minecraft 1.21.1 NeoForge). The
// author declares MIT in `gradle.properties` but ships no LICENSE file; the
// campaign brief rules all-rights-reserved, so **this port is local-only**.
//
// Behaviour: vanilla places a dragon egg on the first dragon kill only.
// Upstream injects at the head of `EnderDragonFight.setDragonKilled`, where
// `previouslyKilled` is still false on the first kill, and:
//   * places a dragon egg at the podium's heightmap position when
//     `previouslyKilled` is true (i.e. on every kill after the first);
//   * always calls `PlaceDragonHead`, which (with `spawnHead`, default false)
//     walks down to the podium bedrock, picks the horizontal direction of the
//     nearest player (north when there is none), and places a dragon wall head
//     one block out, stepping clockwise up to four times until it finds air.
//
// This file is pure logic so it compiles and unit-tests as the crate's
// standalone library; `support.rs` is the game-side glue.

/// The server config, `config/moredragoneggs.conf`.
pub const CONFIG_FILE: &str = "moredragoneggs.conf";

/// Per-world state, `<world>/moredragoneggs/state.tsv`.
pub const STORE_DIR: &str = "moredragoneggs";
pub const STORE_FILE: &str = "state.tsv";

pub const DRAGON_EGG: &str = "minecraft:dragon_egg";
pub const DRAGON_WALL_HEAD: &str = "minecraft:dragon_wall_head";
pub const BEDROCK: &str = "minecraft:bedrock";
pub const AIR: &str = "minecraft:air";
pub const THE_END: &str = "minecraft:the_end";

/// The config upstream reads (`ServerConfig.java:8-20`): one key,
/// `spawnHead`, default false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub spawn_head: bool,
}

/// Parse `config/moredragoneggs.conf`. Upstream's NeoForge file writes
/// `spawnHead = false`; the port also accepts `spawn_head` (the brief's
/// spelling). Comments, blank lines and `[section]` headers are ignored, and
/// anything unrecognised keeps the default.
pub fn parse_config(text: &str) -> Config {
    let mut config = Config::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').to_ascii_lowercase();
        let parsed = match value.as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        };
        if matches!(key, "spawn_head" | "spawnHead")
            && let Some(on) = parsed
        {
            config.spawn_head = on;
        }
    }
    config
}

/// The file written when the config is absent: upstream's default and comment.
pub fn format_config(config: &Config) -> String {
    format!(
        "# More Dragon Eggs (server config)\n\
         # Also spawn a dragon head on the pedestal beside the egg.\n\
         spawn_head = {}\n",
        config.spawn_head
    )
}

/// The persisted kill count, or `None` when the file is not a state file.
pub fn parse_store(text: &str) -> Option<u32> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = match line.split_once(['\t', '=']) {
            Some(pair) => pair,
            None => line.split_once(char::is_whitespace)?,
        };
        if key.trim() != "kills" {
            return None;
        }
        return value.trim().parse().ok();
    }
    None
}

/// `kills\t<n>` — the state file's one row.
pub fn format_store(kills: u32) -> String {
    format!("kills\t{kills}\n")
}

/// The count after one more kill.
pub fn after_kill(previous: u32) -> u32 {
    previous.saturating_add(1)
}

/// Upstream's `if (this.previouslyKilled)` at the head of `setDragonKilled`:
/// an egg for every kill after the first.
pub fn places_egg(previous: u32) -> bool {
    previous >= 1
}

/// The counter a world starts with: upstream's `previouslyKilled` already
/// true means the first kill predates this port. A missing state file in a
/// world with no previous kill starts at zero.
pub fn adopt(previously_killed: bool) -> u32 {
    if previously_killed { 1 } else { 0 }
}

/// `Direction.getNearest((int) dx, 0, (int) dz)` (`MoreDragonEggs.java:37`):
/// the horizontal direction with the largest positive dot product, ties in
/// the enum's order (north, south, west, east), north when none is positive.
pub fn facing(dx: i32, dz: i32) -> &'static str {
    let mut best = 0;
    let mut facing = "north";
    for (name, step_x, step_z) in [
        ("north", 0, -1),
        ("south", 0, 1),
        ("west", -1, 0),
        ("east", 1, 0),
    ] {
        let dot = dx * step_x + dz * step_z;
        if dot > best {
            best = dot;
            facing = name;
        }
    }
    facing
}

/// `Direction.getClockWise()` for a horizontal direction:
/// north → east → south → west → north.
pub fn clockwise(direction: &str) -> &'static str {
    match direction {
        "north" => "east",
        "east" => "south",
        "south" => "west",
        _ => "north",
    }
}

/// `PlaceDragonHead`'s descent (`MoreDragonEggs.java:29-31`): walk down from
/// the heightmap position to the podium's bedrock (or the world floor).
pub fn bedrock_base(start: [i32; 3], is_bedrock: impl Fn([i32; 3]) -> bool) -> [i32; 3] {
    let mut pos = start;
    while pos[1] > 0 && !is_bedrock(pos) {
        pos[1] -= 1;
    }
    pos
}

/// What `head_spot` needs to know about a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Bedrock,
    Air,
    Other,
}

/// The wall head's cell and facing (`MoreDragonEggs.java:41-57`): at most four
/// tries, stepping clockwise each time. One bedrock block directly out is
/// skipped (BetterEnd-style podiums); the head goes in the first air cell.
pub fn head_spot(
    base: [i32; 3],
    direction: &'static str,
    cell: impl Fn([i32; 3]) -> Cell,
) -> Option<([i32; 3], &'static str)> {
    let mut direction = direction;
    for _ in 0..4 {
        let mut spot = step(base, direction);
        if cell(spot) == Cell::Bedrock {
            spot = step(spot, direction);
        }
        if cell(spot) == Cell::Air {
            return Some((spot, direction));
        }
        direction = clockwise(direction);
    }
    None
}

/// The horizontal neighbour of `pos` in `direction` (`BlockPos.relative`).
fn step(pos: [i32; 3], direction: &str) -> [i32; 3] {
    let (dx, dz) = match direction {
        "north" => (0, -1),
        "south" => (0, 1),
        "west" => (-1, 0),
        _ => (1, 0),
    };
    [pos[0] + dx, pos[1], pos[2] + dz]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_no_head() {
        assert!(!Config::default().spawn_head);
    }

    #[test]
    fn config_accepts_upstream_and_brief_spellings() {
        assert!(parse_config("spawnHead = true\n").spawn_head);
        assert!(parse_config("spawn_head=true").spawn_head);
        assert!(!parse_config("spawn_head = false").spawn_head);
        assert!(!parse_config("spawnHead = 0").spawn_head);
        assert!(parse_config("# spawnHead = true\n[server]\nspawn_head = TRUE").spawn_head);
        assert!(!parse_config("nonsense = true\n").spawn_head);
        assert!(!parse_config("").spawn_head);
    }

    #[test]
    fn config_round_trips() {
        for spawn_head in [false, true] {
            let config = Config { spawn_head };
            let text = format_config(&config);
            assert_eq!(parse_config(&text), config);
            assert!(text.contains(if spawn_head { "true" } else { "false" }));
        }
    }

    #[test]
    fn store_round_trips_and_rejects_garbage() {
        for kills in [0, 1, 2, 41] {
            assert_eq!(parse_store(&format_store(kills)), Some(kills));
        }
        assert_eq!(parse_store("kills\t7\n"), Some(7));
        assert_eq!(parse_store("kills=7"), Some(7));
        assert_eq!(parse_store("kills\t7\r\n"), Some(7));
        assert_eq!(parse_store(""), None);
        assert_eq!(parse_store("kills\t-1"), None);
        assert_eq!(parse_store("other\t7"), None);
        assert_eq!(parse_store("kills\tseven"), None);
    }

    #[test]
    fn the_first_kill_has_no_extra_egg_and_later_kills_do() {
        assert!(!places_egg(0));
        assert!(places_egg(1));
        assert!(places_egg(2));
        assert_eq!(after_kill(0), 1);
        assert_eq!(after_kill(1), 2);
        assert_eq!(after_kill(u32::MAX), u32::MAX);
    }

    #[test]
    fn a_world_with_a_previous_kill_adopts_one() {
        assert_eq!(adopt(false), 0);
        assert_eq!(adopt(true), 1);
    }

    #[test]
    fn facing_is_the_nearest_horizontal_direction() {
        assert_eq!(facing(0, 0), "north");
        assert_eq!(facing(3, 0), "east");
        assert_eq!(facing(-3, 0), "west");
        assert_eq!(facing(0, 3), "south");
        assert_eq!(facing(0, -3), "north");
        // Ties follow Direction's enum order: north, south, west, east.
        assert_eq!(facing(2, 2), "south");
        assert_eq!(facing(-2, 2), "south");
        assert_eq!(facing(-2, -2), "north");
        assert_eq!(facing(3, 2), "east");
    }

    #[test]
    fn clockwise_turns_north_east_south_west() {
        assert_eq!(clockwise("north"), "east");
        assert_eq!(clockwise("east"), "south");
        assert_eq!(clockwise("south"), "west");
        assert_eq!(clockwise("west"), "north");
    }

    #[test]
    fn the_head_walks_down_to_the_podium_bedrock() {
        let base = bedrock_base([0, 69, 0], |p| p == [0, 68, 0]);
        assert_eq!(base, [0, 68, 0]);
        // No bedrock anywhere: the world floor.
        assert_eq!(bedrock_base([0, 5, 0], |_| false), [0, 0, 0]);
    }

    #[test]
    fn the_head_goes_one_block_out_from_the_bedrock() {
        let base = [0, 68, 0];
        let spot = head_spot(base, "east", |p| {
            if p == [1, 68, 0] {
                Cell::Air
            } else {
                Cell::Other
            }
        });
        assert_eq!(spot, Some(([1, 68, 0], "east")));
    }

    #[test]
    fn a_bedrock_step_is_skipped() {
        let base = [0, 68, 0];
        let spot = head_spot(base, "east", |p| match p {
            [1, 68, 0] => Cell::Bedrock,
            [2, 68, 0] => Cell::Air,
            _ => Cell::Other,
        });
        assert_eq!(spot, Some(([2, 68, 0], "east")));
    }

    #[test]
    fn a_blocked_direction_turns_clockwise() {
        let base = [0, 68, 0];
        // East is solid, south is air: the head faces south.
        let spot = head_spot(base, "east", |p| match p {
            [0, 68, 1] => Cell::Air,
            _ => Cell::Other,
        });
        assert_eq!(spot, Some(([0, 68, 1], "south")));
    }

    #[test]
    fn four_blocked_tries_place_no_head() {
        assert_eq!(head_spot([0, 68, 0], "north", |_| Cell::Other), None);
    }
}
