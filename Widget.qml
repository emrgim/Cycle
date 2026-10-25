import QtQuick
import Quickshell
import qs.Commons
import qs.Ui
import "js/engine.js" as Cycle

BarWidget {
  id: root
  moduleName: "emrgim.cycle"

  EngineHost {
    id: engineHost
    onMinute: function() { Cycle.tick(false).then(null, function(e) { console.log("cycle: tick failed: " + (e && e.message ? e.message : e)) }) }
  }

  Component.onCompleted: Cycle.arm(engineHost.api)

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: String.fromCodePoint(0xF05A8)
    tooltipText: "Cycle \u2014 click: open \u00b7 right click: apply now"
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) Cycle.tick(true)
      else Quickshell.execDetached(["omarchy-shell", "shell", "summon", "emrgim.cycle"])
    }
  }
}
