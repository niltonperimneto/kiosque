// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Kirigami.ShadowedRectangle {
    id: root

    readonly property color accentColor: "#FF8C00"

    implicitWidth: mainLayout.implicitWidth + Kirigami.Units.largeSpacing * 2
    implicitHeight: mainLayout.implicitHeight + Kirigami.Units.largeSpacing * 2

    property string cardAppId: ""
    property string cardName: ""
    property string cardSummary: ""
    property string cardIconUrl: ""
    
    signal clicked()

    radius: 16
    color: "transparent"

    shadow.size: hoverHandler.hovered ? 16 : 10
    shadow.color: hoverHandler.hovered
        ? Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.22)
        : Qt.rgba(0, 0, 0, 0.12)
    shadow.yOffset: hoverHandler.hovered ? 5 : 3

    border.width: 1
    border.color: hoverHandler.hovered
        ? root.accentColor
        : Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.2)

    Behavior on shadow.size { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
    Behavior on shadow.yOffset { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
    Behavior on border.color { ColorAnimation { duration: 150 } }

    HoverHandler {
        id: hoverHandler
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: root.clicked()
    }

    scale: hoverHandler.hovered ? 1.02 : 1.0
    Behavior on scale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

    // ── Inner Gradient Background with clip for child glows ──
    Rectangle {
        anchors.fill: parent
        radius: parent.radius
        z: -2
        clip: true

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0.0
                color: Qt.rgba(Kirigami.Theme.alternateBackgroundColor.r, Kirigami.Theme.alternateBackgroundColor.g, Kirigami.Theme.alternateBackgroundColor.b, 0.75)
            }
            GradientStop {
                position: 0.5
                color: Qt.tint(Qt.rgba(Kirigami.Theme.alternateBackgroundColor.r, Kirigami.Theme.alternateBackgroundColor.g, Kirigami.Theme.alternateBackgroundColor.b, 0.75), Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.04))
            }
            GradientStop {
                position: 1.0
                color: Qt.tint(Qt.rgba(Kirigami.Theme.alternateBackgroundColor.r, Kirigami.Theme.alternateBackgroundColor.g, Kirigami.Theme.alternateBackgroundColor.b, 0.75), Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.16))
            }
        }

        // Decorative pulsing/ambient glow sphere in top-right
        Rectangle {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: -width / 4
            width: parent.width * 0.4
            height: width
            radius: width / 2
            z: -1

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
                loops: Animation.Infinite
                NumberAnimation { from: 0.5; to: 0.9; duration: 4000; easing.type: Easing.InOutSine }
                NumberAnimation { from: 0.9; to: 0.5; duration: 4000; easing.type: Easing.InOutSine }
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
    }

    ColumnLayout {
        id: mainLayout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            level: 4
            text: i18n("App of the Day")
            color: Kirigami.Theme.textColor
            font.bold: true
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop | Qt.AlignLeft
        }

        Item {
            // Spacer to push the app info to the bottom, aligning with the bottom of WelcomeCard
            Layout.fillHeight: true
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Image {
                source: root.cardIconUrl
                Layout.preferredWidth: Kirigami.Units.gridUnit * 5
                Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                Layout.alignment: Qt.AlignVCenter
                fillMode: Image.PreserveAspectFit
                sourceSize: Qt.size(width, height)
                
                Kirigami.Icon {
                    anchors.fill: parent
                    source: "applications-other"
                    visible: parent.status !== Image.Ready
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Heading {
                    level: 3
                    text: root.cardName
                    color: Kirigami.Theme.textColor
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Controls.Label {
                    text: root.cardSummary
                    color: Kirigami.Theme.textColor
                    opacity: 0.85
                    elide: Text.ElideRight
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    Layout.fillWidth: true
                }
            }
        }
    }
}
