// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

// Confirms removing an application and offers the runtimes it leaves unused,
// pre-selected. The caller removes the app on `confirmed()` and reports the
// outcome with `appRemoved(success)`; the selected runtimes are removed only
// after the app is gone, since Flatpak refuses to remove a runtime in use.
Kirigami.PromptDialog {
    id: root

    objectName: "removeConfirmDialog"

    property string appId: ""
    property string appName: ""
    // Runtime refs confirmed for removal, waiting for the app removal.
    property var pendingRuntimes: []
    readonly property RuntimePlanner planner: RuntimePlanner {}
    readonly property var leftovers: planner.leftovers || []

    signal confirmed()

    function openFor(id, name) {
        appId = id;
        appName = name;
        planner.planRemoval(id);
        open();
    }

    function selectedRuntimes() {
        const refs = [];
        for (let i = 0; i < leftoverRepeater.count; ++i) {
            const box = leftoverRepeater.itemAt(i);
            if (box && box.checked)
                refs.push(box.reference);
        }
        return refs;
    }

    function appRemoved(success) {
        const refs = pendingRuntimes;
        pendingRuntimes = [];
        if (success && refs.length > 0)
            planner.removeRuntimes(refs);
    }

    title: L10n.i18n("Remove %1?", [appName])
    standardButtons: Kirigami.Dialog.NoButton

    ColumnLayout {
        spacing: Kirigami.Units.mediumSpacing

        Controls.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: L10n.i18n("The application is uninstalled. Its settings and data in your home folder are kept.", [])
        }

        Controls.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            visible: root.planner.loading
            running: visible
        }

        Controls.Label {
            objectName: "unusedRuntimesLabel"
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            visible: root.leftovers.length > 0
            wrapMode: Text.WordWrap
            text: L10n.i18np("No other application uses this runtime. Remove it too to free space:",
                             "No other application uses these runtimes. Remove them too to free space:",
                             root.leftovers.length, [])
        }

        Repeater {
            id: leftoverRepeater
            model: root.leftovers

            delegate: Controls.CheckBox {
                required property var modelData
                readonly property string reference: modelData.reference
                objectName: "leftoverRuntime_" + modelData.reference
                Layout.fillWidth: true
                checked: true
                text: modelData.size
                    ? L10n.i18nc("@option:check runtime and its size", "%1 (%2)", [modelData.name, modelData.size])
                    : modelData.name
            }
        }
    }

    customFooterActions: [
        Kirigami.Action {
            objectName: "confirmRemoveAction"
            text: L10n.i18n("Remove", [])
            icon.name: "edit-delete-remove"
            onTriggered: {
                root.pendingRuntimes = root.selectedRuntimes();
                root.close();
                root.confirmed();
            }
        },
        Kirigami.Action {
            text: L10n.i18n("Cancel", [])
            icon.name: "dialog-cancel"
            onTriggered: root.close()
        }
    ]
}
