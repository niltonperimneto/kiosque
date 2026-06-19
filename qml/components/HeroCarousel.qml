// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

ColumnLayout {
    id: root

    required property var model

    readonly property bool isMobile: width < Kirigami.Units.gridUnit * 32

    spacing: Kirigami.Units.largeSpacing

    // ── Carousel ────────────────────────────────────────────────────────
    Item {
        id: carouselContainer
        Layout.fillWidth: true
        Layout.preferredHeight: root.isMobile
                               ? Kirigami.Units.gridUnit * 18
                               : Kirigami.Units.gridUnit * 14

        Controls.SwipeView {
            id: swipeView
            anchors.fill: parent
            clip: true

            Repeater {
                model: root.model

                delegate: Item {
                    id: slideDelegate

                    required property int index
                    required property string name
                    required property string summary
                    required property string iconUrl
                    required property string appId

                    Kirigami.ShadowedRectangle {
                        id: card
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing

                        radius: 16
                        color: "transparent"

                        // Glowing soft shadow using highlight color
                        shadow.size: hoverHandler.hovered ? 24 : 14
                        shadow.color: hoverHandler.hovered
                            ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.22)
                            : Qt.rgba(0, 0, 0, 0.12)
                        shadow.yOffset: hoverHandler.hovered ? 6 : 4

                        border.width: 1
                        border.color: hoverHandler.hovered
                            ? Kirigami.Theme.highlightColor
                            : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

                        Behavior on shadow.size { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                        Behavior on shadow.color { ColorAnimation { duration: 200 } }
                        Behavior on shadow.yOffset { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                        Behavior on border.color { ColorAnimation { duration: 200 } }

                        // ── Interaction ──
                        HoverHandler {
                            id: hoverHandler
                            cursorShape: Qt.PointingHandCursor
                        }

                        TapHandler {
                            onTapped: {
                                applicationWindow().pushAppDetail(slideDelegate.appId);
                            }
                        }

                        // ── Inner Gradient Background with clip for child glows ──
                        Rectangle {
                            anchors.fill: parent
                            radius: card.radius
                            z: -2
                            clip: true

                            gradient: Gradient {
                                orientation: Gradient.Horizontal
                                GradientStop {
                                    position: 0.0
                                    color: Kirigami.Theme.alternateBackgroundColor
                                }
                                GradientStop {
                                    position: 0.5
                                    color: Qt.tint(Kirigami.Theme.alternateBackgroundColor, Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.04))
                                }
                                GradientStop {
                                    position: 1.0
                                    color: Qt.tint(Kirigami.Theme.alternateBackgroundColor, Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.16))
                                }
                            }

                            // Decorative soft ambient glow in top-right
                            Rectangle {
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: -width / 3
                                width: parent.width * 0.45
                                height: width
                                radius: width / 2
                                z: -1

                                gradient: Gradient {
                                    GradientStop {
                                        position: 0.0
                                        color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                                    }
                                    GradientStop {
                                        position: 1.0
                                        color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.0)
                                    }
                                }

                                SequentialAnimation on opacity {
                                    loops: Animation.Infinite
                                    NumberAnimation { from: 0.6; to: 1.0; duration: 4000; easing.type: Easing.InOutSine }
                                    NumberAnimation { from: 1.0; to: 0.6; duration: 4000; easing.type: Easing.InOutSine }
                                }
                            }

                            // Hover highlight hue overlay
                            Rectangle {
                                anchors.fill: parent
                                radius: parent.radius
                                color: hoverHandler.hovered
                                    ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.08)
                                    : "transparent"
                                Behavior on color { ColorAnimation { duration: 150 } }
                            }
                        }


                        // ── Content Layout ──

                        // Ambient halo glow behind the app icon
                        Rectangle {
                            anchors.centerIn: appIcon
                            width: appIcon.width * 2.2
                            height: width
                            radius: width / 2
                            z: 0

                            gradient: Gradient {
                                GradientStop {
                                    position: 0.0
                                    color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.12)
                                }
                                GradientStop {
                                    position: 1.0
                                    color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.0)
                                }
                            }

                            // Pulsing animation for the halo
                            SequentialAnimation on opacity {
                                loops: Animation.Infinite
                                NumberAnimation { from: 0.7; to: 1.0; duration: 3000; easing.type: Easing.InOutSine }
                                NumberAnimation { from: 1.0; to: 0.7; duration: 3000; easing.type: Easing.InOutSine }
                            }
                        }

                        // App Icon (anchored left, centered vertically, clearing left arrow)
                        Image {
                            id: appIcon
                            z: 1
                            source: slideDelegate.iconUrl
                            sourceSize.width: 120
                            sourceSize.height: 120
                            width: 120
                            height: 120
                            anchors.left: parent.left
                            anchors.leftMargin: Kirigami.Units.gridUnit * 4.5
                            anchors.verticalCenter: parent.verticalCenter
                            asynchronous: true
                            fillMode: Image.PreserveAspectFit

                            // Placeholder while loading
                            Kirigami.Icon {
                                anchors.centerIn: parent
                                width: 80
                                height: 80
                                source: "application-x-executable"
                                visible: parent.status !== Image.Ready
                                opacity: 0.4
                            }
                        }

                        // App Info Text (aligned next to icon, with bottom space reserved for button)
                        ColumnLayout {
                            id: textContainer
                            anchors.left: appIcon.right
                            anchors.leftMargin: Kirigami.Units.largeSpacing * 2.5
                            anchors.right: parent.right
                            anchors.rightMargin: Kirigami.Units.gridUnit * 4.5
                            anchors.top: parent.top
                            anchors.topMargin: Kirigami.Units.largeSpacing * 2
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Kirigami.Units.largeSpacing * 2 + viewDetailsButton.implicitHeight + Kirigami.Units.smallSpacing
                            spacing: Kirigami.Units.smallSpacing

                            Kirigami.Heading {
                                id: appNameHeading
                                level: 2
                                text: slideDelegate.name
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                maximumLineCount: 1
                            }

                            Controls.Label {
                                id: appSummaryLabel
                                text: slideDelegate.summary
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                opacity: 0.75
                            }
                        }

                        // View Details Button (anchored to bottom-right of the card, clearing right arrow)
                        Controls.Button {
                            id: viewDetailsButton
                            text: i18n("View Details")
                            icon.name: "go-next-symbolic"
                            highlighted: true

                            anchors.right: parent.right
                            anchors.rightMargin: Kirigami.Units.gridUnit * 4
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Kirigami.Units.largeSpacing * 2

                            onClicked: {
                                applicationWindow().pushAppDetail(slideDelegate.appId);
                            }
                        }

                        states: [
                            State {
                                name: "mobile"
                                when: root.isMobile
                                PropertyChanges {
                                    target: appIcon
                                    width: 64
                                    height: 64
                                    sourceSize.width: 64
                                    sourceSize.height: 64
                                }
                                AnchorChanges {
                                    target: appIcon
                                    anchors.left: undefined
                                    anchors.verticalCenter: undefined
                                    anchors.horizontalCenter: card.horizontalCenter
                                    anchors.top: card.top
                                }
                                PropertyChanges {
                                    target: appIcon
                                    anchors.topMargin: Kirigami.Units.largeSpacing * 1.5
                                    anchors.leftMargin: 0
                                }
                                AnchorChanges {
                                    target: textContainer
                                    anchors.left: card.left
                                    anchors.right: card.right
                                    anchors.top: appIcon.bottom
                                    anchors.bottom: viewDetailsButton.top
                                }
                                PropertyChanges {
                                    target: textContainer
                                    anchors.leftMargin: Kirigami.Units.largeSpacing
                                    anchors.rightMargin: Kirigami.Units.largeSpacing
                                    anchors.topMargin: Kirigami.Units.mediumSpacing
                                    anchors.bottomMargin: Kirigami.Units.mediumSpacing
                                }
                                PropertyChanges {
                                    target: appNameHeading
                                    horizontalAlignment: Text.AlignHCenter
                                }
                                PropertyChanges {
                                    target: appSummaryLabel
                                    horizontalAlignment: Text.AlignHCenter
                                }
                                AnchorChanges {
                                    target: viewDetailsButton
                                    anchors.right: undefined
                                    anchors.horizontalCenter: card.horizontalCenter
                                    anchors.bottom: card.bottom
                                }
                                PropertyChanges {
                                    target: viewDetailsButton
                                    anchors.bottomMargin: Kirigami.Units.largeSpacing
                                    anchors.rightMargin: 0
                                }

                            }
                        ]

                        transitions: [
                            Transition {
                                from: "*"
                                to: "*"
                                AnchorAnimation { duration: 250; easing.type: Easing.OutCubic }
                                NumberAnimation {
                                    properties: "width,height,sourceSize.width,sourceSize.height,anchors.topMargin,anchors.leftMargin,anchors.rightMargin,anchors.bottomMargin"
                                    duration: 250
                                    easing.type: Easing.OutCubic
                                }
                            }
                        ]
                    }
                }
            }
        }

        // Left Navigation Arrow Button (Square with MouseArea and wrap-around)
        Item {
            id: leftArrow
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.mediumSpacing
            anchors.verticalCenter: parent.verticalCenter
            width: 44
            height: 44
            z: 10
            visible: swipeView.count > 1 && !root.isMobile

            Rectangle {
                anchors.fill: parent
                radius: 6
                color: leftArrowMouse.containsMouse
                    ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25)
                    : Qt.rgba(Kirigami.Theme.backgroundColor.r, Kirigami.Theme.backgroundColor.g, Kirigami.Theme.backgroundColor.b, 0.7)
                border.width: 1
                border.color: leftArrowMouse.containsMouse
                    ? Kirigami.Theme.highlightColor
                    : Qt.rgba(Kirigami.Theme.disabledTextColor.r, Kirigami.Theme.disabledTextColor.g, Kirigami.Theme.disabledTextColor.b, 0.3)

                Behavior on color { ColorAnimation { duration: 150 } }
                Behavior on border.color { ColorAnimation { duration: 150 } }

                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: 24
                    height: 24
                    source: "go-previous-symbolic"
                    color: Kirigami.Theme.textColor
                }
            }

            MouseArea {
                id: leftArrowMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    if (swipeView.currentIndex > 0) {
                        swipeView.currentIndex--;
                    } else {
                        swipeView.currentIndex = swipeView.count - 1;
                    }
                }
            }

            scale: leftArrowMouse.containsMouse ? 1.05 : 1.0
            Behavior on scale { NumberAnimation { duration: 150 } }
        }

        // Right Navigation Arrow Button (Square with MouseArea and wrap-around)
        Item {
            id: rightArrow
            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.mediumSpacing
            anchors.verticalCenter: parent.verticalCenter
            width: 44
            height: 44
            z: 10
            visible: swipeView.count > 1 && !root.isMobile

            Rectangle {
                anchors.fill: parent
                radius: 6
                color: rightArrowMouse.containsMouse
                    ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25)
                    : Qt.rgba(Kirigami.Theme.backgroundColor.r, Kirigami.Theme.backgroundColor.g, Kirigami.Theme.backgroundColor.b, 0.7)
                border.width: 1
                border.color: rightArrowMouse.containsMouse
                    ? Kirigami.Theme.highlightColor
                    : Qt.rgba(Kirigami.Theme.disabledTextColor.r, Kirigami.Theme.disabledTextColor.g, Kirigami.Theme.disabledTextColor.b, 0.3)

                Behavior on color { ColorAnimation { duration: 150 } }
                Behavior on border.color { ColorAnimation { duration: 150 } }

                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: 24
                    height: 24
                    source: "go-next-symbolic"
                    color: Kirigami.Theme.textColor
                }
            }

            MouseArea {
                id: rightArrowMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    if (swipeView.currentIndex < swipeView.count - 1) {
                        swipeView.currentIndex++;
                    } else {
                        swipeView.currentIndex = 0;
                    }
                }
            }

            scale: rightArrowMouse.containsMouse ? 1.05 : 1.0
            Behavior on scale { NumberAnimation { duration: 150 } }
        }
    }

    // ── Page indicator ──────────────────────────────────────────────────
    Controls.PageIndicator {
        id: indicator

        Layout.alignment: Qt.AlignHCenter
        count: swipeView.count
        currentIndex: swipeView.currentIndex
        interactive: false // Disabled internal interaction to avoid breaking the QML binding

        delegate: Item {
            required property int index

            // Larger target container for easy clicking
            implicitWidth: index === indicator.currentIndex
                           ? Kirigami.Units.smallSpacing * 7
                           : Kirigami.Units.smallSpacing * 4
            implicitHeight: Kirigami.Units.gridUnit * 1.5

            Rectangle {
                anchors.centerIn: parent

                width: index === indicator.currentIndex
                       ? Kirigami.Units.smallSpacing * 5
                       : Kirigami.Units.smallSpacing * 2
                height: Kirigami.Units.smallSpacing * 2
                radius: implicitHeight / 2

                color: index === indicator.currentIndex
                       ? Kirigami.Theme.highlightColor
                       : Kirigami.Theme.disabledTextColor
                opacity: index === indicator.currentIndex ? 1.0 : 0.45

                Behavior on width {
                    NumberAnimation {
                        duration: 250
                        easing.type: Easing.OutCubic
                    }
                }
                Behavior on color { ColorAnimation { duration: 150 } }
                Behavior on opacity { NumberAnimation { duration: 150 } }
            }

            TapHandler {
                cursorShape: Qt.PointingHandCursor
                onTapped: {
                    swipeView.currentIndex = index;
                }
            }
        }
    }

    // ── Auto-advance timer ──────────────────────────────────────────────
    Timer {
        interval: 5000
        running: swipeView.count > 1
        repeat: true
        onTriggered: {
            if (swipeView.currentIndex < swipeView.count - 1) {
                swipeView.currentIndex++;
            } else {
                swipeView.currentIndex = 0;
            }
        }
    }
}
