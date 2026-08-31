// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

// Flathub's daily pick as a prominent vertical card: top-cropped hero
// screenshot as the header (title bar of the app always visible), then a
// clear vertical flow of logo, title and brief description. The stock
// Kirigami.Card banner center-crops, so the header is a custom TopCropImage;
// Card re-anchors any custom header edge-to-edge on its own.
Kirigami.Card {
    id: root

    property string appName
    property string appSummary
    property string developer
    property url    iconUrl
    property url    screenshotUrl

    showClickFeedback: true

    Accessible.role: Accessible.Button
    Accessible.name: root.appName
    Accessible.onPressAction: root.clicked()

    header: TopCropImage {
        source: root.screenshotUrl
        sourceSizeWidth: Kirigami.Units.gridUnit * 40
        visible: String(root.screenshotUrl).length > 0 && status !== Image.Error
        // 2:1 hero band derived from the card's width — no fixed pixels; the
        // band collapses entirely when there is no screenshot to show.
        implicitHeight: visible ? Math.round(root.width / 2) : 0
        color: Kirigami.Theme.alternateBackgroundColor

        corners.topLeftRadius: root.background.radius - root.background.border.width
        corners.topRightRadius: root.background.radius - root.background.border.width

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
                    text: i18nc("@info:label badge on the daily pick card", "App of the Day")
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

            Image {
                Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                source: root.iconUrl
                sourceSize: Qt.size(width, height)
                fillMode: Image.PreserveAspectFit
                mipmap: true
                asynchronous: true
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: root.appName
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }

                Controls.Label {
                    Layout.fillWidth: true
                    visible: root.developer.length > 0
                    text: root.developer
                    color: Kirigami.Theme.disabledTextColor
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }
            }
        }

        Controls.Label {
            Layout.fillWidth: true
            text: root.appSummary
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
    }
}
