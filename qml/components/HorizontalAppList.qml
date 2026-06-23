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
        Layout.preferredHeight: Kirigami.Units.gridUnit * 14

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
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    anchors.bottomMargin: Kirigami.Units.largeSpacing
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
        Controls.RoundButton {
            id: leftArrow
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.smallSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            icon.name: "go-previous-symbolic"
            visible: listView.contentX > 10
            background: Rectangle {
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.backgroundColor.r,
                               Kirigami.Theme.backgroundColor.g,
                               Kirigami.Theme.backgroundColor.b,
                               leftArrow.hovered ? 0.85 : 0.55)
                border.width: 1
                border.color: Qt.rgba(Kirigami.Theme.textColor.r,
                                      Kirigami.Theme.textColor.g,
                                      Kirigami.Theme.textColor.b, 0.15)
                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
            }
            onClicked: {
                let step = Kirigami.Units.gridUnit * 18;
                listView.contentX = Math.max(listView.contentX - step, 0);
            }
        }

        // Right Navigation Arrow Button
        Controls.RoundButton {
            id: rightArrow
            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.smallSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            icon.name: "go-next-symbolic"
            visible: listView.contentX < (listView.contentWidth - listView.width - 10)
            background: Rectangle {
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.backgroundColor.r,
                               Kirigami.Theme.backgroundColor.g,
                               Kirigami.Theme.backgroundColor.b,
                               rightArrow.hovered ? 0.85 : 0.55)
                border.width: 1
                border.color: Qt.rgba(Kirigami.Theme.textColor.r,
                                      Kirigami.Theme.textColor.g,
                                      Kirigami.Theme.textColor.b, 0.15)
                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
            }
            onClicked: {
                let step = Kirigami.Units.gridUnit * 18;
                let maxContentX = listView.contentWidth - listView.width;
                listView.contentX = Math.min(listView.contentX + step, maxContentX);
            }
        }
    }
}
