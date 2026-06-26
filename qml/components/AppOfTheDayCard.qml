// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

// The "App of the Day" spotlight — a full-width feature surface built on the
// shared FeatureCard so it shares the exact same flair as the hero carousel.
FeatureCard {
    id: root

    property string cardAppId: ""
    property string cardName: ""
    property string cardSummary: ""
    property string cardIconUrl: ""

    readonly property bool isNarrow: width < Kirigami.Units.gridUnit * 32
    readonly property real iconDim: isNarrow ? Kirigami.Units.gridUnit * 4 : Kirigami.Units.gridUnit * 6

    implicitHeight: contentLayout.implicitHeight + Kirigami.Units.largeSpacing * 4

    RowLayout {
        id: contentLayout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 2
        spacing: Kirigami.Units.largeSpacing * 2

        Image {
            source: root.cardIconUrl
            Layout.preferredWidth: root.iconDim
            Layout.preferredHeight: root.iconDim
            Layout.alignment: Qt.AlignVCenter
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            sourceSize.width: root.iconDim
            sourceSize.height: root.iconDim

            Kirigami.Icon {
                anchors.fill: parent
                source: "applications-other"
                visible: parent.status !== Image.Ready
                opacity: 0.4
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: Kirigami.Units.smallSpacing

            // Eyebrow label — accent coloured so the spotlight reads instantly.
            Controls.Label {
                text: i18n("App of the Day")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                font.bold: true
                font.capitalization: Font.AllUppercase
                color: root.accent
                Layout.fillWidth: true
            }

            Kirigami.Heading {
                level: 1
                text: root.cardName
                color: Kirigami.Theme.textColor
                elide: Text.ElideRight
                maximumLineCount: 1
                Layout.fillWidth: true
            }

            Controls.Label {
                text: root.cardSummary
                color: Kirigami.Theme.textColor
                opacity: 0.85
                elide: Text.ElideRight
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                Layout.fillWidth: true
            }
        }

        Controls.Button {
            text: i18n("View Details")
            icon.name: "go-next-symbolic"
            highlighted: true
            visible: !root.isNarrow
            Layout.alignment: Qt.AlignVCenter
            onClicked: root.clicked()
        }
    }
}
