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

    // The live SwipeView instance, recreated by swipeLoader on every model reset.
    // May be null briefly while the Loader rebuilds, so all access is null-guarded.
    readonly property Controls.SwipeView swipeView: swipeLoader.item as Controls.SwipeView

    spacing: Kirigami.Units.largeSpacing

    // A full model reset on a long-lived SwipeView + Repeater leaves pages stacked
    // at the same x-offset (Qt relayout bug). Rebuild the SwipeView from scratch
    // whenever the model resets so every page is positioned correctly.
    Connections {
        target: root.model
        function onModelReset() {
            swipeLoader.active = false; // destroy stale SwipeView + delegates
            swipeLoader.active = true;  // recreate fresh, correctly laid out
        }
    }

    // ── Carousel ────────────────────────────────────────────────────────
    Item {
        id: carouselContainer
        Layout.fillWidth: true
        Layout.preferredHeight: root.isMobile
                               ? Kirigami.Units.gridUnit * 18
                               : Kirigami.Units.gridUnit * 16

        Loader {
            id: swipeLoader
            anchors.fill: parent
            sourceComponent: swipeComponent
        }

        Component {
            id: swipeComponent

            Controls.SwipeView {
            id: view
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
                    required property string developer
                    required property string screenshotUrl
                    required property string brandColorLight
                    required property string brandColorDark

                    HeroBanner {
                        anchors.fill: parent

                        appName: slideDelegate.name
                        appSummary: slideDelegate.summary
                        iconUrl: slideDelegate.iconUrl
                        developer: slideDelegate.developer
                        screenshotUrl: slideDelegate.screenshotUrl
                        brandColorLight: slideDelegate.brandColorLight
                        brandColorDark: slideDelegate.brandColorDark

                        onClicked: applicationWindow().pushAppDetail(slideDelegate.appId)
                    }
                }
            }
            }
        }

        // Left Navigation Arrow (standard RoundButton — matches HorizontalAppList)
        Controls.RoundButton {
            id: leftArrow
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            icon.name: "go-previous-symbolic"
            visible: (root.swipeView ? root.swipeView.count : 0) > 1 && !root.isMobile
            // Overlaid on the banner, so only surface while the carousel is in use.
            opacity: (root.swipeView && root.swipeView.hovered) || leftArrow.hovered ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
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
                if (!root.swipeView) return;
                if (root.swipeView.currentIndex > 0) {
                    root.swipeView.currentIndex--;
                } else {
                    root.swipeView.currentIndex = root.swipeView.count - 1;
                }
            }
        }

        // Right Navigation Arrow (standard RoundButton — matches HorizontalAppList)
        Controls.RoundButton {
            id: rightArrow
            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.largeSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            icon.name: "go-next-symbolic"
            visible: (root.swipeView ? root.swipeView.count : 0) > 1 && !root.isMobile
            // Overlaid on the banner, so only surface while the carousel is in use.
            opacity: (root.swipeView && root.swipeView.hovered) || rightArrow.hovered ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
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
                if (!root.swipeView) return;
                if (root.swipeView.currentIndex < root.swipeView.count - 1) {
                    root.swipeView.currentIndex++;
                } else {
                    root.swipeView.currentIndex = 0;
                }
            }
        }
    }

    // ── Page indicator ──────────────────────────────────────────────────
    Controls.PageIndicator {
        id: indicator

        Layout.alignment: Qt.AlignHCenter
        count: root.swipeView ? root.swipeView.count : 0
        currentIndex: root.swipeView ? root.swipeView.currentIndex : 0
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
                        duration: Kirigami.Units.longDuration
                        easing.type: Easing.OutCubic
                    }
                }
                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
            }

            TapHandler {
                cursorShape: Qt.PointingHandCursor
                onTapped: {
                    if (root.swipeView) root.swipeView.currentIndex = index;
                }
            }
        }
    }

    // ── Auto-advance timer (paused while the pointer is over the carousel) ──
    Timer {
        interval: 5000
        running: (root.swipeView ? root.swipeView.count : 0) > 1 && root.visible
                 && !(root.swipeView && root.swipeView.hovered)
        repeat: true
        onTriggered: {
            if (!root.swipeView) return;
            if (root.swipeView.currentIndex < root.swipeView.count - 1) {
                root.swipeView.currentIndex++;
            } else {
                root.swipeView.currentIndex = 0;
            }
        }
    }
}
