// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque

Kirigami.Page {
    id: page
    title: i18n("Installed")
    property var installedModel: applicationWindow().installedModel

    // Sorting state
    property string sortCriterion: "name"
    property bool sortAscending: true

    // Whether an update check has completed at least once this session, so we
    // don't claim "up to date" before the user has actually checked.
    property bool hasChecked: false

    onSortCriterionChanged: installedModel.sortModel(sortCriterion, sortAscending)
    onSortAscendingChanged: installedModel.sortModel(sortCriterion, sortAscending)

    Component.onCompleted: {
        installedModel.sortModel(sortCriterion, sortAscending);
    }

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

    actions: [
        Kirigami.Action {
            text: i18n("Refresh")
            icon.name: "view-refresh"
            onTriggered: page.installedModel.refresh()
        }
    ]

    // ── Loading overlay ────────────────────────────────────────────────────────
    Controls.BusyIndicator {
        anchors.centerIn: parent
        running: page.installedModel && page.installedModel.loading
        visible: running
        z: 100
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        visible: listView.count === 0 && !(page.installedModel && page.installedModel.loading)
        text: i18n("No Flatpak Applications Installed")
        icon.name: "application-x-executable"
        explanation: i18n("Install applications from the Storefront tab")
    }

    ListView {
        id: listView
        anchors.fill: parent
        model: page.installedModel
        clip: true

        // Width available to content, reserving room for the scrollbar when it
        // is actually shown so rows don't slide underneath it.
        readonly property real availableWidth: width -
            (verticalScrollBar.visible && verticalScrollBar.size < 1.0 ? verticalScrollBar.width : 0)

        // ── Section headers (Updates Available / Up to Date) ───────────────────
        section.property: "sectionGroup"
        section.criteria: ViewSection.FullString
        section.delegate: Kirigami.ListSectionHeader {
            width: listView ? listView.availableWidth : 0
            text: {
                if (section === "updates") return i18n("Updates Available");
                return i18n("Up to Date");
            }
        }

        // ── List header: search + sort controls ────────────────────────────────
        header: ColumnLayout {
            width: listView ? listView.availableWidth : 0
            spacing: 0

            // Updates available banner
            Kirigami.InlineMessage {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing
                Layout.bottomMargin: 0
                visible: page.installedModel && page.installedModel.updates_available_count > 0
                         && !page.installedModel.checking_updates && !page.installedModel.updating
                type: Kirigami.MessageType.Positive
                text: i18nc("%1 = number of updates", "%1 update(s) available", page.installedModel ? page.installedModel.updates_available_count : 0)
                showCloseButton: false
                actions: [
                    Kirigami.Action {
                        text: i18n("Update All")
                        icon.name: "system-software-update"
                        enabled: page.installedModel && page.installedModel.updates_available_count > 0
                        onTriggered: {
                            page.installedModel.setAllUpdatesChecked(true);
                            page.installedModel.updateSelectedApps();
                        }
                    }
                ]
            }

            // Search + sort row
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing

                Kirigami.SearchField {
                    id: searchField
                    Layout.fillWidth: true
                    placeholderText: i18n("Search installed…")
                    onTextChanged: page.installedModel.applySearchFilter(text)
                }

                Controls.Label {
                    text: i18n("Sort:")
                    color: Kirigami.Theme.textColor
                }

                Controls.ComboBox {
                    id: sortCombo
                    implicitWidth: Kirigami.Units.gridUnit * 7
                    model: [i18n("Name"), i18n("Size")]
                    currentIndex: 0
                    onCurrentIndexChanged: {
                        page.sortCriterion = currentIndex === 0 ? "name" : "size";
                    }
                }

                Controls.ToolButton {
                    icon.name: page.sortAscending ? "view-sort-ascending" : "view-sort-descending"
                    onClicked: page.sortAscending = !page.sortAscending
                    Controls.ToolTip.text: page.sortAscending ? i18n("Ascending") : i18n("Descending")
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
            }

            // Platform / runtime updates toggle row
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.smallSpacing
                spacing: Kirigami.Units.smallSpacing
                visible: page.installedModel && page.installedModel.updates_available_count > 0

                Controls.CheckBox {
                    id: showRuntimesCheck
                    text: i18n("Show platform & SDK updates")
                    checked: page.installedModel && page.installedModel.show_runtimes
                    onToggled: page.installedModel.applyShowRuntimes(checked)
                }

                Item { Layout.fillWidth: true }

                Controls.CheckBox {
                    id: updatesOnlyCheck
                    text: i18n("Updates only")
                    onToggled: page.installedModel.applyShowUpdatesOnly(checked)
                }
            }
        }

        // ── App delegate ───────────────────────────────────────────────────────
        delegate: Kirigami.SwipeListItem {
            id: delegateItem
            width: listView ? listView.availableWidth : 0

            onClicked: applicationWindow().pushAppDetail(model.appId)

            // Uninstall progress fill drawn behind content
            background: Rectangle {
                color: delegateItem.pressed ? Kirigami.Theme.highlightColor
                     : delegateItem.hovered ? Kirigami.Theme.alternateBackgroundColor
                     : Kirigami.Theme.backgroundColor

                Behavior on color { ColorAnimation { duration: 100 } }

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

                // App icon
                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.large
                    Layout.preferredHeight: Kirigami.Units.iconSizes.large
                    Layout.alignment: Qt.AlignVCenter
                    source: model.appId || "application-x-executable"
                    fallback: "application-x-executable"
                }

                // Text column
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter
                    spacing: Kirigami.Units.smallSpacing / 2

                    // App name — primary text
                    Controls.Label {
                        Layout.fillWidth: true
                        text: model.name
                        elide: Text.ElideRight
                        font.weight: Font.Medium
                    }

                    // App ID — secondary/caption text
                    Controls.Label {
                        Layout.fillWidth: true
                        text: model.appId
                        elide: Text.ElideRight
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                    }

                    // Metadata row: version · size
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

                // Update checkbox — visible only when this item has an update
                Controls.CheckBox {
                    Layout.alignment: Qt.AlignVCenter
                    visible: model.hasUpdate
                    checked: model.isChecked
                    onToggled: page.installedModel.toggleUpdateChecked(index)
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

        Controls.ScrollBar.vertical: Controls.ScrollBar {
            id: verticalScrollBar
            policy: Controls.ScrollBar.AsNeeded
        }
    }

    // ── Footer: update controls ────────────────────────────────────────────────
    footer: Controls.ToolBar {
        id: footerBar
        // Always shown (when the model exists) so the user can always trigger an
        // update check; the inner rows switch between check / update / progress.
        visible: page.installedModel !== null

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            // Progress bar + status text
            ColumnLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing / 2
                visible: page.installedModel && (page.installedModel.updating || page.installedModel.checking_updates)

                Controls.ProgressBar {
                    Layout.fillWidth: true
                    value: page.installedModel ? page.installedModel.update_progress : 0
                    indeterminate: page.installedModel && (
                        page.installedModel.checking_updates ||
                        (page.installedModel.updating && page.installedModel.update_progress <= 0.01)
                    )
                }

                Controls.Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    font: Kirigami.Theme.smallFont
                    color: Kirigami.Theme.disabledTextColor
                    elide: Text.ElideRight
                    text: {
                        if (!page.installedModel) return "";
                        if (page.installedModel.updating)
                            return page.installedModel.update_status_text || i18n("Updating…");
                        if (page.installedModel.checking_updates)
                            return i18n("Checking for updates…");
                        return "";
                    }
                }
            }

            // Action row: select-all + check + update buttons
            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                visible: page.installedModel && page.installedModel.updates_available_count > 0
                         && !page.installedModel.checking_updates && !page.installedModel.updating

                Controls.CheckBox {
                    text: i18nc("%1 = count", "Select all (%1)", page.installedModel ? page.installedModel.updates_available_count : 0)
                    checked: page.installedModel
                             && page.installedModel.updates_checked_count > 0
                             && page.installedModel.updates_checked_count === page.installedModel.updates_available_count
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
                        const allChecked = page.installedModel.updates_checked_count === page.installedModel.updates_available_count;
                        page.installedModel.setAllUpdatesChecked(!allChecked);
                    }
                }

                Item { Layout.fillWidth: true }

                Controls.Button {
                    text: i18n("Check for Updates")
                    icon.name: "view-refresh"
                    enabled: page.installedModel && !page.installedModel.checking_updates && !page.installedModel.updating
                    onClicked: page.installedModel.checkForUpdates()
                }

                Controls.Button {
                    text: i18nc("%1 = count of selected updates", "Update (%1)", page.installedModel ? page.installedModel.updates_checked_count : 0)
                    icon.name: "system-software-update"
                    highlighted: true
                    enabled: page.installedModel && page.installedModel.updates_checked_count > 0
                             && !page.installedModel.checking_updates && !page.installedModel.updating
                    onClicked: page.installedModel.updateSelectedApps()
                }
            }

            // Check for updates row (shown when no updates have been found yet)
            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                visible: page.installedModel && page.installedModel.updates_available_count === 0
                         && !page.installedModel.checking_updates && !page.installedModel.updating

                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    visible: page.hasChecked
                    source: "checkmark"
                    color: Kirigami.Theme.positiveTextColor
                    isMask: true
                }

                Controls.Label {
                    text: page.hasChecked ? i18n("All applications are up to date")
                                          : i18n("Check for available application updates")
                    color: Kirigami.Theme.disabledTextColor
                    elide: Text.ElideRight
                }

                Item { Layout.fillWidth: true }

                Controls.Button {
                    text: i18n("Check for Updates")
                    icon.name: "view-refresh"
                    onClicked: page.installedModel.checkForUpdates()
                }
            }
        }
    }
}
