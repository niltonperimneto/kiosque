// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque
import "../components" as Components

Kirigami.ScrollablePage {
    id: page
    
    property string categoryId: ""
    property string categoryName: ""
    
    title: i18n(categoryName)
    
    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]
    
    // Independent model for this page to not conflict with global state
    property AppListModel categoryModel: AppListModel {}
    
    property var subcategories: applicationWindow().getSubcategories(categoryId.split("-")[0])

    Component.onCompleted: {
        loadData();
    }
    
    function loadData() {
        if (categoryId === "") {
            categoryModel.refresh();
        } else {
            categoryModel.loadCategory(categoryId);
        }
    }

    // ── Main Content ────────────────────────────────────────────────────
    ColumnLayout {
        width: page.width
        spacing: Kirigami.Units.largeSpacing

        // ── Subcategories List ──────────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0
            visible: subcategories.length > 0

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            ListView {
                id: subcatList
                Layout.fillWidth: true
                Layout.preferredHeight: contentItem.childrenRect.height + Kirigami.Units.largeSpacing * 2
                orientation: ListView.Horizontal
                model: subcategories
                
                leftMargin: Kirigami.Units.largeSpacing
                rightMargin: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing
                
                delegate: Controls.Button {
                    text: i18n(modelData.text)
                    icon.name: modelData.icon
                    flat: !highlighted
                    highlighted: page.categoryId === modelData.categoryId
                    onClicked: {
                        page.categoryId = modelData.categoryId;
                        page.loadData();
                    }
                }
            }
            
            Kirigami.Separator {
                Layout.fillWidth: true
            }
        }

        // Loading state
        Controls.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.gridUnit * 4
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 4
            running: page.categoryModel.loading
            visible: running
        }

        // Empty state
        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 8
            icon.name: "edit-find"
            text: i18n("No Applications Found")
            visible: !page.categoryModel.loading && appGridRepeater.count === 0
        }

        // App grid
        GridLayout {
            id: appGrid
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            
            property int minCellWidth: Kirigami.Units.gridUnit * 12
            columns: Math.max(1, Math.floor(width / minCellWidth))
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            visible: appGridRepeater.count > 0

            Repeater {
                id: appGridRepeater
                model: page.categoryModel
                onCountChanged: console.log("Category Repeater count changed to", count)
                delegate: Item {
                    id: delegateWrapper
                    Layout.fillWidth: true
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 12

                    required property string appId
                    required property string name
                    required property string summary
                    required property string iconUrl

                    Components.AppCard {
                        anchors.fill: parent
                        appId: delegateWrapper.appId
                        name: delegateWrapper.name
                        summary: delegateWrapper.summary
                        iconUrl: delegateWrapper.iconUrl
                    }
                }
            }
        }
        
        // Bottom spacing
        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing * 2
        }
    }
}
