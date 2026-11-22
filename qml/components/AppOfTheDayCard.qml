// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Window
import "../style" as Styles
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

// Flathub's daily pick on the shared FeatureCard surface, so it has the same
// hover lift, accent border, shadow and glow as the other feature surfaces.
// Its screenshot sits in a pointer-following spotlight.
FeatureCard {
    id: root

    property string appName
    property string appSummary
    property string developer
    property url    iconUrl
    property url    screenshotUrl

    readonly property bool isNarrow: width < Kirigami.Units.gridUnit * 30
    readonly property bool hasScreenshot: String(root.screenshotUrl).length > 0
                                          && dailyScreenshot.status !== Image.Error

    implicitWidth: Kirigami.Units.gridUnit * 30
    implicitHeight: layout.implicitHeight + Kirigami.Units.largeSpacing * 4
    Layout.fillWidth: true

    activeFocusOnTab: true
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()

    Accessible.role: Accessible.Button
    Accessible.name: i18nc("@info:accessible daily pick card", "App of the Day: %1", root.appName)
    Accessible.description: root.appSummary
    Accessible.onPressAction: root.clicked()

    // Keyboard focus ring (hover is drawn by FeatureCard itself).
    Rectangle {
        anchors.fill: parent
        anchors.margins: -2
        radius: root.radius + 2
        color: "transparent"
        border.width: 2
        border.color: Kirigami.Theme.focusColor
        visible: root.activeFocus
    }

    ColumnLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 2
        spacing: Kirigami.Units.largeSpacing * 1.5

        // Screenshot under a spotlight that follows the pointer.
        Item {
            id: spotlight
            objectName: "dailySpotlight"
            Layout.fillWidth: true
            Layout.preferredHeight: root.hasScreenshot
                ? Math.min(width * 0.5, Kirigami.Units.gridUnit * 20) : 0
            visible: root.hasScreenshot
            clip: true

            property real lightX: pointer.hovered ? pointer.point.position.x : width * 0.65
            property real lightY: pointer.hovered ? pointer.point.position.y : height * 0.35
            Behavior on lightX { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
            Behavior on lightY { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
            onLightXChanged: glow.requestPaint()
            onLightYChanged: glow.requestPaint()
            HoverHandler { id: pointer }

            Canvas {
                id: glow
                anchors.fill: parent
                property color accent: root.accent
                // Brighter while the card is hovered.
                property real strength: root.hovered ? 0.55 : 0.3
                Behavior on strength { NumberAnimation { duration: Kirigami.Units.longDuration } }
                onStrengthChanged: requestPaint()
                onAccentChanged: requestPaint()
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: {
                    let ctx = getContext("2d")
                    ctx.clearRect(0, 0, width, height)
                    let radius = Math.max(width, height) * 0.7
                    let gradient = ctx.createRadialGradient(spotlight.lightX, spotlight.lightY, 0,
                                                           spotlight.lightX, spotlight.lightY, radius)
                    gradient.addColorStop(0, Qt.alpha(accent, strength))
                    gradient.addColorStop(0.5, Qt.alpha(accent, strength * 0.35))
                    gradient.addColorStop(1, Qt.alpha(accent, 0))
                    ctx.fillStyle = gradient
                    ctx.fillRect(0, 0, width, height)
                }
            }

            Image {
                id: dailyScreenshot
                objectName: "dailyScreenshot"
                anchors.centerIn: parent
                // Bound logical size by the decoded pixel size, including HiDPI.
                width: Math.min(parent.width * 0.85, implicitWidth / Screen.devicePixelRatio)
                height: Math.min(parent.height * 0.82, implicitHeight / Screen.devicePixelRatio)
                source: root.screenshotUrl
                sourceSize.width: 1600
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                mipmap: true
                visible: status === Image.Ready
                // Rises slightly with the card on hover.
                scale: root.hovered ? 1.03 : 1.0
                Behavior on scale { NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic } }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing * 1.5

            Image {
                id: dailyIcon
                Layout.preferredWidth: root.isNarrow ? Kirigami.Units.iconSizes.large : Kirigami.Units.iconSizes.huge
                Layout.preferredHeight: Layout.preferredWidth
                Layout.alignment: Qt.AlignVCenter
                source: root.iconUrl
                sourceSize: Qt.size(Layout.preferredWidth, Layout.preferredWidth)
                fillMode: Image.PreserveAspectFit
                mipmap: true
                asynchronous: true

                Kirigami.Icon {
                    anchors.fill: parent
                    source: "applications-all-symbolic"
                    fallback: "application-x-executable"
                    color: Kirigami.Theme.disabledTextColor
                    isMask: true
                    visible: dailyIcon.status !== Image.Ready
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                spacing: Kirigami.Units.smallSpacing

                // Eyebrow label in the accent colour, so the pick reads at a glance.
                Controls.Label {
                    Layout.fillWidth: true
                    text: i18nc("@info:label eyebrow on the daily pick card", "App of the Day")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.bold: true
                    font.capitalization: Font.AllUppercase
                    color: root.accent
                }

                Kirigami.Heading {
                    font.family: Styles.Style.fontFamily
                    Layout.fillWidth: true
                    level: 1
                    text: root.appName
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }

                Controls.Label {
                    Layout.fillWidth: true
                    visible: root.developer.length > 0
                    text: root.developer
                    textFormat: Text.PlainText
                    color: Kirigami.Theme.disabledTextColor
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }
            }

            Controls.Button {
                text: i18n("View Details")
                icon.name: "go-next-symbolic"
                highlighted: true
                visible: !root.isNarrow
                Layout.alignment: Qt.AlignVCenter
                // The whole card is already one tab stop.
                activeFocusOnTab: false
                onClicked: root.clicked()
            }
        }

        Controls.Label {
            Layout.fillWidth: true
            visible: text.length > 0
            text: root.appSummary
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
            opacity: 0.85
        }
    }
}
