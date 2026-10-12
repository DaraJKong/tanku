#!/usr/bin/env bash
# Runs two scripted games as two machines on a network of their own: a host at 10.77.0.1 and a
# guest at 10.77.0.2, joined by a virtual cable.
#
# Two games on one machine talk over the loopback, where every address is the machine's own
# and nothing is ever late or lost. That hides what goes wrong between two machines, so a
# change to the network is checked with this.
#
# Nothing leaves this machine and nothing here needs privileges. It takes Linux with user
# namespaces, and the tools `unshare`, `nsenter`, `ip` and `tc`.
#
# Usage: [WALL=mdns|shut] [HOST_OPTIONS=...] two_machines.sh HOST_SCRIPT GUEST_SCRIPT [LINE]
#
# The scripts are development scripts, as `TANKU_DEV_SCRIPT` takes them. LINE worsens the cable
# in both directions, in the words of `tc netem`: "delay 50ms 10ms loss 2%" makes a round trip
# of 100 ms that loses one packet in fifty each way.
#
#   tanku_client/dev/two_machines.sh \
#       "host; wait 6; press Ready; wait 9; shot host.png; quit" \
#       "wait 2; join 10.77.0.1; wait 3; press Ready; wait 9; shot guest.png; quit" \
#       "delay 50ms 10ms loss 2%"
#
# WALL puts a firewall on both machines, which takes the tool `nft`. Each lets in the answers
# to what its machine asked and nothing else, and the host has opened the port of the game.
# With `mdns`, multicast DNS comes in too, as it does through the firewalls that systems ship
# with: a guest must still find the game. With `shut`, it is kept out: the address must still
# join.
#
# HOST_OPTIONS is what the command line of the host's game holds, such as
# "--playtest --tank engine=7,muzzle=7". The guest's game has none.
set -euo pipefail
cd "$(dirname "$0")/../.."

logs=target/two_machines
mkdir -p "$logs"
# Built once here, so that the two games don't wait for each other's build.
cargo build -q -p tanku_client

export HOST_SCRIPT="$1" GUEST_SCRIPT="$2" LINE="${3:-}" LOGS="$logs" WALL="${WALL:-}"
export HOST_OPTIONS="${HOST_OPTIONS:-}"
unshare --user --map-root-user --net bash -euo pipefail -c '
    ip link set lo up
    ip link add host type veth peer name guest

    # The guest has a network of its own, which a sleeping process keeps open.
    unshare --net sleep 3600 &
    other=$!
    trap "kill $other" EXIT
    until [ "$(readlink /proc/$other/ns/net)" != "$(readlink /proc/self/ns/net)" ]; do
        sleep 0.02
    done
    ip link set guest netns "$other"
    there() { nsenter --target "$other" --net "$@"; }

    # Each end is the way out of the other, so that a game finds an address to show.
    ip addr add 10.77.0.1/24 dev host
    ip link set host up
    ip route add default via 10.77.0.2
    there ip link set lo up
    there ip addr add 10.77.0.2/24 dev guest
    there ip link set guest up
    there ip route add default via 10.77.0.1
    if [ -n "$WALL" ]; then
        wall() {
            "$@" nft add table inet wall
            "$@" nft add chain inet wall incoming "{ type filter hook input priority 0; policy drop; }"
            "$@" nft add rule inet wall incoming iif lo accept
            "$@" nft add rule inet wall incoming ct state established,related accept
            if [ "$WALL" = mdns ]; then
                "$@" nft add rule inet wall incoming ip daddr 224.0.0.251 udp dport 5353 accept
            fi
        }
        wall
        nft add rule inet wall incoming udp dport 28405 accept
        wall there
    fi
    if [ -n "$LINE" ]; then
        tc qdisc add dev host root netem $LINE
        there tc qdisc add dev guest root netem $LINE
    fi

    # A scripted game opens no window. It is kept from the desktop all the same.
    game() {
        env -u WAYLAND_DISPLAY -u DISPLAY TANKU_DEV_SCRIPT="$1" \
            timeout 600 cargo run -q -p tanku_client -- $HOST_OPTIONS
    }
    game "$HOST_SCRIPT" > "$LOGS/host.log" 2>&1 &
    first=$!
    there env -u WAYLAND_DISPLAY -u DISPLAY TANKU_DEV_SCRIPT="$GUEST_SCRIPT" \
        timeout 600 cargo run -q -p tanku_client > "$LOGS/guest.log" 2>&1 &
    second=$!
    wait "$first" "$second"
'
echo "What the two games said is in $logs/host.log and $logs/guest.log."
