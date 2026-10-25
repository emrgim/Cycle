import QtQuick
import Quickshell
import "js/engine.js" as Cycle

Item {
  id: root
  EngineHost {
    id: engineHost
    onMinute: function() { Cycle.tick(false).then(null, function(e) { console.log("cycle: tick failed: " + (e && e.message ? e.message : e)) }) }
  }
  Component.onCompleted: Cycle.arm(engineHost.api)
}
