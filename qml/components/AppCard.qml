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

    // Same accent as the feature surfaces so tiles read as the same family.
    readonly property color accent: Kirigami.Theme.highlightColor

    implicitWidth: Kirigami.Units.gridUnit * 12

    showClickFeedback: true
    hoverEnabled: true

    // Use the View colour set so the card surface is distinct from the
    // (Window-coloured) page background in both light and dark themes.
    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false

    // A lighter take on the FeatureCard language: same radius, accent and hover
    // border, with a subtle accent-tinted glow + lift so tiles feel alive on
    // hover without competing with the full feature surfaces.
    background: Kirigami.ShadowedRectangle {
        radius: Kirigami.Units.cornerRadius
        color: root.hovered
            ? Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.08))
            : Kirigami.Theme.backgroundColor
        border.width: 1
        border.color: root.hovered
            ? root.accent
            : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.12)
        shadow.size: root.hovered ? Kirigami.Units.gridUnit * 0.6 : Kirigami.Units.gridUnit * 0.4
        shadow.color: root.hovered
            ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.18)
            : Qt.rgba(0, 0, 0, 0.12)
        shadow.yOffset: root.hovered ? 3 : 1

        Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
        Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
        Behavior on shadow.size { NumberAnimation { duration: Kirigami.Units.shortDuration; easing.type: Easing.OutCubic } }
        Behavior on shadow.yOffset { NumberAnimation { duration: Kirigami.Units.shortDuration; easing.type: Easing.OutCubic } }
    }

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
