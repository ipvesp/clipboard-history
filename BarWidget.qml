import QtQuick
import Quickshell
import Quickshell.Io
import qs.Ui

BarWidget {
  id: root
  moduleName: "io.github.ipvesp.clipboard-history"

  property var preferences: ({ history_limit: 100, show_bar: true })
  readonly property bool shown: preferences.show_bar !== false
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function open() { if (shown && panelLoader.item) panelLoader.item.open() }
  function close() { if (panelLoader.item) panelLoader.item.close() }
  function toggle() { if (shown && panelLoader.item) panelLoader.item.toggle() }
  function closeForPopoutSwitch() { if (panelLoader.item) panelLoader.item.closeForPopoutSwitch() }

  function injectPanel() {
    if (!panelLoader.item) return
    panelLoader.item.bar = root.bar
    panelLoader.item.anchorItem = button
    panelLoader.item.hostWidget = root
    panelLoader.item.preferences = root.preferences
  }

  onBarChanged: injectPanel()
  onPreferencesChanged: {
    injectPanel()
    if (!shown) close()
  }

  implicitWidth: shown ? button.implicitWidth : 0
  implicitHeight: shown ? button.implicitHeight : 0
  visible: shown

  FileView {
    id: preferencesFile
    path: (Quickshell.env("XDG_CONFIG_HOME") || (Quickshell.env("HOME") + "/.config")) + "/clipboard-history/settings.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      try { root.preferences = JSON.parse(text()) }
      catch (e) { root.preferences = ({ history_limit: 100, show_bar: true }) }
    }
    onLoadFailed: root.preferences = ({ history_limit: 100, show_bar: true })
  }

  Timer {
    interval: 1000
    running: true
    onTriggered: preferencesFile.reload()
  }

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: { root.injectPanel(); Qt.callLater(root.injectPanel) }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰅌"
    tooltipText: "Clipboard History"
    onPressed: function(code) { if (code === Qt.LeftButton) root.toggle() }
  }
}
