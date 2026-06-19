// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Kirigami.ShadowedRectangle {
    id: root

    property int star0: 0
    property int star1: 0
    property int star2: 0
    property int star3: 0
    property int star4: 0
    property int star5: 0
    property int total: 0

    property real averageRating: {
        if (total === 0) return 0.0;
        return ((star1 * 1) + (star2 * 2) + (star3 * 3) + (star4 * 4) + (star5 * 5)) / total;
    }

    radius: 12
    color: Kirigami.Theme.alternateBackgroundColor
    border.width: 1
    border.color: Qt.rgba(0, 0, 0, 0.05)
    implicitHeight: layout.implicitHeight + Kirigami.Units.largeSpacing * 2

    GridLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.largeSpacing
        columnSpacing: Kirigami.Units.gridUnit * 2
        
        readonly property bool isNarrow: root.width < Kirigami.Units.gridUnit * 22
        columns: isNarrow ? 1 : 2

        // Left side: Average & Total
        ColumnLayout {
            Layout.alignment: layout.isNarrow ? Qt.AlignHCenter : Qt.AlignVCenter
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: layout.isNarrow
            Layout.preferredWidth: layout.isNarrow ? -1 : Kirigami.Units.gridUnit * 8

            Controls.Label {
                text: root.averageRating.toFixed(1)
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 3.5
                font.bold: true
                Layout.alignment: Qt.AlignHCenter
            }

            RowLayout {
                spacing: 2
                Layout.alignment: Qt.AlignHCenter
                Repeater {
                    model: 5
                    Kirigami.Icon {
                        source: "rating"
                        width: Kirigami.Units.iconSizes.smallMedium
                        height: width
                        color: index < Math.round(root.averageRating) ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                    }
                }
            }

            Controls.Label {
                text: i18np("%1 Review", "%1 Reviews", root.total)
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.alignment: Qt.AlignHCenter
            }
        }

        // Right side: Bar Chart
        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: layout.isNarrow ? Qt.AlignHCenter : Qt.AlignVCenter
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: [
                    { label: "5", count: root.star5 },
                    { label: "4", count: root.star4 },
                    { label: "3", count: root.star3 },
                    { label: "2", count: root.star2 },
                    { label: "1", count: root.star1 }
                ]

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Controls.Label {
                        text: modelData.label
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        color: Kirigami.Theme.textColor
                        Layout.preferredWidth: implicitWidth
                    }

                    Kirigami.Icon {
                        source: "rating"
                        width: Kirigami.Units.iconSizes.small
                        height: width
                        color: Kirigami.Theme.textColor
                        opacity: 0.7
                    }

                    // The progress bar
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: Kirigami.Units.smallSpacing * 1.5
                        radius: height / 2
                        color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)

                        Rectangle {
                            width: root.total > 0 ? (modelData.count / root.total) * parent.width : 0
                            height: parent.height
                            radius: parent.radius
                            color: Kirigami.Theme.highlightColor
                        }
                    }
                }
            }
        }
    }
}
