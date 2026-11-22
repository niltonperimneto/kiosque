// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque

// Pending updates. Uses the window's shared InstalledModel so an update in
// progress stays visible when navigating away and back; the model is
// filtered to updatable entries while this page exists.
Kirigami.ScrollablePage {
    id: page
    objectName: "updatesPage"
    title: i18n("Updates")
    supportsRefreshing: true

    property var installedModel: applicationWindow().installedModel

    readonly property int availableCount: installedModel ? installedModel.updates_available_count : 0
    readonly property int selectedCount: installedModel ? installedModel.updates_checked_count : 0
    readonly property bool updating: installedModel !== null && installedModel.updating
    readonly property string checkError: installedModel ? installedModel.updates_error : ""
    readonly property bool checking: installedModel !== null && installedModel.checking_updates

    // Whether a check finished during this session, so "up to date" is only
    // claimed after an actual check.
    property bool hasChecked: false

    function reload() {
        installedModel.checkForUpdates();
    }

    onRefreshingChanged: {
        if (refreshing) {
            page.reload();
            refreshing = false;
        }
    }

    Component.onCompleted: {
        installedModel.applySearchFilter("");
        installedModel.applyShowUpdatesOnly(true);
    }
    Component.onDestruction: {
        if (installedModel) {
            installedModel.applyShowUpdatesOnly(false);
        }
    }

    Connections {
        target: page.installedModel
        ignoreUnknownSignals: true
        function onChecking_updatesChanged() {
            if (!page.installedModel.checking_updates) {
                page.hasChecked = page.installedModel.updates_error === "";
            }
        }
    }

    actions: [
        Kirigami.Action {
            objectName: "updateAllAction"
            text: i18n("Update All")
            icon.name: "system-software-update"
            visible: page.availableCount > 0
            enabled: !page.updating && !page.checking
            onTriggered: {
                page.installedModel.setAllUpdatesChecked(true);
                page.installedModel.updateSelectedApps();
            }
        },
        Kirigami.Action {
            objectName: "checkUpdatesAction"
            text: i18n("Check for Updates")
            icon.name: "view-refresh"
            enabled: !page.updating && !page.checking
            onTriggered: page.reload()
        }
    ]

    header: ColumnLayout {
        spacing: 0
        visible: page.updating || page.checking || page.availableCount > 0 || page.checkError !== ""

        Kirigami.InlineMessage {
            objectName: "updateCheckError"
            Layout.fillWidth: true
            type: Kirigami.MessageType.Error
            text: page.checkError
            visible: text !== ""
        }
        // Progress of the running update or check.
        ColumnLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing
            visible: page.updating || page.checking

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.largeSpacing

                Controls.ProgressBar {
                    objectName: "updateProgressBar"
                    Layout.fillWidth: true
                    from: 0
                    to: 1
                    value: page.installedModel ? Math.max(0, page.installedModel.update_progress) : 0
                    indeterminate: page.checking || page.installedModel.update_progress <= 0.01
                }

                Controls.Button {
                    objectName: "cancelUpdateButton"
                    visible: page.updating
                    text: i18n("Cancel")
                    icon.name: "dialog-cancel"
                    onClicked: page.installedModel.cancelUpdate()
                }
            }

            Controls.Label {
                objectName: "updateStatusLabel"
                Layout.fillWidth: true
                elide: Text.ElideRight
                color: Kirigami.Theme.disabledTextColor
                text: {
                    if (!page.updating)
                        return i18n("Checking for updates…");
                    if (page.installedModel.update_status_text !== "")
                        return page.installedModel.update_status_text;
                    return page.installedModel.update_progress < 0
                        ? i18n("Initializing update…") : i18n("Updating…");
                }
            }
        }

        // Selection summary for the list below.
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.largeSpacing
            visible: page.availableCount > 0 && !page.updating

            Controls.CheckBox {
                objectName: "selectAllUpdates"
                text: i18np("%1 update available", "%1 updates available", page.availableCount)
                tristate: true
                checkState: page.selectedCount === 0 ? Qt.Unchecked
                          : page.selectedCount === page.availableCount ? Qt.Checked
                          : Qt.PartiallyChecked
                onClicked: page.installedModel.setAllUpdatesChecked(page.selectedCount !== page.availableCount)
            }

            Item { Layout.fillWidth: true }

            Controls.Button {
                objectName: "updateSelectedButton"
                visible: page.selectedCount > 0 && page.selectedCount < page.availableCount
                text: i18np("Update %1 Selected", "Update %1 Selected", page.selectedCount)
                icon.name: "system-software-update"
                enabled: !page.checking
                onClicked: page.installedModel.updateSelectedApps()
            }
        }

        Kirigami.Separator { Layout.fillWidth: true }
    }

    ListView {
        id: listView
        objectName: "updatesList"
        model: page.installedModel
        clip: true
        currentIndex: -1

        Kirigami.PlaceholderMessage {
            objectName: "upToDateMessage"
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: listView.count === 0 && !page.checking && !page.updating
            icon.name: page.checkError ? "dialog-error-symbolic" : page.hasChecked ? "checkmark" : "system-software-update"
            text: page.checkError ? i18n("Could Not Check for Updates") : page.hasChecked ? i18n("Up to Date") : i18n("No Updates Found Yet")
            explanation: page.checkError ? page.checkError : page.hasChecked
                ? i18n("All installed applications and runtimes are up to date.")
                : i18n("Check now to see whether newer versions are available.")
            helpfulAction: page.hasChecked && !page.checkError ? null : checkNowAction

            Kirigami.Action {
                id: checkNowAction
                text: i18n("Check for Updates")
                icon.name: "view-refresh"
                onTriggered: page.reload()
            }
        }

        delegate: Controls.ItemDelegate {
            id: row

            required property int index
            required property string name
            required property string appId
            required property string version
            required property string size
            required property bool isChecked

            width: ListView.view.width
            text: name
            Accessible.description: appId
            onClicked: applicationWindow().pushAppDetail(row.appId)

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                Controls.CheckBox {
                    Layout.alignment: Qt.AlignVCenter
                    checked: row.isChecked
                    enabled: !page.updating
                    Accessible.name: i18n("Include %1 in the update", row.name)
                    onToggled: page.installedModel.toggleUpdateChecked(row.index)
                }

                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                    Layout.alignment: Qt.AlignVCenter
                    source: row.appId
                    fallback: "application-x-executable"
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    Controls.Label {
                        Layout.fillWidth: true
                        text: row.name
                        elide: Text.ElideRight
                    }
                    Controls.Label {
                        Layout.fillWidth: true
                        text: [row.appId, row.version, row.size].filter(s => s !== "").join("  ·  ")
                        elide: Text.ElideRight
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                    }
                }
            }
        }
    }
}
