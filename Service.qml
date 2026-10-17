import QtQuick
import Quickshell
import "js/engine.js" as Cycle

Item {
  id: root
  EngineHost {
    id: engineHost
    onMinute: function() { Cycle.tick(false) }
  }
  Component.onCompleted: Cycle.arm(engineHost.api)
}
