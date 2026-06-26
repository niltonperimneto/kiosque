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

    property string searchQuery: ""
    property bool hasVisibleApps: {
        if (searchQuery.length === 0) return true;
        for (let i = 0; i < appGrid.children.length; i++) {
            let child = appGrid.children[i];
            if (child.isMatch === true) {
                return true;
            }
        }
        return false;
    }

    titleDelegate: Kirigami.SearchField {
        id: searchField
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        placeholderText: i18n("Search in %1…").arg(page.title)
        onTextChanged: page.searchQuery = text
    }
    
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
                    highlighted: {
                        if (page.categoryId === modelData.categoryId) return true;
                        if (page.categoryId.indexOf("-") === -1) {
                            let parts = modelData.categoryId.split("-");
                            if (parts[0] === page.categoryId && (parts[1] === "All" || parts[1] === "Game")) {
                                return true;
                            }
                        }
                        return false;
                    }
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
            visible: !page.categoryModel.loading && (appGridRepeater.count === 0 || !page.hasVisibleApps)
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

            visible: appGridRepeater.count > 0 && page.hasVisibleApps

            Repeater {
                id: appGridRepeater
                model: page.categoryModel
                onCountChanged: console.log("Category Repeater count changed to", count)
                delegate: Item {
                    id: delegateWrapper
                    
                    required property string appId
                    required property string name
                    required property string summary
                    required property string iconUrl

                    property bool isMatch: page.searchQuery.length === 0 || 
                                           name.toLowerCase().indexOf(page.searchQuery.toLowerCase()) !== -1 ||
                                           appId.toLowerCase().indexOf(page.searchQuery.toLowerCase()) !== -1 ||
                                           summary.toLowerCase().indexOf(page.searchQuery.toLowerCase()) !== -1

                    Layout.fillWidth: isMatch
                    Layout.preferredHeight: isMatch ? Kirigami.Units.gridUnit * 12 : 0
                    visible: isMatch

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
