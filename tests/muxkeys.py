#!/usr/bin/env python3
"""The keys and the mouse of the native mux: prefix chords, splits, focus, zoom, tabs, workspaces, resize mode, rename prompt, paste,
copy mode (OSC 52), the mouse (focus, divider, wheel, selection) and a keymap file. Usage: python3 tests/muxkeys.py"""
import base64, re
from muxlib import *
E = Env(keys="")   # an empty keys file: the defaults (prefix ctrl+b)
c = E.client(100, 30)
assert c.wait_for("$ "), c.out[-200:]
P = "\x02"
check, ok = E.check, E.ok

# splits and focus
c.send(P + "v"); E.settle()
check(len(E.panes()) == 2 and E.focused() == "w1:p2", "prefix+v splits to the right and focuses the new pane", E.panes())
check(E.layout()["panes"][0]["rect"]["x"] == 0 and E.layout()["panes"][1]["rect"]["x"] > 0, "side by side")
c.send(P + "h"); E.settle()
check(len(E.panes()) == 3 and E.focused() == "w1:p3", "prefix+h splits below")
c.send(P + "\x1b[D"); E.settle()
check(E.focused() == "w1:p1", "prefix+left focuses the pane on the left", E.focused())
c.send("\x1b[1;7C"); E.settle()   # ctrl+alt+right without the prefix
check(E.focused() in ("w1:p2", "w1:p3"), "ctrl+alt+right focuses right without the prefix", E.focused())
c.send(P + ";"); E.settle()
check(E.focused() == "w1:p1", "prefix+; goes back to the last pane", E.focused())
# zoom
c.send(P + "z"); E.settle()
check(E.layout()["zoomed"] is True and len(E.layout()["panes"]) == 1, "prefix+z zooms")
c.send(P + "z"); E.settle()
check(E.layout()["zoomed"] is False and len(E.layout()["panes"]) == 3, "and back")
# close
c.send(P + "x"); E.settle()
check(len(E.panes()) == 2, "prefix+x closes the pane", E.panes())
# a literal prefix key reaches the program: ctrl+b twice
c.send("cat -v\r"); E.settle()
c.send(P + P); E.settle()
c.send("\r"); c.wait_for("^B")
check(b"^B" in c.out, "prefix twice sends the key itself")
c.send("\x03")
# tabs
c.send(P + "c"); E.settle()
check(len(E.tabs()) == 2 and E.tabs()[1]["focused"], "prefix+c: a new tab, shown")
c.send(P + "1"); E.settle()
check(E.tabs()[0]["focused"], "prefix+1 shows the first tab")
c.send(P + "n"); E.settle()
check(E.tabs()[1]["focused"], "prefix+n: the next tab")
c.send("\x1bb"); E.settle()   # alt+b: nothing
c.send("\x1b1"); E.settle()   # alt+1
check(E.tabs()[0]["focused"], "alt+1 without the prefix")
# rename prompt
c.send(P + ","); E.settle()
c.send("work"); c.send("\r"); E.settle()
check(E.tabs()[0]["label"] == "work", "prefix+, renames the tab with a prompt", E.tabs())
c.send(P + ","); c.send("zzz"); c.send("\x1b", 0.4); E.settle()
check(E.tabs()[0]["label"] == "work", "esc cancels the prompt")
# workspaces
c.send(P + "C"); E.settle(0.4)
check(len(E.workspaces()) == 2 and E.workspaces()[1]["focused"], "prefix+shift+c: a new workspace")
c.send(P + "P"); E.settle()
check(E.workspaces()[0]["focused"], "prefix+shift+p: the previous workspace")
c.send("\x1b[1;3B"); E.settle()   # alt+down
check(E.workspaces()[1]["focused"], "alt+down: the next workspace")
c.send(P + "K"); E.settle(0.4)
check(len(E.workspaces()) == 1, "prefix+shift+k closes the workspace")
# resize mode
c.send(P + "\x1b[1;5A"); E.settle()   # (the two panes are stacked now: up/down move their divider)
w0 = E.layout()["panes"][0]["rect"]["height"]
c.send("\x1b[A"); c.send("\x1b[A"); E.settle()
w1 = E.layout()["panes"][0]["rect"]["height"]
check(w1 != w0, "resize mode: arrows move the divider", (w0, w1))
c.send("\x1b", 0.4)
c.send("\x1b[A"); E.settle()
check(E.layout()["panes"][0]["rect"]["height"] == w1, "esc leaves the resize mode (the arrow goes to the pane)")
# help overlay
c.send(P + "?"); E.settle()
check(c.wait_for("detach"), "prefix+? shows the keys")
c.send("q"); E.settle()
# bracketed paste into a pane that did not ask for it, and into one that did
c.send("cat -v\r"); E.settle()
n = len(c.out)
c.send("\x1b[200~pasted text\x1b[201~"); c.send("\r")
check(c.wait_for("pasted text", since=n), "paste arrives as text")
c.send("\x03")
E.call("pane.run", {"command": "printf '\\033[?2004h'; cat -v"}); E.settle(0.4)
n = len(c.out)
c.send("\x1b[200~wrapped\x1b[201~"); c.send("\r")
check(c.wait_for("^[[200~wrapped^[[201~", since=n), "bracketed paste is passed on when the pane asked for it", c.out[n:][-200:])
c.send("\x03"); E.call("pane.run", {"command": "printf '\\033[?2004l'"}); E.settle(0.3)
# mouse: focus by click (a fresh split side by side)
c.send(P + "v"); E.settle()
lay = E.layout()["panes"]
left = next(p for p in lay if p["rect"]["x"] == 0 and p["rect"]["y"] == 0); right = next(p for p in lay if p["rect"]["x"] > 0)
E.call("pane.focus", {"pane_id": left["pane_id"]})
c.mouse(0, right["rect"]["x"] + 3, 3); c.mouse(0, right["rect"]["x"] + 3, 3, press=False); E.settle()
check(E.focused() == right["pane_id"], "a click focuses the pane", (E.focused(), right["pane_id"]))
# drag the divider
dx = right["rect"]["x"] - 1
before = E.layout()["panes"][0]["rect"]["width"]
c.mouse(0, dx + 1, 5); c.mouse(32, dx - 9, 5); c.mouse(0, dx - 9, 5, press=False); E.settle()
after = E.layout()["panes"][0]["rect"]["width"]
check(after < before - 5, "dragging the divider resizes", (before, after))
# wheel: scrollback of the pane
E.call("pane.run", {"pane_id": E.focused(), "command": "seq 1 200"}); E.settle(0.6)
x = E.rect(E.focused())["x"]
c.mouse(64, x + 3, 5); c.mouse(64, x + 3, 5)
sc = next(p for p in E.panes() if p["focused"])["scroll"]
check(sc["offset_from_bottom"] == 6, "the wheel scrolls the pane history", sc)
c.send("q"); E.settle()
check(next(p for p in E.panes() if p["focused"])["scroll"]["offset_from_bottom"] == 0, "typing snaps back to the live screen")
c.send("\x15")   # (the q stays at the prompt: clear the line)
# copy mode: select the line with seq output and copy
c.send(P + "[", 0.3)
c.send("k"); c.send("V"); n = len(c.out); c.send("y", 0.4)
m = re.search(rb"\x1b\]52;c;([A-Za-z0-9+/=]+)\x07", c.out[n:])
txt = base64.b64decode(m.group(1)).decode() if m else None
check(txt is not None and txt.strip().isdigit(), "copy mode: V then y copies the line through OSC 52", (txt, c.out[n:][-200:]))
# a selection with the mouse
E.call("pane.run", {"command": "echo selectme_now"}); E.settle(0.5)
text = E.result("pane.read", {"source": "visible"})["text"].split("\n")
row = max(i for i, l in enumerate(text) if l.startswith("selectme_now"))
r = E.rect(E.focused())
n = len(c.out)
c.mouse(0, r["x"] + 1, r["y"] + row + 1); c.mouse(32, r["x"] + 8, r["y"] + row + 1); c.mouse(0, r["x"] + 8, r["y"] + row + 1, press=False); E.settle(0.3)
m = re.search(rb"\x1b\]52;c;([A-Za-z0-9+/=]+)\x07", c.out[n:])
txt = base64.b64decode(m.group(1)).decode() if m else None
check(txt == "selectme", "dragging with the mouse copies the selection", txt)
# detach by key
c.send(P + "d")
check(c.wait_exit() and c.status == 0, "prefix+d detaches")

# a keymap file: another prefix, a rebinding, a command
E.fresh()
open(f"{E.T}/keys", "w").write(f"prefix = ctrl+space\nsplit_vertical = prefix+s\ndetach = none\nrun prefix+t touch {E.T}/ran_$HERDR_PANE_ID\n")
c = E.client(100, 30)
assert c.wait_for("$ ")
c.send("\x00s"); E.settle()
check(len(E.panes()) == 1 + 0 or len(E.panes()) == 2, "ctrl+space is the prefix, s splits")
c.send("\x00t"); E.settle(0.5)
check(any(f.startswith("ran_w") for f in os.listdir(E.T)), "run binds a shell command", os.listdir(E.T))
c.send("\x00d"); c.pump(0.4)
check(c.status is None and c.wait_exit(0.5) is False, "detach = none removes the key")
E.done()
