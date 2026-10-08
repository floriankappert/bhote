import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// bhote in the bar: the dog, and the number of things for you (topics in review, agents that wait for an answer) in the
// urgent colour. The data is `bhote bar` (Waybar-style JSON from the topic files and the panels' agent lists: no herdr call).
// Left click: the herdr window and its bhote panel. Right click: the bhote search. Middle click: refresh.
BarWidget {
  id: root
  moduleName: "bhote.bar"

  property string label: "\u{f0a43}"
  property string tip: "bhote"
  property string cls: "idle"
  readonly property string here: decodeURIComponent(Qt.resolvedUrl(".").toString().replace(/^file:\/\//, ""))

  function refresh() {
    if (!barProc.running) barProc.running = true
  }

  function act(what) {
    if (root.bar) root.bar.run("sh " + root.bar.shellQuote(root.here + "focus.sh") + " " + what)
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  IpcHandler {
    target: "bhote.bar"
    function refresh(): void { root.broadcast("refresh") }
  }

  Process {
    id: barProc
    command: ["sh", "-c", "PATH=\"$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; exec bhote bar"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var j = null
        try { j = JSON.parse(String(text || "")) } catch (e) {}
        if (!j) { root.cls = "idle"; root.tip = "bhote: no answer (is bhote installed?)"; return }
        root.label = j.text || "\u{f0a43}"
        root.tip = j.tooltip || "bhote"
        root.cls = j["class"] || "idle"
      }
    }
  }

  Timer {
    interval: Math.max(2, Number(root.setting("refreshIntervalSec", 5))) * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.label
    tooltipText: root.tip
    active: root.cls === "attention"
    dimmed: root.cls === "idle"
    onPressed: function(b) {
      if (b === Qt.RightButton) root.act("search")
      else if (b === Qt.MiddleButton) root.refresh()
      else root.act("panel")
    }
  }
}
