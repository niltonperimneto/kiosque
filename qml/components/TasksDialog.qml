// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

// Every queued, running and recently finished installation, removal and
// update, whichever page started it. Work continues while this is closed.
Kirigami.Dialog {
    id: dialog
    objectName: "tasksDialog"

    required property JobsModel jobs

    title: L10n.i18n("Tasks", [])
    preferredWidth: Kirigami.Units.gridUnit * 26
    padding: 0
    standardButtons: Kirigami.Dialog.NoButton

    function stateText(state, error, status) {
        switch (state) {
        case "queued": return L10n.i18n("Waiting…", []);
        case "running": return status !== "" ? status : L10n.i18n("In progress…", []);
        case "done": return L10n.i18n("Completed", []);
        case "cancelled": return L10n.i18n("Cancelled", []);
        default: return error !== "" ? L10n.i18n("Failed: %1", [error]) : L10n.i18n("Failed", []);
        }
    }

    function kindText(kind, title) {
        switch (kind) {
        case "uninstall": return L10n.i18n("Removing %1", [title]);
        case "update": return L10n.i18n("Updating %1", [title]);
        default: return L10n.i18n("Installing %1", [title]);
        }
    }

    customFooterActions: [
        Kirigami.Action {
            objectName: "clearFinishedTasks"
            text: L10n.i18n("Clear Finished", [])
            icon.name: "edit-clear-history"
            enabled: dialog.jobs.finished_count > 0
            onTriggered: dialog.jobs.clearFinished()
        }
    ]

    ListView {
        id: list
        objectName: "tasksList"
        implicitHeight: Math.max(Kirigami.Units.gridUnit * 6,
                                 Math.min(contentHeight, Kirigami.Units.gridUnit * 20))
        clip: true
        model: dialog.jobs

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 2
            visible: list.count === 0
            icon.name: "checkmark"
            text: L10n.i18n("No Tasks", [])
            explanation: L10n.i18n("Installations, removals and updates appear here.", [])
        }

        delegate: Controls.ItemDelegate {
            id: row

            required property var jobId
            required property string title
            required property string kind
            required property string jobState
            required property real progress
            required property string status
            required property string error
            required property string appId
            required property bool active

            width: ListView.view.width
            hoverEnabled: appId !== ""
            down: false
            Accessible.name: dialog.kindText(kind, title)
            Accessible.description: dialog.stateText(jobState, error, status)
            onClicked: {
                if (row.appId !== "") {
                    dialog.close();
                    applicationWindow().pushAppDetail(row.appId);
                }
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                    source: row.appId !== "" ? row.appId : "system-software-update"
                    fallback: "application-x-executable"
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Controls.Label {
                        Layout.fillWidth: true
                        text: dialog.kindText(row.kind, row.title)
                        elide: Text.ElideRight
                    }

                    Controls.ProgressBar {
                        Layout.fillWidth: true
                        visible: row.active
                        from: 0
                        to: 1
                        value: Math.max(0, row.progress)
                        indeterminate: row.jobState === "queued" || row.progress < 0
                    }

                    Controls.Label {
                        Layout.fillWidth: true
                        text: dialog.stateText(row.jobState, row.error, row.status)
                        elide: Text.ElideRight
                        font: Kirigami.Theme.smallFont
                        color: row.jobState === "failed" ? Kirigami.Theme.negativeTextColor
                             : row.jobState === "done" ? Kirigami.Theme.positiveTextColor
                             : Kirigami.Theme.disabledTextColor
                    }
                }

                Controls.ToolButton {
                    objectName: "cancelTask_" + row.jobId
                    visible: row.active
                    icon.name: "dialog-cancel"
                    text: L10n.i18n("Cancel", [])
                    display: Controls.AbstractButton.IconOnly
                    Accessible.name: L10n.i18n("Cancel %1", [dialog.kindText(row.kind, row.title)])
                    Controls.ToolTip.text: text
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: dialog.jobs.cancel(row.jobId)
                }
            }
        }
    }
}
