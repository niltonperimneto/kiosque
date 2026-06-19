// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Kirigami.ScrollablePage {
    id: page
    title: i18n("Categories")

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]

    Kirigami.CardsGridView {
        id: grid
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        cellWidth: Kirigami.Units.gridUnit * 12
        cellHeight: Kirigami.Units.gridUnit * 8

        model: applicationWindow().categoriesModel

        delegate: Kirigami.Card {
            width: grid.cellWidth - Kirigami.Units.largeSpacing
            height: grid.cellHeight - Kirigami.Units.largeSpacing
            
            // Allow clicking the entire card
            onClicked: {
                applicationWindow().currentSection = "categories";
                applicationWindow().currentCategory = model.category;
                
                if (model.category === "") {
                    applicationWindow().pageStack.push("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "", categoryName: model.text });
                } else {
                    applicationWindow().pageStack.push("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: model.category, categoryName: model.text });
                }
            }

            contentItem: ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    source: model.icon
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                    Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                }

                Controls.Label {
                    text: i18n(model.text)
                    font.bold: true
                    Layout.alignment: Qt.AlignHCenter
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }
    }
}
