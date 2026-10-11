"""Drives a Firefox that shows no window through a visit of a page, for `browser_guest.sh`.

    python3 firefox.py URL OUT_DIR PORT STEP...

Firefox must have been started with `--marionette` and listen on PORT. The steps run one after
the other:

    wait:SECONDS     does nothing for so long
    shot:NAME        saves what the page shows as OUT_DIR/NAME.png
    click:X:Y        clicks the left button at a point of the page
    key:KEY:SECONDS  holds a key for so long
    paste:TEXT       puts a text on the clipboard of the browser and presses Ctrl+V
    eval:SCRIPT      runs a script in the page and prints what it returns

The clicks and the keys are real ones to the page: a browser counts them as the player's own.
"""

import base64
import json
import socket
import sys
import time


class Firefox:
    """Speaks to Firefox as its own test tools do: each message is its length, a colon and JSON."""

    def __init__(self, port):
        deadline = time.time() + 40
        while True:
            try:
                self.socket = socket.create_connection(("127.0.0.1", port), timeout=120)
                break
            except OSError:
                if time.time() > deadline:
                    raise
                time.sleep(0.2)
        self.unread = b""
        self.sent = 0
        self.read()
        self.send("WebDriver:NewSession", {})

    def receive(self):
        # A Firefox that is gone sends nothing, as often as it is asked: waiting for more
        # would never end.
        received = self.socket.recv(1 << 20)
        if not received:
            raise ConnectionError("Firefox is gone")
        return received

    def read(self):
        while b":" not in self.unread:
            self.unread += self.receive()
        length, _, rest = self.unread.partition(b":")
        length = int(length)
        while len(rest) < length:
            rest += self.receive()
        self.unread = rest[length:]
        return json.loads(rest[:length])

    def send(self, name, parameters):
        self.sent += 1
        message = json.dumps([0, self.sent, name, parameters]).encode()
        self.socket.sendall(str(len(message)).encode() + b":" + message)
        _, _, error, result = self.read()
        if error:
            raise RuntimeError(error)
        return result

    def act(self, kind, name, actions, **parameters):
        source = {"type": kind, "id": name, "actions": actions, **parameters}
        self.send("WebDriver:PerformActions", {"actions": [source]})

    def run(self, script, arguments=()):
        asked = {"script": script, "args": list(arguments), "newSandbox": False}
        return self.send("WebDriver:ExecuteScript", asked).get("value")


def pause(seconds):
    return {"type": "pause", "duration": int(float(seconds) * 1000)}


def main():
    url, out, port = sys.argv[1], sys.argv[2], int(sys.argv[3])
    firefox = Firefox(port)
    firefox.send("WebDriver:Navigate", {"url": url})
    start = time.time()

    def said(text):
        print(f"{time.time() - start:5.1f} s: {text}", flush=True)

    for step in sys.argv[4:]:
        kind, _, rest = step.partition(":")
        if kind == "wait":
            time.sleep(float(rest))
        elif kind == "shot":
            shot = firefox.send("WebDriver:TakeScreenshot", {"full": False})["value"]
            with open(f"{out}/{rest}.png", "wb") as image:
                image.write(base64.b64decode(shot))
            said(f"shot {rest}")
        elif kind == "click":
            x, y = (int(part) for part in rest.split(":"))
            actions = [
                {"type": "pointerMove", "x": x, "y": y, "duration": 50},
                {"type": "pointerDown", "button": 0},
                pause(0.12),
                {"type": "pointerUp", "button": 0},
            ]
            firefox.act("pointer", "mouse", actions, parameters={"pointerType": "mouse"})
            said(f"clicked {x}, {y}")
        elif kind == "key":
            key, seconds = rest.split(":")
            actions = [{"type": "keyDown", "value": key}, pause(seconds), {"type": "keyUp", "value": key}]
            firefox.act("key", "keys", actions)
            said(f"held {key} for {seconds} s")
        elif kind == "paste":
            # The clipboard is the browser's own, which a page can't write to unasked.
            firefox.send("Marionette:SetContext", {"value": "chrome"})
            copy = (
                'Cc["@mozilla.org/widget/clipboardhelper;1"]'
                ".getService(Ci.nsIClipboardHelper).copyString(arguments[0]);"
            )
            firefox.run(copy, [rest])
            firefox.send("Marionette:SetContext", {"value": "content"})
            control = ""
            actions = [
                {"type": "keyDown", "value": control},
                {"type": "keyDown", "value": "v"},
                pause(0.08),
                {"type": "keyUp", "value": "v"},
                {"type": "keyUp", "value": control},
            ]
            firefox.act("key", "keys", actions)
            said(f"pasted {rest}")
        elif kind == "eval":
            said(f"{rest} -> {firefox.run(rest)}")
        else:
            sys.exit(f"not a step: {step}")
    try:
        firefox.send("Marionette:Quit", {})
    except (OSError, RuntimeError):
        pass


main()
