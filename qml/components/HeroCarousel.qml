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
            hoverEnabled: true

            Repeater {
                model: root.model

                delegate: Item {
                    id: slideDelegate

                    required property int index
                    required property string name
                    required property string summary
                    required property string iconUrl
                    required property string appId

                    // Shared flair via FeatureCard — same look as the spotlight.
                    FeatureCard {
                        id: card
                        anchors.fill: parent
                        // Small inset so the glow/shadow has room without clipping
                        // in the SwipeView, while staying aligned with other bands.
                        anchors.margins: Kirigami.Units.largeSpacing

                        // A full-bleed slide must not scale (it would clip in the
                        // SwipeView); the gradient/glow/hover-border still apply.
                        hoverScale: false
                        animateGlow: slideDelegate.SwipeView.isCurrentItem && root.visible

                        onClicked: applicationWindow().pushAppDetail(slideDelegate.appId)

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
                            highlighted: hovered

                            anchors.right: parent.right
                            anchors.rightMargin: Kirigami.Units.gridUnit * 4
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Kirigami.Units.largeSpacing * 2

                            onClicked: applicationWindow().pushAppDetail(slideDelegate.appId)
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

        // Left Navigation Arrow (standard RoundButton — matches HorizontalAppList)
        Controls.RoundButton {
            id: leftArrow
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.largeSpacing * 2 + Kirigami.Units.smallSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            icon.name: "go-previous-symbolic"
            visible: swipeView.count > 1 && !root.isMobile
            background: Rectangle {
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.backgroundColor.r,
                               Kirigami.Theme.backgroundColor.g,
                               Kirigami.Theme.backgroundColor.b,
                               leftArrow.hovered ? 0.85 : 0.55)
                border.width: 1
                border.color: leftArrow.hovered
                    ? Kirigami.Theme.highlightColor
                    : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
            }
            onClicked: {
                if (swipeView.currentIndex > 0) {
                    swipeView.currentIndex--;
                } else {
                    swipeView.currentIndex = swipeView.count - 1;
                }
            }
        }

        // Right Navigation Arrow (standard RoundButton — matches HorizontalAppList)
        Controls.RoundButton {
            id: rightArrow
            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.largeSpacing * 2 + Kirigami.Units.smallSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            icon.name: "go-next-symbolic"
            visible: swipeView.count > 1 && !root.isMobile
            background: Rectangle {
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.backgroundColor.r,
                               Kirigami.Theme.backgroundColor.g,
                               Kirigami.Theme.backgroundColor.b,
                               rightArrow.hovered ? 0.85 : 0.55)
                border.width: 1
                border.color: rightArrow.hovered
                    ? Kirigami.Theme.highlightColor
                    : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
            }
            onClicked: {
                if (swipeView.currentIndex < swipeView.count - 1) {
                    swipeView.currentIndex++;
                } else {
                    swipeView.currentIndex = 0;
                }
            }
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
        running: swipeView.count > 1 && root.visible
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
