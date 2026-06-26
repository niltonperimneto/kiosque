// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque

// Installed applications manager.
//
// Layout follows the rest of the app (Kirigami.ScrollablePage, search in the
// title delegate, global controls in the toolbar) and the KDE HIG: a dense,
// editable list of rows with contextual actions — not discovery cards. Update
// handling is consolidated into a single "Pending Updates" card at the top of
// the list (à la Bazaar's library page) instead of being spread across a banner,
// header checkboxes and a multi-state footer.
Kirigami.ScrollablePage {
    id: page
    title: i18n("Installed")
    supportsRefreshing: true

    property var installedModel: applicationWindow().installedModel

    // Sorting state — drives installedModel.sortModel().
    property string sortCriterion: "name"
    property bool sortAscending: true

    // Filter state mirrored locally so the toolbar menu can show check marks.
    property bool showUpdatesOnly: false

    // Whether an update check has completed at least once this session, so we
    // don't claim "up to date" before the user has actually checked.
    property bool hasChecked: false

    titleDelegate: Kirigami.SearchField {
        id: searchField
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        placeholderText: i18n("Search installed…")
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

    Component.onCompleted: installedModel.sortModel(sortCriterion, sortAscending)

    // Mark hasChecked once a check finishes (checking_updates goes true → false).
    Connections {
        target: page.installedModel
        ignoreUnknownSignals: true
        function onChecking_updatesChanged() {
            if (page.installedModel && !page.installedModel.checking_updates) {
                page.hasChecked = true;
            }
        }
    }

    // ── Toolbar: refresh + sort + filter ───────────────────────────────────────
    actions: [
        Kirigami.Action {
            text: i18n("Refresh")
            icon.name: "view-refresh"
            onTriggered: page.installedModel.refresh()
        },
        Kirigami.Action {
            text: i18n("Sort")
            icon.name: "view-sort"

            Kirigami.Action {
                text: i18n("Name")
                checkable: true
                checked: page.sortCriterion === "name"
                onTriggered: page.sortCriterion = "name"
            }
            Kirigami.Action {
                text: i18n("Size")
                checkable: true
                checked: page.sortCriterion === "size"
                onTriggered: page.sortCriterion = "size"
            }
            Kirigami.Action { separator: true }
            Kirigami.Action {
                text: i18n("Ascending")
                checkable: true
                checked: page.sortAscending
                onTriggered: page.sortAscending = true
            }
            Kirigami.Action {
                text: i18n("Descending")
                checkable: true
                checked: !page.sortAscending
                onTriggered: page.sortAscending = false
            }
        },
        Kirigami.Action {
            text: i18n("Filter")
            icon.name: "view-filter"

            Kirigami.Action {
                text: i18n("Show platform & SDK updates")
                checkable: true
                checked: page.installedModel && page.installedModel.show_runtimes
                onTriggered: page.installedModel.applyShowRuntimes(checked)
            }
            Kirigami.Action {
                text: i18n("Updates only")
                checkable: true
                checked: page.showUpdatesOnly
                onTriggered: {
                    page.showUpdatesOnly = checked;
                    page.installedModel.applyShowUpdatesOnly(checked);
                }
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
            text: i18n("No Flatpak Applications Installed")
            icon.name: "application-x-executable"
            explanation: i18n("Install applications from the Storefront tab")
        }

        // ── Section headers (Updates Available / Up to Date) ───────────────────
        section.property: "sectionGroup"
        section.criteria: ViewSection.FullString
        section.delegate: Kirigami.ListSectionHeader {
            width: ListView.view ? ListView.view.width : 0
            text: section === "updates" ? i18n("Updates Available") : i18n("Up to Date")
        }

        // ── Pending Updates card ───────────────────────────────────────────────
        // Collapses to zero height when there is nothing to update. Owns the
        // batch update actions and the update-in-progress indicator.
        header: Item {
            width: listView.width
            implicitHeight: updatesCard.visible
                ? updatesCard.implicitHeight + Kirigami.Units.largeSpacing * 2
                : 0

            Kirigami.AbstractCard {
                id: updatesCard
                visible: page.installedModel && page.installedModel.updates_available_count > 0

                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: Kirigami.Units.largeSpacing

                readonly property bool busy: page.installedModel
                    && (page.installedModel.updating || page.installedModel.checking_updates)

                contentItem: ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing

                    // ── Update-in-progress indicator ───────────────────────────
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing / 2
                        visible: updatesCard.busy

                        Controls.ProgressBar {
                            Layout.fillWidth: true
                            value: page.installedModel ? page.installedModel.update_progress : 0
                            indeterminate: page.installedModel && page.installedModel.updating
                                && page.installedModel.update_progress <= 0.01
                        }

                        Controls.Label {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            font: Kirigami.Theme.smallFont
                            color: Kirigami.Theme.disabledTextColor
                            elide: Text.ElideRight
                            text: page.installedModel && page.installedModel.updating
                                ? (page.installedModel.update_status_text || i18n("Updating…"))
                                : i18n("Checking for updates…")
                        }
                    }

                    // ── Update actions ─────────────────────────────────────────
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing
                        visible: !updatesCard.busy

                        Kirigami.Heading {
                            Layout.fillWidth: true
                            level: 4
                            elide: Text.ElideRight
                            text: i18np("%1 update available", "%1 updates available",
                                        page.installedModel ? page.installedModel.updates_available_count : 0)
                        }

                        Controls.Button {
                            text: i18n("Update All")
                            icon.name: "system-software-update"
                            highlighted: true
                            onClicked: {
                                page.installedModel.setAllUpdatesChecked(true);
                                page.installedModel.updateSelectedApps();
                            }
                        }
                    }

                    // ── Selective update row ───────────────────────────────────
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing
                        visible: !updatesCard.busy

                        Controls.CheckBox {
                            text: i18nc("%1 = count", "Select all (%1)",
                                        page.installedModel ? page.installedModel.updates_available_count : 0)
                            tristate: true
                            checkState: {
                                if (!page.installedModel) return Qt.Unchecked;
                                const c = page.installedModel.updates_checked_count;
                                const a = page.installedModel.updates_available_count;
                                if (c === 0) return Qt.Unchecked;
                                if (c === a) return Qt.Checked;
                                return Qt.PartiallyChecked;
                            }
                            onClicked: {
                                const allChecked = page.installedModel.updates_checked_count
                                    === page.installedModel.updates_available_count;
                                page.installedModel.setAllUpdatesChecked(!allChecked);
                            }
                        }

                        Item { Layout.fillWidth: true }

                        Controls.Button {
                            text: i18nc("%1 = count of selected updates", "Update (%1)",
                                        page.installedModel ? page.installedModel.updates_checked_count : 0)
                            icon.name: "system-software-update"
                            enabled: page.installedModel && page.installedModel.updates_checked_count > 0
                            onClicked: page.installedModel.updateSelectedApps()
                        }
                    }
                }
            }
        }

        // ── App delegate ───────────────────────────────────────────────────────
        delegate: Kirigami.SwipeListItem {
            id: delegateItem
            width: ListView.view ? ListView.view.width : 0

            onClicked: applicationWindow().pushAppDetail(model.appId)

            // Uninstall progress fill drawn behind content.
            background: Rectangle {
                color: delegateItem.pressed ? Kirigami.Theme.highlightColor
                     : delegateItem.hovered ? Kirigami.Theme.alternateBackgroundColor
                     : Kirigami.Theme.backgroundColor

                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }

                Rectangle {
                    visible: page.installedModel && page.installedModel.uninstalling_app_id === model.appId
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: parent.width * (page.installedModel ? page.installedModel.uninstall_progress : 0.0)
                    color: Qt.rgba(
                        Kirigami.Theme.negativeTextColor.r,
                        Kirigami.Theme.negativeTextColor.g,
                        Kirigami.Theme.negativeTextColor.b,
                        0.2
                    )
                    Behavior on width { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                }
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                // Update selection checkbox — only on rows that have an update.
                Controls.CheckBox {
                    Layout.alignment: Qt.AlignVCenter
                    visible: model.hasUpdate
                    checked: model.isChecked
                    onToggled: page.installedModel.toggleUpdateChecked(index)
                }

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
                        Layout.fillWidth: true
                        visible: model.version !== "" || model.size !== ""
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
                }
            }

            actions: [
                Kirigami.Action {
                    text: i18n("Launch")
                    icon.name: "media-playback-start"
                    visible: !model.isRuntime
                    onTriggered: page.installedModel.launchApp(index)
                },
                Kirigami.Action {
                    text: i18n("Uninstall")
                    icon.name: "edit-delete"
                    onTriggered: page.installedModel.uninstallApp(index)
                }
            ]
        }
    }

    // ── Footer: persistent update-check control ────────────────────────────────
    footer: Controls.ToolBar {
        visible: page.installedModel !== null

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            // Indeterminate progress while checking (updating progress lives in
            // the Pending Updates card).
            ColumnLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing / 2
                visible: page.installedModel && page.installedModel.checking_updates

                Controls.ProgressBar {
                    Layout.fillWidth: true
                    indeterminate: true
                }

                Controls.Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    font: Kirigami.Theme.smallFont
                    color: Kirigami.Theme.disabledTextColor
                    text: i18n("Checking for updates…")
                }
            }

            // Idle row: status + check button.
            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                visible: page.installedModel
                         && !page.installedModel.checking_updates && !page.installedModel.updating

                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    visible: page.hasChecked && page.installedModel
                             && page.installedModel.updates_available_count === 0
                    source: "checkmark"
                    color: Kirigami.Theme.positiveTextColor
                    isMask: true
                }

                Controls.Label {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    color: Kirigami.Theme.disabledTextColor
                    text: {
                        if (page.installedModel && page.installedModel.updates_available_count > 0)
                            return i18n("Updates are available");
                        return page.hasChecked ? i18n("All applications are up to date")
                                               : i18n("Check for available application updates");
                    }
                }

                Controls.Button {
                    text: i18n("Check for Updates")
                    icon.name: "view-refresh"
                    onClicked: page.installedModel.checkForUpdates()
                }
            }
        }
    }
}
