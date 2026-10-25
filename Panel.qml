import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "io.github.ipvesp.clipboard-history"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  property var preferences: ({ history_limit: 100, show_bar: true })

  function open() { root.controller.show() }
  function close() { root.controller.hide() }
  function toggle() { if (root.opened) close(); else open() }
  function switchPanel(direction) {
    return root.bar && typeof root.bar.switchPanelFrom === "function"
      ? root.bar.switchPanelFrom(root.hostWidget || root, direction) : false
  }
  function changeLimit(value) {
    var next = Math.max(5, Math.min(200, value))
    if (root.bar) root.bar.run(Quickshell.env("HOME") + "/.local/bin/vesp ch history-limit " + next)
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.hostWidget || root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(260))
    contentHeight: panel.fittedContentHeight(content.implicitHeight + Style.space(24))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: content
        width: parent.width - Style.space(24)
        anchors.centerIn: parent
        spacing: Style.space(14)

        Text {
          text: "CLIPBOARD HISTORY"
          color: root.contentForeground
          font.family: root.contentFontFamily
          font.pixelSize: Style.font.subtitle
          font.bold: true
        }

        Rectangle {
          width: parent.width
          height: Style.space(38)
          radius: 0
          color: Qt.rgba(root.contentForeground.r, root.contentForeground.g, root.contentForeground.b, 0.08)
          Text {
            anchors.centerIn: parent
            text: "Open history"
            color: root.contentForeground
            font.family: root.contentFontFamily
          }
          MouseArea {
            anchors.fill: parent
            onClicked: { if (root.bar) root.bar.run(Quickshell.env("HOME") + "/.local/bin/vesp ch open"); root.close() }
          }
        }

        Row {
          width: parent.width
          spacing: Style.space(10)
          Text {
            text: "Saved items"
            color: root.contentForeground
            font.family: root.contentFontFamily
            anchors.verticalCenter: parent.verticalCenter
          }
          Item { width: Math.max(0, parent.width - 178); height: 1 }
          Text {
            text: "−"
            color: root.contentForeground
            font.pixelSize: 20
            MouseArea { anchors.fill: parent; onClicked: root.changeLimit(Number(root.preferences.history_limit || 100) - 1) }
          }
          Text {
            text: String(root.preferences.history_limit || 100)
            color: root.contentForeground
            anchors.verticalCenter: parent.verticalCenter
          }
          Text {
            text: "+"
            color: root.contentForeground
            font.pixelSize: 20
            MouseArea { anchors.fill: parent; onClicked: root.changeLimit(Number(root.preferences.history_limit || 100) + 1) }
          }
        }

        Text {
          text: "Show in bar  •  " + (root.preferences.show_bar === false ? "Off" : "On")
          color: root.contentForeground
          font.family: root.contentFontFamily
          MouseArea {
            anchors.fill: parent
            onClicked: {
              if (root.bar) root.bar.run(Quickshell.env("HOME") + "/.local/bin/vesp ch bar " + (root.preferences.show_bar === false ? "show" : "hide"))
            }
          }
        }
      }
    }
  }
}
