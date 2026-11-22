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
    property string subtitle
    property string iconName
    property int headingLevel: 2

    spacing: Kirigami.Units.smallSpacing

    RowLayout {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            visible: root.iconName.length > 0
            source: root.iconName
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            Kirigami.Heading {
                level: root.headingLevel
                text: root.title
                Layout.fillWidth: true
            }

            Controls.Label {
                visible: root.subtitle.length > 0
                text: root.subtitle
                color: Kirigami.Theme.disabledTextColor
                font: Kirigami.Theme.smallFont
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
        }
    }

    Item {
        Layout.fillWidth: true
        // Card height plus the breathing room the delegates reserve for the
        // hover lift and drop shadow.
        Layout.preferredHeight: Kirigami.Units.gridUnit * 12 + Kirigami.Units.largeSpacing * 2

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
                NumberAnimation { property: "opacity"; from: 0; to: 1.0; duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic }
                NumberAnimation { property: "scale"; from: 0.9; to: 1.0; duration: Kirigami.Units.longDuration; easing.type: Easing.OutBack }
            }
            displaced: Transition {
                NumberAnimation { properties: "x,y"; duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic }
            }

            // Padding to align with the rest of the page
            leftMargin: Kirigami.Units.largeSpacing
            rightMargin: Kirigami.Units.largeSpacing

            delegate: Item {
                id: delegateWrapper
                width: Kirigami.Units.gridUnit * 11
                height: listView.height

                required property var model
                readonly property var entry: model.modelData !== undefined ? model.modelData : model
                readonly property string appId: entry.appId || ""
                readonly property string name: entry.name || ""
                readonly property string summary: entry.summary || ""
                readonly property string iconUrl: entry.iconUrl || ""


                StorefrontAppCard {
                    anchors.fill: parent
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    anchors.bottomMargin: Kirigami.Units.largeSpacing
                    appName: delegateWrapper.name
                    appSummary: delegateWrapper.summary
                    iconUrl: delegateWrapper.iconUrl
                    onClicked: applicationWindow().pushAppDetail(delegateWrapper.appId)
                }
            }

            Behavior on contentX {
                enabled: !listView.moving && !listView.dragging
                NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic }
            }
        }

        // Left Navigation Arrow Button
        Controls.RoundButton {
            id: leftArrow
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.smallSpacing
            anchors.verticalCenter: parent.verticalCenter
            z: 10
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            icon.name: "go-previous-symbolic"
            visible: listView.contentX > 10
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
                let step = Kirigami.Units.gridUnit * 12;
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
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            icon.name: "go-next-symbolic"
            visible: listView.contentX < (listView.contentWidth - listView.width - 10)
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
                let step = Kirigami.Units.gridUnit * 12;
                let maxContentX = listView.contentWidth - listView.width;
                listView.contentX = Math.min(listView.contentX + step, maxContentX);
            }
        }
    }
}
