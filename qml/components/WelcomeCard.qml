// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import QtQuick.Effects

Kirigami.Card {
    id: root

    readonly property color accentColor: "#00A5CF"
    hoverEnabled: true

    // View colour set → surface distinct from the page background.
    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false

    header: Kirigami.Heading {
        level: 4
        text: i18n("Welcome to Kiosque")
        color: Kirigami.Theme.textColor
        font.bold: true
        padding: Kirigami.Units.largeSpacing
    }

    background: Kirigami.ShadowedRectangle {
        color: "transparent"
        radius: 16
        
        shadow.size: hoverHandler.hovered ? 16 : 10
        shadow.color: hoverHandler.hovered
            ? Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.22)
            : Qt.rgba(0, 0, 0, 0.12)
        shadow.yOffset: hoverHandler.hovered ? 5 : 3
        
        border.width: 1
        border.color: hoverHandler.hovered
            ? root.accentColor
            : Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.5)

        Behavior on shadow.size { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
        Behavior on shadow.yOffset { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
        Behavior on border.color { ColorAnimation { duration: 150 } }

        // ── Inner Gradient Background with clip for child glows ──
        Rectangle {
            id: innerBg
            anchors.fill: parent
            radius: parent.radius
            z: -2

            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop {
                    position: 0.0
                    color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.14))
                }
                GradientStop {
                    position: 0.5
                    color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.28))
                }
                GradientStop {
                    position: 1.0
                    color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.45))
                }
            }

            // Decorative pulsing/ambient glow sphere in top-right
            Item {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: -width / 4
                width: parent.width * 0.4
                height: width
                z: -1

                opacity: hoverHandler.hovered ? 1.0 : 0.6
                Behavior on opacity { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }

                Rectangle {
                    anchors.fill: parent
                    radius: width / 2

                    gradient: Gradient {
                        GradientStop {
                            position: 0.0
                            color: Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.2)
                        }
                        GradientStop {
                            position: 1.0
                            color: "transparent"
                        }
                    }

                    SequentialAnimation on opacity {
                        running: root.visible
                        loops: Animation.Infinite
                        NumberAnimation { from: 0.5; to: 0.9; duration: 4000; easing.type: Easing.InOutSine }
                        NumberAnimation { from: 0.9; to: 0.5; duration: 4000; easing.type: Easing.InOutSine }
                    }
                }
            }

            // Hover highlight overlay
            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                color: hoverHandler.hovered
                    ? Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.08)
                    : "transparent"
                Behavior on color { ColorAnimation { duration: 150 } }
            }

            // Mask item for MultiEffect to enforce rounded corners
            Rectangle {
                id: maskRect
                anchors.fill: parent
                radius: parent.radius
                color: "black"
                visible: false
            }

            layer.enabled: true
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: maskRect
            }
        }
    }

    HoverHandler {
        id: hoverHandler
        cursorShape: Qt.PointingHandCursor
    }

    scale: hoverHandler.hovered ? 1.02 : 1.0
    Behavior on scale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

    contentItem: Item {
        implicitHeight: layout.implicitHeight + Kirigami.Units.largeSpacing * 2
        implicitWidth: layout.implicitWidth + Kirigami.Units.largeSpacing * 2

        ColumnLayout {
            id: layout
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing

            Controls.Label {
                text: i18n("Discover, install, and update your applications effortlessly. Kiosque provides a modern, fast experience for managing your Flatpaks, inspired by Flathub.")
                color: Kirigami.Theme.textColor
                opacity: 0.85
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                Layout.fillHeight: true
                verticalAlignment: Text.AlignVCenter
            }
        }
    }
}
