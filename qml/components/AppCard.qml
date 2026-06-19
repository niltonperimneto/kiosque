// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    required property string name
    required property string summary
    required property string iconUrl
    required property string appId

    implicitWidth: Kirigami.Units.gridUnit * 12
    implicitHeight: cardContent.implicitHeight + Kirigami.Units.largeSpacing * 2 + Kirigami.Units.smallSpacing * 2

    // ── Card surface ────────────────────────────────────────────────────
    Kirigami.ShadowedRectangle {
        id: card

        anchors {
            fill: parent
            margins: Kirigami.Units.smallSpacing
        }

        radius: 8
        color: hoverHandler.hovered
            ? Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.06))
            : Kirigami.Theme.backgroundColor

        shadow.size: hoverHandler.hovered ? 12 : 8
        shadow.color: hoverHandler.hovered
            ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
            : Qt.rgba(0, 0, 0, 0.10)
        shadow.yOffset: hoverHandler.hovered ? 4 : 2

        border.width: 1
        border.color: hoverHandler.hovered
            ? Kirigami.Theme.highlightColor
            : Kirigami.Theme.disabledTextColor

        Behavior on color { ColorAnimation { duration: 150 } }
        Behavior on border.color { ColorAnimation { duration: 150 } }
        Behavior on shadow.color { ColorAnimation { duration: 150 } }
        Behavior on shadow.size {
            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
        }

        scale: hoverHandler.hovered ? 1.02 : 1.0
        Behavior on scale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

        // ── Content ─────────────────────────────────────────────────────
        ColumnLayout {
            id: cardContent

            anchors {
                fill: parent
                margins: Kirigami.Units.largeSpacing
            }
            spacing: Kirigami.Units.mediumSpacing

            Item {
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: 64
                Layout.preferredHeight: 64

                Image {
                    anchors.fill: parent
                    source: root.iconUrl
                    sourceSize.width: 64
                    sourceSize.height: 64
                    asynchronous: true
                    fillMode: Image.PreserveAspectFit

                    Kirigami.Icon {
                        anchors.centerIn: parent
                        width: 48
                        height: 48
                        source: "application-x-executable"
                        visible: parent.status !== Image.Ready
                        opacity: 0.3
                    }
                }
            }

            Kirigami.Heading {
                level: 4
                text: root.name
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                maximumLineCount: 1
            }

            Controls.Label {
                text: root.summary
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                color: Kirigami.Theme.disabledTextColor
                font.pointSize: Kirigami.Theme.smallFont.pointSize
            }
        }

        // ── Interaction ─────────────────────────────────────────────────
        HoverHandler {
            id: hoverHandler
            cursorShape: Qt.PointingHandCursor
        }

        TapHandler {
            onTapped: {
                applicationWindow().pushAppDetail(root.appId);
            }
        }
    }
}
