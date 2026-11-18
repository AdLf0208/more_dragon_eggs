#!/usr/bin/env bash
# In-game check for More Dragon Eggs (fail-capable), SDK package 0.2.0.
#
# STATUS (2026-11-18): the 0.1.0 `.hnmod` run recorded in notes.md is HISTORICAL. 0.2.0 is a
# native SDK package, so the game must be built by the **launcher**
# (`horizon-launcher native-build . --instance <i> --trust-native-code`); `horizon-launcher` is
# not installed in this environment, so this script has NOT been run for 0.2.0. Run it and record
# the transcript before claiming `ported`.
#
#   GAME=/path/to/launcher-built/horizon-client CTL=/path/to/horizon-ctl MODE=default tools/e2e.sh
#   MODE=head ...                                   # spawn_head = true (config differential)
#   SELFTEST=1 ...                                  # asserts the opposite: the run MUST exit 1
#   CONTROL_GAME=/path/to/unmodified/horizon-client # extra leg: no port, no second-kill egg
#
# The pin already places vanilla's first-kill egg on the podium; upstream More Dragon Eggs adds an
# egg at the same heightmap position on every LATER kill (and, with spawnHead, a wall head on every
# kill). So the run:
#
#   kill 1  -> vanilla egg only; the port's state file records one kill; no head (MODE=default)
#              or a south-facing head (MODE=head)
#   remove the vanilla egg
#   respawn the dragon with four end crystals on the portal ring
#   kill 2  -> the egg at the podium is the PORT's egg (vanilla adds none after the first kill);
#              this is the fail-capable assertion: without the port the cell stays air
#   reset the port's state file to 0, remove the egg, respawn, kill 3
#              -> no egg (the counter, not a lucky vanilla path, drives it)
#
# SELFTEST=1 turns the kill-2 assertion around (it expects NO egg), so a healthy mod fails it and
# the log ends `EXIT:1`: proof the scenario can fail rather than merely pass.
#
# The fight state lives on the integrated server thread, not in `ctl state` (that is the window's
# client), so this script observes it the way a player would: `execute if entity` for the dragon,
# `ctl blocks` for the podium, the `/kill` feedback, and the mod's own state file.
#
# Needs the prepared 26.3 resources (RES), a launcher-built game (GAME) and the pin's horizon-ctl.
set -uo pipefail

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/moredragoneggs
CTL=${CTL:-}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/moredragoneggs-e2e.sock}
MODE=${MODE:-default}
SELFTEST=${SELFTEST:-0}
WORLD=MDEggsE2E
LOG=${LOG:-/tmp/opencode/moredragoneggs-$MODE.log}

STATUS=1
SCRATCH=""

[ -n "$GAME" ] || {
    echo "missing GAME: point it at a launcher-built horizon-client (horizon-launcher native-build . --trust-native-code)" >&2
    exit 1
}
[ -x "$CTL" ] || {
    echo "missing CTL: the horizon-ctl built from the same sources as GAME (a shared control binary from another pin is not protocol-compatible)" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
dragon() { ctl command "execute if entity @e[type=minecraft:ender_dragon]" | jq -r '.chat[0]'; }
egg_y() {
    ctl blocks 0 50 0 0 90 0 \
        | jq -r '.blocks[] | select(.[3] == "minecraft:dragon_egg") | .[1]' | head -1
}
head_count() {
    ctl blocks -2 $((Y - 2)) -2 2 $((Y + 10)) 2 \
        | jq '[.blocks[] | select(.[3] | startswith("minecraft:dragon_wall_head"))] | length'
}
# The SDK's per-world data folder (`world_data_files()`): `<world>/horizon-mod-data/<mod id>/`.
kill_count() { cat "$STATE" 2>/dev/null || echo "missing"; }

cleanup() {
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    [ -n "$SCRATCH" ] && [ -f "$SCRATCH/game/logs/latest.log" ] \
        && cp "$SCRATCH/game/logs/latest.log" "/tmp/opencode/moredragoneggs-game-$MODE.log" || true
    [ -n "$SCRATCH" ] && rm -rf "$SCRATCH" 2>/dev/null || true
    echo "EXIT:$STATUS"
}
trap cleanup EXIT

home() {
    SCRATCH=$(mktemp -d /tmp/opencode/mdeggs-home.XXXXXX)
    HORIZON_HOME=$SCRATCH/game
    HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
    XDG_DATA_HOME=$SCRATCH/data
    XDG_CONFIG_HOME=$SCRATCH/config
    HORIZON_NO_AUDIO=1
    export HORIZON_HOME HORIZON_LAUNCHER_HOME XDG_DATA_HOME XDG_CONFIG_HOME HORIZON_NO_AUDIO
    mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME" "$HORIZON_HOME/config"
    rm -f "$SOCK"
    # The SDK's `config_files()` owns `config/<file>` beside the game's options.txt.
    local head=$1
    cat > "$HORIZON_HOME/config/moredragoneggs.conf" <<EOF
# e2e: ${head}
spawn_head = ${head}
EOF
    STATE=$HORIZON_HOME/saves/$WORLD/horizon-mod-data/moredragoneggs/state.tsv
}

launch() {
    local game=$1
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch \
        --game "$game" --world "$WORLD" --seed 20261104 --creative --headless --wait \
        --resources "$RES"
    ctl wait-ready 120
    ctl wait 10
}

# `execute if entity` answers "Test passed. Count: N" or "Test failed".
wait_dragon() {
    local tries=${1:-90}
    for _ in $(seq "$tries"); do
        [ "$(dragon)" = "Test passed. Count: 1" ] && return 0
        ctl wait 20 >/dev/null
    done
    return 1
}

wait_egg() {
    local tries=${1:-60}
    for _ in $(seq "$tries"); do
        [ -n "$(egg_y)" ] && return 0
        ctl wait 20 >/dev/null
    done
    return 1
}

kill_dragon() {
    local reply
    reply=$(ctl command "kill @e[type=minecraft:ender_dragon]")
    grep -q 'Killed Ender Dragon' <<<"$reply" || { echo "$reply" >&2; fail "the kill command did not kill the dragon"; }
}

# Four crystals in the ring cells `tryRespawn` wants (`origin + 1`, three out), then one real
# right-click on an obsidian post to run EndCrystalItem.useOn and `EnderDragonFight.tryRespawn`.
# Post N is at (20, 100, N); stand south of it on the stone plateau built at y=99.
respawn_dragon() {
    local post=$1 stand_z=$2
    ctl command "summon minecraft:end_crystal 0.5 $((Y + 1)) 3.5"
    ctl command "summon minecraft:end_crystal 0.5 $((Y + 1)) -2.5"
    ctl command "summon minecraft:end_crystal 2.5 $((Y + 1)) 0.5"
    ctl command "summon minecraft:end_crystal -2.5 $((Y + 1)) 0.5"
    ctl wait 5
    ctl command "tp 20.5 100 $stand_z"
    ctl wait 5
    ctl look-at-block 20 100 "$post" south || fail "cannot aim at respawn post $post"
    ctl give minecraft:end_crystal 8 >/dev/null || fail "cannot give end crystals"
    ctl select minecraft:end_crystal >/dev/null 2>&1 || true
    ctl wait 3
    ctl use-item >/dev/null
    ctl wait 10
    ctl wait 20 >/dev/null
    wait_dragon 90 || fail "the dragon did not respawn after the ritual (post $post)"
    echo "dragon respawned (post $post)"
}

# The podium, the arena and a plateau for the respawn ritual.
prepare_end() {
    ctl command "time set noon"
    ctl command "execute in minecraft:the_end run tp @s 0.5 70 8.5"
    ctl wait 60
    echo "== the End fight: a dragon, and the podium's bedrock column (0,0) =="
    wait_dragon 60 || fail "no dragon in the End fight"
    COLUMN=$(ctl blocks 0 40 0 0 90 0 | jq -r '.blocks[] | select(.[3] == "minecraft:bedrock") | .[1]')
    Y=$(sort -n <<<"$COLUMN" | head -1)
    [ -n "$Y" ] || { echo "$COLUMN" >&2; fail "no bedrock column at 0,0"; }
    echo "podium origin y=$Y (end-portal level); egg will sit at its top + 1"
    [ -z "$(egg_y)" ] || { echo "podium already has an egg at $(egg_y)" >&2; fail "not a fresh fight"; }
    echo "== a plateau for the respawn ritual, far from the podium =="
    ctl command "fill 17 99 -2 23 99 8 minecraft:stone"
    ctl command "fill 17 100 -2 23 104 8 minecraft:air"
    for post in 0 3 6; do
        ctl command "setblock 20 100 $post minecraft:obsidian"
    done
    ctl wait 5
}

# ---- leg A: the main run (the port installed) ------------------------------------------------
main_leg() {
    home "$([ "$MODE" = head ] && echo true || echo false)"
    echo "== launch (creative, $MODE) =="
    launch "$GAME"
    prepare_end

    echo "== kill 1: vanilla's egg, our first kill; no PORT egg, head only in MODE=head =="
    kill_dragon
    wait_egg 60 || fail "the first kill's vanilla egg never appeared"
    EGG=$(egg_y)
    echo "first-kill (vanilla) egg at y=$EGG"
    echo "state after kill 1: $(kill_count)"
    [ "$(kill_count)" = $'kills\t1' ] || fail "expected one kill in the state file, got '$(kill_count)'"
    HEADS=$(head_count)
    echo "dragon wall heads after kill 1: $HEADS"
    if [ "$MODE" = head ]; then
        [ "$HEADS" -ge 1 ] || fail "spawn_head=true but no dragon wall head after kill 1"
        HEAD=$(ctl block 0 $((EGG - 1)) 1)
        echo "head beside the podium: $HEAD"
        grep -q '"block":"minecraft:dragon_wall_head"' <<<"$HEAD" || fail "no head at the podium's south side"
        grep -q 'facing=south' <<<"$HEAD" || fail "the head does not face the player (south)"
    else
        [ "$HEADS" -eq 0 ] || fail "spawn_head=false but a dragon wall head appeared"
    fi
    ctl command "setblock 0 $EGG 0 minecraft:air"
    ctl wait 5
    [ -z "$(egg_y)" ] || fail "the vanilla egg could not be removed"
    if [ "$MODE" = default ]; then
        # A thin block the engine's MOTION_BLOCKING height counts but a `full_collision` predicate
        # does not: the port must take the engine's height (one above the lantern).
        ctl command "setblock 0 $EGG 0 minecraft:lantern" >/dev/null
        ctl wait 5
        echo "podium marker: $(ctl block 0 $EGG 0)"
    fi

    echo "== respawn 1, then kill 2 -> the port must place the egg =="
    respawn_dragon 0 1.5
    kill_dragon
    if [ "$SELFTEST" = 1 ]; then
        # Inverted on purpose: a healthy mod places the egg here, so this leg must FAIL.
        ctl wait 60
        CONTROL=$(egg_y)
        [ -z "$CONTROL" ] || fail "SELFTEST: the port placed the second-kill egg as it should, so the inverted assertion fails here"
        echo "SELFTEST: no egg after kill 2 (an unhealthy run)"
        exit 0
    fi
    wait_egg 60 || { ctl blocks 0 50 0 0 90 0 >&2; fail "no egg after the second kill"; }
    SECOND=$(egg_y)
    echo "egg after kill 2: $SECOND"
    if [ "$MODE" = default ]; then
        [ "$SECOND" = "$((EGG + 1))" ] \
            || fail "the second egg is at $SECOND; the engine heightmap above the lantern at $EGG gives $((EGG + 1))"
    fi
    echo "state after kill 2: $(kill_count)"
    [ "$(kill_count)" = $'kills\t2' ] || fail "expected two kills in the state file, got '$(kill_count)'"
    HEADS=$(head_count)
    echo "dragon wall heads after kill 2: $HEADS"
    if [ "$MODE" = head ]; then
        [ "$HEADS" -ge 1 ] || fail "spawn_head=true but no dragon wall head after kill 2"
    else
        [ "$HEADS" -eq 0 ] || fail "spawn_head=false but a dragon wall head appeared"
    fi

    echo "== state-reset control: 0 kills + kill 3 must place NO egg =="
    ctl command "setblock 0 $SECOND 0 minecraft:air"
    printf 'kills\t0\n' > "$STATE"
    ctl wait 5
    [ -z "$(egg_y)" ] || fail "the second-kill egg could not be removed"
    respawn_dragon 3 4.5
    kill_dragon
    ctl wait 60
    CONTROL=$(egg_y)
    echo "egg after kill 3 (state reset): ${CONTROL:-none}"
    [ -z "$CONTROL" ] || { ctl blocks 0 50 0 0 90 0 >&2; fail "CONTROL FAILED: a state of 0 still placed an egg"; }
    echo "state after kill 3: $(kill_count)"
    [ "$(kill_count)" = $'kills\t1' ] || fail "expected the reset counter to record one kill, got '$(kill_count)'"
    if [ "$MODE" = head ]; then
        HEADS=$(head_count)
        echo "dragon wall heads after kill 3: $HEADS"
        [ "$HEADS" -ge 1 ] || fail "spawn_head=true but no dragon wall head after kill 3"
        echo "PASS (head): a head on every kill, an egg only when the state says a dragon died before"
    else
        echo "PASS (default): first-kill vanilla egg only; the port's egg on kill 2 at the engine heightmap cell; no egg after a state reset; no head with spawn_head=false"
    fi
    ctl quit >/dev/null 2>&1 || true
}

# ---- leg B: the unmodified game (optional differential) --------------------------------------
# The same two kills on a game without the port: kill 2 must leave the podium empty. This is the
# differential that says the egg came from the mod, not from a vanilla path.
control_leg() {
    echo "== control: the unmodified game, the same two kills =="
    local game=$CONTROL_GAME
    home false
    launch "$game"
    prepare_end
    kill_dragon
    wait_egg 60 || fail "control: the unmodified game's first-kill egg never appeared"
    local egg
    egg=$(egg_y)
    ctl command "setblock 0 $egg 0 minecraft:air"
    ctl wait 5
    respawn_dragon 0 1.5
    kill_dragon
    ctl wait 60
    local second
    second=$(egg_y)
    echo "control egg after kill 2: ${second:-none}"
    [ -z "$second" ] || { ctl blocks 0 50 0 0 90 0 >&2; fail "CONTROL FAILED: the unmodified game placed a second-kill egg"; }
    echo "PASS (control): no port, no second-kill egg"
    ctl quit >/dev/null 2>&1 || true
}

exec > >(tee "$LOG") 2>&1
main_leg
STATUS=$?
if [ "$STATUS" -ne 0 ]; then
    exit "$STATUS"
fi
if [ -n "$CONTROL_GAME" ] && [ "$SELFTEST" != 1 ]; then
    control_leg
    STATUS=$?
fi
exit "$STATUS"
