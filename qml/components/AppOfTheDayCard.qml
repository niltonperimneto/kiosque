// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import com.kiosque
import QtQuick.Window
import "../style" as Styles
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Daily pick with a density-aware screenshot and a pointer-following spotlight.
Kirigami.Card {
    id: root

    property string appName
    property string appSummary
    property string developer
    property url    iconUrl
    property var screenshots: []
    readonly property var collageImages: screenshots.length > 0 ? screenshots.slice(0, 3)
                                          : (String(screenshotUrl).length > 0 ? [screenshotUrl] : [])
    property url    screenshotUrl

    showClickFeedback: true
    activeFocusOnTab: true
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()

    Accessible.role: Accessible.Button
    Accessible.name: root.appName
    Accessible.onPressAction: root.clicked()

    header: Item {
        id: spotlight
        objectName: "dailySpotlight"
        implicitHeight: Math.min(root.width * 0.55, Kirigami.Units.gridUnit * 22)
        clip: true
        property real lightX: pointer.hovered ? pointer.point.position.x : width * 0.65
        property real lightY: pointer.hovered ? pointer.point.position.y : height * 0.35
        Behavior on lightX { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
        Behavior on lightY { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
        onLightXChanged: glow.requestPaint()
        onLightYChanged: glow.requestPaint()
        HoverHandler { id: pointer }

        Rectangle {
            anchors.fill: parent
            color: Kirigami.Theme.alternateBackgroundColor
        }
        Canvas {
            id: glow
            anchors.fill: parent
            property color accent: Kirigami.Theme.highlightColor
            onAccentChanged: requestPaint()
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            onPaint: {
                let ctx = getContext("2d")
                ctx.clearRect(0, 0, width, height)
                let radius = Math.max(width, height) * 0.8
                let gradient = ctx.createRadialGradient(spotlight.lightX, spotlight.lightY, 0,
                                                       spotlight.lightX, spotlight.lightY, radius)
                gradient.addColorStop(0, Qt.alpha(accent, 0.65))
                gradient.addColorStop(0.45, Qt.alpha(accent, 0.24))
                gradient.addColorStop(1, Qt.alpha(accent, 0))
                ctx.fillStyle = gradient
                ctx.fillRect(0, 0, width, height)
            }
        }
        Repeater {
            model: root.collageImages
            delegate: Item {
                id: tile
                required property int index
                required property var modelData
                readonly property bool center: index === 0
                width: Math.min(spotlight.width * (center ? 0.64 : 0.48), Kirigami.Units.gridUnit * 34)
                height: spotlight.height * (center ? 0.8 : 0.65)
                x: (spotlight.width - width) / 2 + (index === 1 ? -spotlight.width * 0.26 : index === 2 ? spotlight.width * 0.26 : 0)
                   + (spotlight.lightX / Math.max(1, spotlight.width) - 0.5) * (center ? 12 : 24)
                y: (spotlight.height - height) / 2 + (index === 1 ? -12 : index === 2 ? 18 : 0)
                   + (spotlight.lightY / Math.max(1, spotlight.height) - 0.5) * (center ? 8 : 16)
                rotation: index === 1 ? -8 : index === 2 ? 8 : 0
                z: center ? 2 : 1
                scale: hover.hovered ? 1.035 : 1
                Behavior on scale { NumberAnimation { duration: 160 } }
                HoverHandler { id: hover }
                RemoteImageView {
                    objectName: tile.center ? "dailyScreenshot" : "dailyScreenshot" + tile.index
                    anchors.centerIn: parent
                    width: Math.min(parent.width, implicitWidth / Screen.devicePixelRatio)
                    height: Math.min(parent.height, implicitHeight / Screen.devicePixelRatio)
                    remoteSource: tile.modelData
                    sourceSize.width: 1600
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    mipmap: true
                    visible: status === Image.Ready
                }
            }
        }

        // "App of the Day" badge overlaid on the hero so the content column
        // below stays an uncluttered logo → title → description flow.
        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.margins: Kirigami.Units.largeSpacing
            radius: height / 2
            color: Qt.alpha("black", 0.6)
            implicitWidth: badgeRow.implicitWidth + Kirigami.Units.largeSpacing * 2
            implicitHeight: badgeRow.implicitHeight + Kirigami.Units.smallSpacing * 2

            Row {
                id: badgeRow
                anchors.centerIn: parent
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    source: "favorite-symbolic"
                    color: "white"
                    width: Kirigami.Units.iconSizes.small
                    height: width
                    anchors.verticalCenter: parent.verticalCenter
                }
                Controls.Label {
                    text: L10n.i18nc("@info:label badge on the daily pick card", "App of the Day", [])
                    color: "white"
                    font: Kirigami.Theme.smallFont
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
        }
    }

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            RemoteImageView {
                Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                remoteSource: root.iconUrl
                sourceSize: Qt.size(width, height)
                fillMode: Image.PreserveAspectFit
                mipmap: true
                asynchronous: true
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Kirigami.Heading {
                    font.family: Styles.Style.fontFamily
                    Layout.fillWidth: true
                    level: 2
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
        }

        Controls.Label {
            Layout.fillWidth: true
            text: root.appSummary
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
    }
}
