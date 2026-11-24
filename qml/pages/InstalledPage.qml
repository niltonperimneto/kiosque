// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque
import "../components" as Components

// Installed applications manager: a dense list of rows with contextual
// actions (KDE HIG). Pending updates are handled on the Updates page; this
// page only links to it.
Kirigami.ScrollablePage {
    id: page
    title: L10n.i18n("Installed", [])
    // Typing (see Main.qml), Ctrl+F or the Search action pulls the filter bar
    // down; the list is filtered in place as the query changes.
    readonly property bool hasOwnSearch: true
    readonly property bool searchActive: searchBar.shown
    readonly property alias searchBar: searchBar

    function openSearch(typed) {
        searchBar.start(typed || "");
    }
    supportsRefreshing: true

    property var installedModel: applicationWindow().installedModel

    // Sorting state — drives installedModel.sortModel().
    property string sortCriterion: "name"
    property bool sortAscending: true

    header: Components.DropDownSearchBar {
        id: searchBar
        fieldObjectName: "installedFilterField"
        placeholderText: L10n.i18n("Filter installed…", [])
        onTextChanged: page.installedModel.applySearchFilter(text)
    }

    onSortCriterionChanged: installedModel.sortModel(sortCriterion, sortAscending)
    onSortAscendingChanged: installedModel.sortModel(sortCriterion, sortAscending)

    onRefreshingChanged: {
        if (refreshing) {
            installedModel.refresh();
            refreshing = false;
        }
    }

    // The model is shared with the Updates page; set and clear this page's filters.
    Component.onCompleted: {
        installedModel.applyShowUpdatesOnly(false);
        installedModel.applySearchFilter("");
        installedModel.sortModel(sortCriterion, sortAscending);
    }
    Component.onDestruction: {
        if (installedModel) {
            installedModel.applySearchFilter("");
        }
    }

    // ── Toolbar: refresh + sort + filter ───────────────────────────────────────
    actions: [
        Kirigami.Action {
            text: L10n.i18n("Search", [])
            icon.name: "search"
            displayHint: Kirigami.DisplayHint.IconOnly
            visible: !searchBar.shown
            onTriggered: page.openSearch("")
        },
        Kirigami.Action {
            text: L10n.i18n("Refresh", [])
            icon.name: "view-refresh"
            onTriggered: page.installedModel.refresh()
        },
        Kirigami.Action {
            text: L10n.i18n("Sort", [])
            icon.name: "view-sort"

            Kirigami.Action {
                text: L10n.i18n("Name", [])
                checkable: true
                checked: page.sortCriterion === "name"
                onTriggered: page.sortCriterion = "name"
            }
            Kirigami.Action {
                text: L10n.i18n("Size", [])
                checkable: true
                checked: page.sortCriterion === "size"
                onTriggered: page.sortCriterion = "size"
            }
            Kirigami.Action { separator: true }
            Kirigami.Action {
                text: L10n.i18n("Ascending", [])
                checkable: true
                checked: page.sortAscending
                onTriggered: page.sortAscending = true
            }
            Kirigami.Action {
                text: L10n.i18n("Descending", [])
                checkable: true
                checked: !page.sortAscending
                onTriggered: page.sortAscending = false
            }
        },
        Kirigami.Action {
            text: L10n.i18n("View", [])
            icon.name: "view-filter"

            Kirigami.Action {
                text: L10n.i18n("Show runtimes", [])
                checkable: true
                checked: page.installedModel && page.installedModel.show_runtimes
                onTriggered: page.installedModel.applyShowRuntimes(checked)
            }
        }
    ]

    // ── Installed list ─────────────────────────────────────────────────────────
    ListView {
        id: listView
        model: page.installedModel
        clip: true
        reuseItems: true

        // Loading overlay
        Controls.BusyIndicator {
            anchors.centerIn: parent
            running: page.installedModel && page.installedModel.loading
            visible: running
            z: 100
        }

        // Empty state
        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: listView.count === 0 && !(page.installedModel && page.installedModel.loading)
            text: page.installedModel && page.installedModel.installed_error ? L10n.i18n("Could not read installed applications", []) : L10n.i18n("No Flatpak Applications Installed", [])
            icon.name: "application-x-executable"
            explanation: page.installedModel && page.installedModel.installed_error ? page.installedModel.installed_error : L10n.i18n("Install applications from the Storefront tab", [])
        }

        // ── Section headers (Updates Available / Up to Date) ───────────────────
        section.property: "sectionGroup"
        section.criteria: ViewSection.FullString
        section.delegate: Kirigami.ListSectionHeader {
            width: ListView.view ? ListView.view.width : 0
            text: section === "updates" ? L10n.i18n("Updates Available", []) : L10n.i18n("Up to Date", [])
        }

        // ── Link to pending updates ────────────────────────────────────────────
        header: Item {
            width: listView.width
            height: updatesMessage.visible ? updatesMessage.implicitHeight : 0

            Kirigami.InlineMessage {
                id: updatesMessage
                objectName: "updatesAvailableMessage"
                width: parent.width
                position: Kirigami.InlineMessage.Position.Header
                type: Kirigami.MessageType.Information
                visible: page.installedModel && page.installedModel.updates_available_count > 0
                text: L10n.i18np("%1 update is available.", "%1 updates are available.", page.installedModel ? page.installedModel.updates_available_count : 0, [])
                actions: Kirigami.Action {
                    objectName: "reviewUpdatesAction"
                    text: L10n.i18n("Review Updates", [])
                    icon.name: "system-software-update"
                    onTriggered: applicationWindow().navigate("updates")
                }
            }
        }

        // ── App delegate ───────────────────────────────────────────────────────
        delegate: Kirigami.SwipeListItem {
            id: delegateItem
            width: ListView.view ? ListView.view.width : 0

            onClicked: applicationWindow().pushAppDetail(model.appId)

            readonly property bool uninstalling: page.installedModel !== null
                && page.installedModel.uninstalling_app_id !== ""
                && page.installedModel.uninstalling_app_id === model.appId

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.large
                    Layout.preferredHeight: Kirigami.Units.iconSizes.large
                    Layout.alignment: Qt.AlignVCenter
                    source: model.appId || "application-x-executable"
                    fallback: "application-x-executable"
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter
                    spacing: Kirigami.Units.smallSpacing / 2

                    Controls.Label {
                        Layout.fillWidth: true
                        text: model.name
                        elide: Text.ElideRight
                        font.weight: Font.Medium
                    }

                    Controls.Label {
                        Layout.fillWidth: true
                        text: model.appId
                        elide: Text.ElideRight
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                    }

                    Controls.Label {
                        objectName: "sizeLabel"
                        Layout.fillWidth: true
                        visible: !delegateItem.uninstalling && (model.version !== "" || model.size !== "")
                        text: {
                            let parts = [];
                            if (model.version !== "") parts.push(model.version);
                            if (model.size !== "") parts.push(model.size);
                            return parts.join("  ·  ");
                        }
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                        elide: Text.ElideRight
                    }

                    // Uninstall progress: indeterminate until the backend
                    // reports a fraction (uninstall_progress < 0).
                    Controls.ProgressBar {
                        objectName: "uninstallProgressBar"
                        Layout.fillWidth: true
                        visible: delegateItem.uninstalling
                        from: 0
                        to: 1
                        value: page.installedModel ? Math.max(0, page.installedModel.uninstall_progress) : 0
                        indeterminate: page.installedModel !== null && page.installedModel.uninstall_progress < 0
                    }
                }

                Controls.Button {
                    objectName: "cancelUninstallButton"
                    Layout.alignment: Qt.AlignVCenter
                    visible: delegateItem.uninstalling
                    text: L10n.i18n("Cancel", [])
                    icon.name: "dialog-cancel"
                    onClicked: page.installedModel.cancelUninstall()
                }
            }

            actions: [
                Kirigami.Action {
                    text: L10n.i18n("Open", [])
                    icon.name: "media-playback-start"
                    visible: !model.isRuntime
                    onTriggered: page.installedModel.launchApp(index)
                },
                Kirigami.Action {
                    text: L10n.i18n("Uninstall", [])
                    icon.name: "edit-delete"
                    enabled: !(page.installedModel && page.installedModel.uninstalling_app_id !== "")
                    onTriggered: page.installedModel.uninstallApp(index)
                }
            ]
        }
    }

}
