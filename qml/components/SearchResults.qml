// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

// Search results shown in place of a page's own content: a grid of app cards
// with loading, empty and error states. The caller owns the model and runs
// the search; this only presents it.
ColumnLayout {
    id: root

    required property AppListModel model
    // The trimmed query the model was last asked for.
    property string query: ""
    readonly property int count: resultsRepeater.count

    spacing: Kirigami.Units.largeSpacing

    Kirigami.Heading {
        level: 2
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        visible: root.query.length > 0 && !root.model.loading && resultsRepeater.count > 0
        wrapMode: Text.WordWrap
        text: L10n.i18np("%1 result for “%2”", "%1 results for “%2”", resultsRepeater.count, [root.query])
    }

    Controls.BusyIndicator {
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: Kirigami.Units.gridUnit * 4
        running: root.query.length > 0 && root.model.loading && resultsRepeater.count === 0
        visible: running
    }

    Kirigami.PlaceholderMessage {
        objectName: "searchErrorMessage"
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.gridUnit * 4
        visible: root.query.length > 0 && !root.model.loading && root.model.error_message !== ""
        type: Kirigami.PlaceholderMessage.Type.Actionable
        icon.name: "network-disconnect"
        text: L10n.i18n("Search Is Unavailable", [])
        explanation: root.model.error_message
        helpfulAction: Kirigami.Action {
            icon.name: "view-refresh"
            text: L10n.i18n("Try Again", [])
            onTriggered: root.model.search(root.query)
        }
    }

    Kirigami.PlaceholderMessage {
        objectName: "searchEmptyMessage"
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.gridUnit * 4
        visible: root.query.length > 0 && !root.model.loading
                 && root.model.error_message === "" && resultsRepeater.count === 0
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
        objectName: "searchResultsGrid"
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        visible: root.query.length > 0 && resultsRepeater.count > 0

        readonly property int minCellWidth: Kirigami.Units.gridUnit * 12
        columns: Math.max(1, Math.floor(width / minCellWidth))
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.largeSpacing

        Repeater {
            id: resultsRepeater
            model: root.model

            delegate: Item {
                id: result

                required property string appId
                required property string name
                required property string summary
                required property string iconUrl

                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 12

                StorefrontAppCard {
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
