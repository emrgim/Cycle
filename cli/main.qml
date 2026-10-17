import QtQuick
import Quickshell
import "../js/engine.js" as Cycle

QtObject {
  id: root
  Component.onCompleted: {
    var comp = Qt.createComponent("../EngineHost.qml")
    var hostItem = comp.createObject(root)
    var enc = Quickshell.env("CYCLE_ARGS") || ""
    var args = []
    if (enc.length && hostItem) {
      var bytes = hostItem.b64decode(enc)
      var cur = ""
      for (var i = 0; i < bytes.length; i++) {
        if (bytes[i] === 0) { args.push(cur); cur = "" }
        else cur += String.fromCharCode(bytes[i])
      }
      if (cur.length) args.push(cur)
    }
    Cycle.setHost(hostItem.api)
    Cycle.main(args).then(function(code) { Qt.exit(code || 0) }, function() { Qt.exit(1) })
  }
}
