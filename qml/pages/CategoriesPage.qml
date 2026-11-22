// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Kirigami.Page {
    id: page
    title: i18n("Categories")

    padding: 0


    GridView {
        id: grid
        anchors.fill: parent
        clip: true
        cellWidth: Kirigami.Units.gridUnit * 12
        cellHeight: Kirigami.Units.gridUnit * 8

        topMargin: Kirigami.Units.largeSpacing
        bottomMargin: Kirigami.Units.largeSpacing
        leftMargin: Kirigami.Units.largeSpacing
        rightMargin: Kirigami.Units.largeSpacing

        Controls.ScrollBar.vertical: Controls.ScrollBar {}

        model: applicationWindow().categoriesModel

        delegate: Kirigami.Card {
            id: categoryCard

            // Entry of Categories.all: { text, icon, category }.
            required property var modelData

            width: grid.cellWidth - Kirigami.Units.largeSpacing
            height: grid.cellHeight - Kirigami.Units.largeSpacing
            Accessible.name: modelData.text

            // Allow clicking the entire card
            onClicked: {
                applicationWindow().currentSection = "categories";
                applicationWindow().currentCategory = categoryCard.modelData.category;
                applicationWindow().pageStack.push("qrc:/qml/pages/CategoryAppListPage.qml",
                    { categoryId: categoryCard.modelData.category, categoryName: categoryCard.modelData.text });
            }

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    source: categoryCard.modelData.icon
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                    Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                }

                Controls.Label {
                    text: categoryCard.modelData.text
                    font.bold: true
                    Layout.alignment: Qt.AlignHCenter
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }
    }
}
