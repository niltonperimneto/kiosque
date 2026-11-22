// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque
import "../components" as Components

// Catalog-wide search, reachable from every navigation surface and Ctrl+F.
Kirigami.ScrollablePage {
    id: page
    objectName: "searchPage"

    title: L10n.i18n("Search", [])
    // Its title search field handles Ctrl+F (see Main.qml).
    readonly property bool hasOwnSearch: true

    // Initial query; the title field updates `queryText` as the user types.
    property string query: ""
    property string queryText: query
    readonly property string activeQuery: queryText.trim()

    // The title field lives in a delegate, outside this page's id scope.
    property Item searchField: null

    // A private model, so searching never disturbs other pages' lists.
    property AppListModel resultsModel: AppListModel {}

    function focusSearch() {
        if (page.searchField) {
            page.searchField.forceActiveFocus();
            page.searchField.selectAll();
        }
    }

    function runSearch() {
        if (page.activeQuery.length > 0) {
            resultsModel.search(page.activeQuery);
        }
    }

    titleDelegate: Kirigami.SearchField {
        id: searchField
        objectName: "globalSearchField"
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        placeholderText: L10n.i18n("Search applications…", [])
        text: page.query
        // Accept automatically, shortly after typing stops.
        delaySearch: true
        onTextChanged: page.queryText = text
        onAccepted: page.runSearch()
        Component.onCompleted: page.searchField = this
    }

    Component.onCompleted: {
        if (page.activeQuery.length > 0) {
            page.runSearch();
        }
        Qt.callLater(page.focusSearch);
    }

    ColumnLayout {
        width: page.width
        spacing: Kirigami.Units.largeSpacing

        Kirigami.PlaceholderMessage {
            objectName: "searchPromptMessage"
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.gridUnit * 4
            visible: page.activeQuery.length === 0
            icon.name: "search"
            text: L10n.i18n("Search for Applications", [])
            explanation: L10n.i18n("Type a name, a feature or a developer to search Flathub.", [])
        }

        Kirigami.Heading {
            level: 2
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            visible: page.activeQuery.length > 0 && resultsRepeater.count > 0
            wrapMode: Text.WordWrap
            text: L10n.i18np("%1 result for “%2”", "%1 results for “%2”", resultsRepeater.count, [page.activeQuery])
        }

        Controls.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.gridUnit * 4
            running: page.resultsModel.loading && resultsRepeater.count === 0
            visible: running
        }

        Kirigami.PlaceholderMessage {
            objectName: "searchErrorMessage"
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.gridUnit * 4
            visible: page.activeQuery.length > 0 && !page.resultsModel.loading
                     && page.resultsModel.error_message !== ""
            type: Kirigami.PlaceholderMessage.Type.Actionable
            icon.name: "network-disconnect"
            text: L10n.i18n("Search Is Unavailable", [])
            explanation: page.resultsModel.error_message
            helpfulAction: Kirigami.Action {
                icon.name: "view-refresh"
                text: L10n.i18n("Try Again", [])
                onTriggered: page.runSearch()
            }
        }

        Kirigami.PlaceholderMessage {
            objectName: "searchEmptyMessage"
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.gridUnit * 4
            visible: page.activeQuery.length > 0 && !page.resultsModel.loading
                     && page.resultsModel.error_message === "" && resultsRepeater.count === 0
            icon.name: "edit-find"
            text: L10n.i18n("No Applications Found", [])
            explanation: L10n.i18n("Try a different search term, or browse the categories.", [])
            helpfulAction: Kirigami.Action {
                icon.name: "applications-all-symbolic"
                text: L10n.i18n("Browse Categories", [])
                onTriggered: applicationWindow().openCategories()
            }
        }

        GridLayout {
            id: resultsGrid
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            visible: page.activeQuery.length > 0 && resultsRepeater.count > 0

            readonly property int minCellWidth: Kirigami.Units.gridUnit * 12
            columns: Math.max(1, Math.floor(width / minCellWidth))
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            Repeater {
                id: resultsRepeater
                model: page.resultsModel

                delegate: Item {
                    id: result

                    required property string appId
                    required property string name
                    required property string summary
                    required property string iconUrl

                    Layout.fillWidth: true
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 12

                    Components.StorefrontAppCard {
                        anchors.fill: parent
                        appName: result.name
                        appSummary: result.summary
                        iconUrl: result.iconUrl
                        onClicked: applicationWindow().pushAppDetail(result.appId)
                    }
                }
            }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing * 2
        }
    }
}
