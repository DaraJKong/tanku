#!/usr/bin/env bash
# Profiles a scripted game in a release build, and prints what `tanku_client/src/profile.rs`
# measured. No window opens.
#
# Usage: [SPANS=1] profile.sh SCENE [VAR=VALUE ...]
#
# With SPANS=1, the build has a span for every system and every schedule, which tells where
# the time goes. The spans cost time of their own, as much as the frame or more, so the times
# of the frames are read from a build without them. Going from one build to the other compiles
# the engine again, in a couple of minutes.
#
#   menu    the main menu, for 12 s
#   fight   a fight of four players in sector 21, three of whom the game drives: every
#           tank of the group of fifteen is awake
#   cross   the crossing into that sector, where the group of fifteen wakes and its models are
#           built
#   host    the same sector hosted for three guests that wait behind its barrier, while the
#           host fights under a shield: four games on a network of their own, which nothing
#           leaves, so that the host is announced to nobody (Linux with user namespaces and
#           the tool `ip`)
#
# The variables go to the game that is profiled, the host's in `host`: TANKU_PROFILE_UNCAPPED=1
# lets its frames come as fast as they can, TANKU_PROFILE_CSV=FILE writes every span to a file
# and TANKU_PROFILE_SLOW=MS tells of the frames that take longer than that.
#
# A graphics card that has little to do lowers its clocks, and every pass then takes longer.
# Where `nvidia-smi` is found, the clocks of the card are read twice a second meanwhile, and
# the line `clocks` tells their state over the second half of the run: compare two builds at
# the same state only.
set -euo pipefail
cd "$(dirname "$0")/../.."

scene="${1:-}"
shift || true
case "$scene" in
menu)
    after=3
    script="wait 12; quit"
    ;;
fight)
    after=6
    script="start TANKU 4 normal teammates expert; wait 1; warp 20; wait 2; cross; play 0 1 0 8 12; wait 12; quit"
    ;;
cross)
    after=2.5
    script="start TANKU 4 normal teammates expert; wait 1; warp 20; wait 2; bubble; cross; wait 3; quit"
    ;;
host)
    after=17.5
    script="host; wait 9; press Ready; wait 5; warp 20; wait 2; cross; bubble; play 0 1 0 8 12; wait 12; quit"
    guest="wait 2; join 127.0.0.1; wait 3; press Ready; wait 30; quit"
    ;;
*)
    echo "Usage: $0 menu|fight|cross|host [VAR=VALUE ...]" >&2
    exit 2
    ;;
esac

logs="$PWD/target/profile/$scene"
mkdir -p "$logs"
rm -f "$logs"/*.log "$logs"/clocks.csv
if [ "${SPANS:-0}" = 1 ]; then
    features=profile
else
    features=script
fi
cargo build -q --release -p tanku_client --no-default-features --features "$features"
export GAME="$PWD/target/release/tanku_client" LOGS="$logs" SCRIPT="$script" AFTER="$after"
export GUEST="${guest:-}" EXTRA="$*"

if command -v nvidia-smi > /dev/null; then
    nvidia-smi --query-gpu=pstate,clocks.gr,clocks.mem,utilization.gpu --format=csv,noheader,nounits \
        -lms 500 > "$logs/clocks.csv" 2> /dev/null &
    clocks=$!
    trap 'kill "$clocks" 2> /dev/null || true' EXIT
fi

# A scripted game opens no window. It is kept from the desktop all the same.
play() {
    cd "$LOGS"
    if [ -z "$GUEST" ]; then
        env -u WAYLAND_DISPLAY -u DISPLAY $EXTRA TANKU_PROFILE="$AFTER" TANKU_DEV_SCRIPT="$SCRIPT" \
            timeout 300 "$GAME" > game.log 2>&1
        return
    fi
    env -u WAYLAND_DISPLAY -u DISPLAY $EXTRA TANKU_PROFILE="$AFTER" TANKU_DEV_SCRIPT="$SCRIPT" \
        timeout 300 "$GAME" > game.log 2>&1 &
    for seat in 2 3 4; do
        env -u WAYLAND_DISPLAY -u DISPLAY TANKU_DEV_SCRIPT="$GUEST" \
            timeout 300 "$GAME" > "guest$seat.log" 2>&1 &
    done
    wait
}
export -f play
if [ -z "${guest:-}" ]; then
    play
else
    unshare --user --map-root-user --net bash -euo pipefail -c 'ip link set lo up; play'
fi

if [ -n "${clocks:-}" ]; then
    kill "$clocks" 2> /dev/null || true
    wait "$clocks" 2> /dev/null || true
    awk -F', ' '{ rows[NR] = $0 } END {
        for (row = int(NR / 2) + 1; row <= NR; row++) {
            split(rows[row], cell, ", "); state[cell[1]]++; core += cell[2]; memory += cell[3]
            busy += cell[4]; counted++
        }
        if (counted == 0) exit
        printf "clocks:"; for (name in state) printf " %s x%d", name, state[name]
        printf ", core %.0f MHz, memory %.0f MHz, busy %.0f%%\n", core / counted, memory / counted, busy / counted
    }' "$logs/clocks.csv"
fi
faults=$(cat "$logs"/*.log | grep -c -E 'ERROR|WARN|panicked' || true)
echo "faults in the logs: $faults"
grep '^PROFILE' "$logs/game.log" || echo "no profile: see $logs/game.log"
