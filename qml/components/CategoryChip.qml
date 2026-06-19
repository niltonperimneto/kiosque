// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Controls.AbstractButton {
    id: root

    required property string label
    required property string categoryId
    property bool selected: false

    implicitWidth: chipRow.implicitWidth + Kirigami.Units.largeSpacing * 2
    implicitHeight: chipRow.implicitHeight + Kirigami.Units.smallSpacing * 2

    contentItem: Item {}

    background: Rectangle {
        id: chipBackground

        radius: root.height / 2
        color: root.selected
               ? Kirigami.Theme.highlightColor
               : Qt.rgba(Kirigami.Theme.highlightColor.r,
                         Kirigami.Theme.highlightColor.g,
                         Kirigami.Theme.highlightColor.b,
                         hoverHandler.hovered ? 0.18 : 0.10)

        Behavior on color {
            ColorAnimation { duration: 150 }
        }
    }

    RowLayout {
        id: chipRow
        anchors.centerIn: parent
        spacing: Kirigami.Units.smallSpacing

        Controls.Label {
            text: root.label
            font.weight: root.selected ? Font.DemiBold : Font.Normal
            color: root.selected
                   ? Kirigami.Theme.highlightedTextColor
                   : Kirigami.Theme.textColor

            Behavior on color {
                ColorAnimation { duration: 150 }
            }
        }
    }

    HoverHandler {
        id: hoverHandler
        cursorShape: Qt.PointingHandCursor
    }
}
