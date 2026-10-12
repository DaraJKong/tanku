#!/usr/bin/env bash
# Runs a scripted game and a browser as two machines on a network of their own: the game hosts
# at 10.77.0.1, and a Firefox at 10.77.0.2 opens the page of the web build with the link of
# that host at its end, as a player who was handed the link does.
#
# A browser doesn't join as a desktop game does: it speaks WebTransport, takes the certificate
# of the host by its print, and reads what it is sent in its own way. None of that shows
# between two desktop games, so a change to the network is checked with this too.
#
# Nothing leaves this machine, no window opens and nothing here needs privileges. It takes
# Linux with user namespaces, the tools `unshare`, `nsenter`, `ip` and `tc`, Python 3, Firefox,
# and the web build in target/web-dist, which `tanku_client/web/build.sh` makes.
#
# Usage: [OPEN=bare] browser_guest.sh HOST_SCRIPT STEPS [LINE]
#
# HOST_SCRIPT is a development script, as `TANKU_DEV_SCRIPT` takes it. STEPS are what the
# browser does once the page is open, separated by spaces: they are listed at the top of
# `firefox.py`. In a step, LINK stands for what the host hands out. LINE worsens the cable in
# both directions, in the words of `tc netem`.
#
#   tanku_client/dev/browser_guest.sh \
#       "host; wait 34; press Ready; wait 7; shot host.png; wait 14; quit" \
#       "wait:24 shot:lobby click:262:425 wait:12 click:683:600 key:w:2 shot:run wait:5" \
#       "delay 60ms 10ms loss 2%"
#
# The page is 1366 by 682 pixels. What the browser shows goes to target/browser_guest, with
# what the host and the browser said. With OPEN=bare the page opens without the link, for the
# steps to bring it in: `paste:LINK` pastes it into the page.
set -euo pipefail
cd "$(dirname "$0")/../.."
[ $# -ge 2 ] || { echo "Usage: [OPEN=bare] browser_guest.sh HOST_SCRIPT STEPS [LINE]"; exit 1; }

out=target/browser_guest
[ -f target/web-dist/tanku_bg.wasm ] || { echo "No web build: run tanku_client/web/build.sh first."; exit 1; }
# A browser that starts afresh each time keeps nothing of the run before.
rm -rf target/browser_guest/profile
mkdir -p "$out/profile"
printf 'user_pref("media.volume_scale", "0.0");\nuser_pref("marionette.port", 2828);\n' > "$out/profile/user.js"
# Built once here, so that the browser doesn't wait for the build of the game.
cargo build -q -p tanku_client

export HOST_SCRIPT="$1" STEPS="$2" LINE="${3:-}" OUT="$out" OPEN="${OPEN:-}"
unshare --user --map-root-user --net bash -euo pipefail -c '
    ip link set lo up
    ip link add host type veth peer name guest

    # The browser has a network of its own, which a sleeping process keeps open.
    unshare --net sleep 3600 &
    other=$!
    # Whatever was started is told to end, and the script ends once it has.
    tidy() {
        kill $other ${host:-} ${pages:-} ${browser:-} 2> /dev/null || true
        wait
    }
    trap tidy EXIT
    until [ "$(readlink /proc/$other/ns/net)" != "$(readlink /proc/self/ns/net)" ]; do
        sleep 0.02
    done
    ip link set guest netns "$other"
    there() { nsenter --target "$other" --net "$@"; }

    ip addr add 10.77.0.1/24 dev host
    ip link set host up
    ip route add default via 10.77.0.2
    there ip link set lo up
    there ip addr add 10.77.0.2/24 dev guest
    there ip link set guest up
    there ip route add default via 10.77.0.1
    if [ -n "$LINE" ]; then
        tc qdisc add dev host root netem $LINE
        there tc qdisc add dev guest root netem $LINE
    fi

    # A scripted game opens no window. It is kept from the desktop all the same.
    env -u WAYLAND_DISPLAY -u DISPLAY TANKU_DEV_SCRIPT="$HOST_SCRIPT" \
        timeout 600 cargo run -q -p tanku_client > "$OUT/host.log" 2>&1 &
    host=$!

    # The game writes what it hands out for browsers to its log.
    link=""
    for _ in $(seq 1 300); do
        link=$(grep -m 1 -o "#[0-9.]*:[0-9]*/[0-9a-f]\{64\}" "$OUT/host.log") && break
        sleep 0.2
    done
    [ -n "$link" ] || { echo "The host handed out no link: see $OUT/host.log."; exit 1; }
    echo "The host hands out $link"

    # A browser only joins from a page that it loaded safely, as from its own machine. This
    # one keeps the desktop in its environment: without it, it draws no 3D at all. It opens
    # no window for all that.
    #
    # Both are started by `nsenter` itself and not through `there`: a function that is sent to
    # the background runs in a shell of its own, and `$!` is then that shell. Stopping it
    # leaves the program running for good.
    nsenter --target "$other" --net python3 -m http.server 8765 --bind 127.0.0.1 \
        --directory target/web-dist > /dev/null 2>&1 &
    pages=$!
    nsenter --target "$other" --net env MOZ_HEADLESS=1 firefox --headless --no-remote \
        --marionette -remote-allow-system-access --profile "$OUT/profile" > "$OUT/firefox.log" 2>&1 &
    browser=$!
    page="http://localhost:8765/"
    [ "$OPEN" = bare ] || page="$page$link"
    # The steps are words, and none of them is a pattern of file names.
    set -f
    there python3 tanku_client/dev/firefox.py "$page" "$OUT" 2828 ${STEPS//LINK/$link}
    kill "$browser" "$pages" 2> /dev/null || true
    wait "$host" || true
    host=""
'
echo "What the browser showed and what was said is in $out."
