// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Kirigami.ShadowedRectangle {
    id: root

    property string userName: ""
    property string dateStr: ""
    property int rating: 0
    property string summary: ""
    property string description: ""
    property string version: ""

    radius: 8
    color: Kirigami.Theme.backgroundColor
    border.width: 1
    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)
    shadow.size: 4
    shadow.color: Qt.rgba(0, 0, 0, 0.05)
    shadow.yOffset: 2

    implicitHeight: layout.implicitHeight + Kirigami.Units.largeSpacing * 2

    ColumnLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.mediumSpacing

            // User Icon Placeholder
            Rectangle {
                width: Kirigami.Units.iconSizes.medium
                height: width
                radius: width / 2
                color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2)
                
                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: Kirigami.Units.iconSizes.small
                    height: width
                    source: "user-identity"
                    color: Kirigami.Theme.highlightColor
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Controls.Label {
                    text: root.userName !== "" ? root.userName : i18n("Anonymous")
                    font.bold: true
                    Layout.fillWidth: true
                }

                Controls.Label {
                    text: root.dateStr
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                }
            }

            // Version Badge
            Rectangle {
                visible: root.version !== ""
                Layout.alignment: Qt.AlignTop
                color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)
                radius: 4
                implicitWidth: versionLabel.implicitWidth + Kirigami.Units.mediumSpacing
                implicitHeight: versionLabel.implicitHeight + Kirigami.Units.smallSpacing

                Controls.Label {
                    id: versionLabel
                    anchors.centerIn: parent
                    text: i18n("v%1", root.version)
                    font.pointSize: Kirigami.Theme.smallFont.pointSize * 0.9
                    color: Kirigami.Theme.textColor
                    opacity: 0.8
                }
            }
        }

        RowLayout {
            spacing: 2
            Layout.topMargin: Kirigami.Units.smallSpacing
            Repeater {
                model: 5
                Kirigami.Icon {
                    source: "rating"
                    width: Kirigami.Units.iconSizes.small
                    height: width
                    // ODRS ratings are out of 100
                    color: index < Math.round(root.rating / 20) ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                }
            }
        }

        Controls.Label {
            text: root.summary
            font.bold: true
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            visible: root.summary !== ""
        }

        Controls.Label {
            text: root.description
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            opacity: 0.9
            lineHeight: 1.2
            visible: root.description !== ""
        }
    }
}
