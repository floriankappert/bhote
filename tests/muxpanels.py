#!/usr/bin/env python3
"""The panels built into the native mux: left and right panel per tab from the settings, widths, no re-creation after a close,
the jump key, the popup. The panel is a fake program here (it says which side it is). Usage: python3 tests/muxpanels.py"""
import os, time
from muxlib import *
FAKE = '#!/bin/sh\necho "panel side=$BHOTE_SIDE" > "$(dirname "$0")/side.$BHOTE_SIDE"\nprintf "PANEL-%s\\n" "$BHOTE_SIDE"\nexec sleep 600\n'
E = Env(config="PANEL_LAYOUT=split\nPANEL_LEFT_WIDTH=40\n", panel_bin=FAKE)
check = E.check
labels = lambda: {p["pane_id"]: p.get("label", "") for p in E.panes()}
# nothing before a client shows a size
E.start()
check(len(E.panes()) == 1, "before a client attaches there are no panels")
c = E.client(200, 50)
check(c.wait_for("PANEL-right") and c.wait_for("PANEL-left"), "both panels draw")
E.settle(0.3)
pl = E.panes()
check(len(pl) == 3, "one tab: main pane and two panels", pl)
by = {p.get("label", ""): p["pane_id"] for p in pl}
lay = {p["pane_id"]: p["rect"] for p in E.layout()["panes"]}
check(lay[by["bhote-left"]]["width"] == 40, "the left panel is 40 columns (PANEL_LEFT_WIDTH)", lay)
check(lay[by["bhote-right"]]["width"] == 56, "the right panel is 56 columns", lay)
check(lay[by["bhote-left"]]["x"] == 0 and lay[by[""]]["x"] > 0 and lay[by["bhote-right"]]["x"] > lay[by[""]]["x"], "left, main, right in this order")
check(E.focused() == by[""], "the focus stays on the main pane")
check(os.path.exists(f"{E.T}/side.left") and os.path.exists(f"{E.T}/side.right"), "the panels know their side (BHOTE_SIDE)")
# a new tab and a new workspace get panels too
c.send("\x02c"); E.settle(0.6)
tab2 = [p for p in E.panes() if p["tab_id"] == "w1:t2"]
check(len(tab2) == 3, "a new tab gets its panels", tab2)
c.send("\x02C"); E.settle(0.8)
check(len([p for p in E.panes() if p["workspace_id"] == "w2"]) == 3, "a new workspace gets its panels")
c.send("\x02P"); E.settle()
# the jump key: main -> right panel -> back
main = by[""]
E.call("pane.focus", {"pane_id": main}); E.settle(0.2)
c.send("\x02t"); E.settle(0.3)
check(E.focused() == by["bhote-right"], "prefix+t jumps to the panel", E.focused())
c.send("\x02t"); E.settle(0.3)
check(E.focused() == main, "and back to the pane before", E.focused())
# a closed panel stays closed; prefix+b brings it back
E.call("pane.close", {"pane_id": by["bhote-left"]}); E.settle(0.5)
t1 = lambda: {p["pane_id"]: p.get("label", "") for p in E.panes() if p["tab_id"] == "w1:t1"}
check("bhote-left" not in t1().values(), "a closed panel is not made again by itself", t1())
E.call("pane.focus", {"pane_id": main})
c.send("\x02b"); E.settle(0.8)
check("bhote-left" in t1().values(), "prefix+b makes the missing panel again", t1())
# widths follow pane resize like the plugin's panel_set_width does (the panel is the right/left part of a split)
newleft = next(k for k, v in t1().items() if v == "bhote-left")
w0 = E.rect(newleft)["width"]
E.call("pane.resize", {"pane_id": newleft, "direction": "right", "amount": 0.05}); E.settle(0.3)
check(E.rect(newleft)["width"] > w0, "pane resize widens the left panel", (w0, E.rect(newleft)["width"]))
# the popup
c.kill()
E.fresh()
open(f"{E.T}/keys", "w").write("popup ctrl+alt+t echo popup_$((6*7)); read x; echo got_$x; sleep 1\n")
open(f"{E.T}/config", "w").write("AUTOSTART=off\n")
c = E.client(120, 40)
assert c.wait_for("$ ")
c.send("\x1b\x14", 0.6)   # ctrl+alt+t as terminals send it (ESC ctrl+t)
check(c.wait_for("popup_42"), "the popup key opens a floating pane that runs its command", c.out[-200:])
check(E.focused() is not None and next(p for p in E.panes() if p["focused"])["pane_id"] != "w1:p1", "the popup has the focus")
c.send("hi\r", 0.5)
check(c.wait_for("got_hi"), "typing goes to the popup")
E.settle(0.8)
check(len(E.panes()) == 1 and E.focused() == "w1:p1", "when its process ends the popup is gone and the focus is back", E.panes())
E.done()
