import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

// Icon-only Cycle launcher. All logic lives in the Rust `cycle` binary;
// commands are exec'd directly (no shell).
BarWidget {
  id: root
  moduleName: "emrgim.cycle"

  readonly property string cycleBin: Quickshell.env("HOME") + "/.local/bin/cycle"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰖨"
    tooltipText: "Cycle — click: open · right click: apply now"
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) Quickshell.execDetached([root.cycleBin, "apply-now"])
      else Quickshell.execDetached([root.cycleBin, "app"])
    }
  }
}
