// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Rectangle {
    id: root

    required property real progress
    required property string message
    property bool active: false

    visible: active
    color: Qt.rgba(0, 0, 0, 0.45)

    // Block interaction behind overlay
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onClicked: (mouse) => mouse.accepted = true
    }

    // ── Centered card ───────────────────────────────────────────────────
    Kirigami.ShadowedRectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - Kirigami.Units.gridUnit * 4,
                        Kirigami.Units.gridUnit * 22)
        height: contentColumn.implicitHeight + Kirigami.Units.largeSpacing * 4

        radius: 12
        color: Kirigami.Theme.backgroundColor

        shadow.size: 24
        shadow.color: Qt.rgba(0, 0, 0, 0.20)
        shadow.yOffset: 6

        ColumnLayout {
            id: contentColumn

            anchors {
                fill: parent
                margins: Kirigami.Units.largeSpacing * 2
            }
            spacing: Kirigami.Units.largeSpacing

            Kirigami.Heading {
                level: 4
                text: root.message
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }

            Controls.ProgressBar {
                id: progressBar

                Layout.fillWidth: true
                from: 0.0
                to: 1.0
                value: root.progress
                indeterminate: root.progress < 0.01

                background: Rectangle {
                    implicitWidth: 200
                    implicitHeight: 8
                    radius: height / 2
                    color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                }

                contentItem: Item {
                    implicitWidth: 200
                    implicitHeight: 8
                    
                    Kirigami.ShadowedRectangle {
                        width: progressBar.position * parent.width
                        height: parent.height
                        radius: height / 2
                        color: Kirigami.Theme.highlightColor
                        shadow.size: 12
                        shadow.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.6)
                        
                        // Pulsing effect
                        SequentialAnimation on opacity {
                            loops: Animation.Infinite
                            running: progressBar.value > 0.0 && progressBar.value < 1.0
                            NumberAnimation { to: 0.7; duration: 800; easing.type: Easing.InOutSine }
                            NumberAnimation { to: 1.0; duration: 800; easing.type: Easing.InOutSine }
                        }

                        // Shimmer sweep
                        Rectangle {
                            width: parent.height * 6
                            height: parent.height
                            radius: parent.radius
                            color: Qt.rgba(1, 1, 1, 0.3)
                            visible: progressBar.value > 0.0 && progressBar.value < 1.0
                            
                            NumberAnimation on x {
                                from: -parent.height * 6
                                to: progressBar.width + parent.height * 6
                                duration: 1500
                                loops: Animation.Infinite
                            }
                        }
                    }
                }

                Behavior on value {
                    NumberAnimation {
                        duration: 350
                        easing.type: Easing.OutCubic
                    }
                }
            }

            Controls.Label {
                text: root.progress >= 0.01
                      ? Math.round(root.progress * 100) + "%"
                      : i18n("Preparing…")
                Layout.alignment: Qt.AlignHCenter
                color: Kirigami.Theme.disabledTextColor
                font.pointSize: Kirigami.Theme.smallFont.pointSize
            }
        }
    }

    Behavior on opacity {
        NumberAnimation { duration: 200 }
    }
}
