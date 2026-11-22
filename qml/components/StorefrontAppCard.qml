// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import com.kiosque
import "../style" as Styles
import org.kde.kirigami as Kirigami

// A clickable application tile: icon in a tinted frame, name and summary
// below, on a Kirigami.ShadowedRectangle surface. Sized entirely from
// Kirigami.Units so it scales with the user's font DPI; lifts subtly on hover.
Item {
    id: root

    // Data supplied by a ListView/GridView delegate over the Rust model.
    property string appName
    property string appSummary
    property url    iconUrl

    activeFocusOnTab: true
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()

    signal clicked()

    readonly property bool hovered: hoverArea.containsMouse

    Kirigami.Theme.inherit: false
    Kirigami.Theme.colorSet: Kirigami.Theme.View

    implicitWidth: Kirigami.Units.gridUnit * 10
    implicitHeight: Kirigami.Units.gridUnit * 12

    Accessible.role: Accessible.Button
    Accessible.name: root.appName
    Accessible.onPressAction: root.clicked()

    // ── Card surface ────────────────────────────────────────────────────
    Kirigami.ShadowedRectangle {
        id: surface
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor
        border.width: 1
        border.color: root.activeFocus ? Kirigami.Theme.highlightColor : Qt.alpha(Kirigami.Theme.textColor, 0.10)

        shadow.size: root.hovered ? Kirigami.Units.largeSpacing * 2 : Kirigami.Units.largeSpacing
        shadow.yOffset: root.hovered ? Kirigami.Units.smallSpacing : 2
        shadow.color: Qt.alpha("black", root.hovered ? 0.25 : 0.15)

        Behavior on shadow.size { NumberAnimation { duration: Styles.Style.hoverDuration } }
        Behavior on shadow.yOffset { NumberAnimation { duration: Styles.Style.hoverDuration } }

        Column {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing

            // Tinted frame so every icon sits on a consistent backdrop
            // regardless of its own shape or transparency.
            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Kirigami.Units.gridUnit * 5
                height: width
                radius: Kirigami.Units.cornerRadius
                color: Kirigami.Theme.alternateBackgroundColor

                RemoteImageView {
                    id: cardIcon
                    anchors.centerIn: parent
                    width: Kirigami.Units.iconSizes.large
                    height: width
                    remoteSource: root.iconUrl
                    sourceSize: Qt.size(width, height)
                    fillMode: Image.PreserveAspectFit
                    mipmap: true
                    asynchronous: true
                }

                // Generic application icon when the app has none (or it fails).
                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: Kirigami.Units.iconSizes.medium
                    height: width
                    source: "applications-all-symbolic"
                    fallback: "application-x-executable"
                    color: Kirigami.Theme.disabledTextColor
                    isMask: true
                    visible: cardIcon.status === Image.Null || cardIcon.status === Image.Error
                }
            }

            Kirigami.Heading {
                font.family: Styles.Style.fontFamily
                width: parent.width
                level: 4
                text: root.appName
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                maximumLineCount: 1
            }

            Text {
                width: parent.width
                text: root.appSummary
                textFormat: Text.PlainText
                color: Kirigami.Theme.disabledTextColor
                font: Kirigami.Theme.smallFont
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
            }
        }
    }

    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    // Scale from the centre so the lift reads symmetrically.
    transformOrigin: Item.Center
    scale: root.hovered ? Styles.Style.hoverScale : 1.0
    Behavior on scale { NumberAnimation { duration: Styles.Style.hoverDuration; easing.type: Styles.Style.hoverEasing } }
}
