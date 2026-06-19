// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

ColumnLayout {
    id: root
    property var appModel
    property string title

    spacing: Kirigami.Units.smallSpacing

    Kirigami.Heading {
        level: 3
        text: root.title
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
    }

    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 12

        ListView {
            id: listView
            anchors.fill: parent

            orientation: ListView.Horizontal
            model: root.appModel
            spacing: Kirigami.Units.largeSpacing
            clip: true

            Controls.BusyIndicator {
                anchors.centerIn: parent
                running: root.appModel && root.appModel.loading !== undefined ? root.appModel.loading : false
                visible: running
                z: 100
            }

            add: Transition {
                NumberAnimation { property: "opacity"; from: 0; to: 1.0; duration: 300; easing.type: Easing.OutCubic }
                NumberAnimation { property: "scale"; from: 0.9; to: 1.0; duration: 300; easing.type: Easing.OutBack }
            }
            displaced: Transition {
                NumberAnimation { properties: "x,y"; duration: 300; easing.type: Easing.OutCubic }
            }

            // Padding to align with the rest of the page
            leftMargin: Kirigami.Units.largeSpacing
            rightMargin: Kirigami.Units.largeSpacing

            delegate: Item {
                id: delegateWrapper
                width: Kirigami.Units.gridUnit * 16
                height: listView.height

                required property string appId
                required property string name
                required property string summary
                required property string iconUrl

                AppCard {
                    anchors.fill: parent
                    appId: delegateWrapper.appId
                    name: delegateWrapper.name
                    summary: delegateWrapper.summary
                    iconUrl: delegateWrapper.iconUrl
                }
            }

            Behavior on contentX {
                enabled: !listView.moving && !listView.dragging
                NumberAnimation { duration: 300; easing.type: Easing.OutCubic }
            }
        }

        // Left Navigation Arrow Button
        Item {
            id: leftArrow
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.mediumSpacing
            anchors.verticalCenter: parent.verticalCenter
            width: 32
            height: 32
            z: 10
            visible: listView.contentX > 10

            Rectangle {
                anchors.fill: parent
                radius: 16
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
                    width: 16
                    height: 16
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
                    let step = Kirigami.Units.gridUnit * 18;
                    listView.contentX = Math.max(listView.contentX - step, 0);
                }
            }

            scale: leftArrowMouse.containsMouse ? 1.05 : 1.0
            Behavior on scale { NumberAnimation { duration: 150 } }
        }

        // Right Navigation Arrow Button
        Item {
            id: rightArrow
            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.mediumSpacing
            anchors.verticalCenter: parent.verticalCenter
            width: 32
            height: 32
            z: 10
            visible: listView.contentX < (listView.contentWidth - listView.width - 10)

            Rectangle {
                anchors.fill: parent
                radius: 16
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
                    width: 16
                    height: 16
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
                    let step = Kirigami.Units.gridUnit * 18;
                    let maxContentX = listView.contentWidth - listView.width;
                    listView.contentX = Math.min(listView.contentX + step, maxContentX);
                }
            }

            scale: rightArrowMouse.containsMouse ? 1.05 : 1.0
            Behavior on scale { NumberAnimation { duration: 150 } }
        }
    }
}
