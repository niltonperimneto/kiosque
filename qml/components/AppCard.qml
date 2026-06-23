// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// A clickable application tile: icon, name and a short summary.
// Built on Kirigami.AbstractCard so it inherits standard hover/press
// feedback, focus handling and theming per the KDE HIG.
Kirigami.AbstractCard {
    id: root

    required property string name
    required property string summary
    required property string iconUrl
    required property string appId

    implicitWidth: Kirigami.Units.gridUnit * 12

    showClickFeedback: true
    hoverEnabled: true

    onClicked: applicationWindow().pushAppDetail(root.appId)

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        Image {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Kirigami.Units.iconSizes.huge
            Layout.preferredHeight: Kirigami.Units.iconSizes.huge
            source: root.iconUrl
            sourceSize.width: Kirigami.Units.iconSizes.huge
            sourceSize.height: Kirigami.Units.iconSizes.huge
            asynchronous: true
            fillMode: Image.PreserveAspectFit

            Kirigami.Icon {
                anchors.centerIn: parent
                width: Kirigami.Units.iconSizes.large
                height: width
                source: "application-x-executable"
                visible: parent.status !== Image.Ready
                opacity: 0.3
            }
        }

        Kirigami.Heading {
            level: 4
            text: root.name
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            maximumLineCount: 1
        }

        Controls.Label {
            text: root.summary
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }
    }
}
