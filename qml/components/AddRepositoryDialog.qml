// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import org.kde.ki18n
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

// The only way to add a Flatpak repository. Both a manually entered URL and an
// opened .flatpakrepo file are inspected first, so the user always confirms
// the descriptor's origin and signing key before anything is added.
//
// phase: "enter" (manual URL) → "loading" → "confirm" → "done" | "failed".
Kirigami.Dialog {
    id: dialog

    // KDE Addons resolve these functions in their creation context.
    KI18nContext { id: kdeTranslations; translationDomain: "kiosque" }
    function i18nd() { return kdeTranslations.i18nd.apply(kdeTranslations, arguments); }
    function i18ndc() { return kdeTranslations.i18ndc.apply(kdeTranslations, arguments); }
    function i18ndp() { return kdeTranslations.i18ndp.apply(kdeTranslations, arguments); }
    function i18ndcp() { return kdeTranslations.i18ndcp.apply(kdeTranslations, arguments); }
    objectName: "repoFileDialog"

    property string phase: "enter"
    property string source: ""
    property string requestedName: ""
    property string repoTitle: ""
    property string repoUrl: ""
    property string errorText: ""

    // Ask for a .flatpakrepo URL, then inspect it.
    function openManual() {
        clearInspection();
        sourceField.text = "";
        manualNameField.text = "";
        phase = "enter";
        open();
        sourceField.forceActiveFocus();
    }

    // Inspect `path` (local path or URL); `name` overrides the suggested name.
    function inspect(path, name) {
        clearInspection();
        source = path;
        requestedName = name || "";
        phase = "loading";
        open();
        RepoModel.inspectRepoFile(path);
    }

    function clearInspection() {
        repoTitle = "";
        repoUrl = "";
        errorText = "";
        repoFileNameField.text = "";
    }

    title: phase === "failed" ? L10n.i18n("Cannot Add Repository", [])
         : phase === "enter" ? L10n.i18n("Add Repository", [])
         : L10n.i18n("Add Repository?", [])
    preferredWidth: Kirigami.Units.gridUnit * 24
    padding: Kirigami.Units.largeSpacing

    onRejected: RepoModel.discardRepoFile()

    Connections {
        target: RepoModel
        function onRepoFileInspected(success, suggestedName, title, url, error) {
            if (dialog.phase !== "loading") {
                return;
            }
            if (success) {
                dialog.repoTitle = title;
                dialog.repoUrl = url;
                repoFileNameField.text = dialog.requestedName !== "" ? dialog.requestedName : suggestedName;
                dialog.phase = "confirm";
            } else {
                dialog.errorText = L10n.i18n("Could not open the file: %1", [error]);
                dialog.phase = "failed";
            }
        }
    }

    // The buttons change with the phase, so they live in the content: the
    // footer DialogButtonBox mis-positions buttons swapped after opening.
    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Controls.Label {
            Layout.fillWidth: true
            visible: dialog.phase === "enter"
            text: L10n.i18n("Enter the address of a .flatpakrepo file. Its details are shown for confirmation before the repository is added for your user.", [])
            wrapMode: Text.WordWrap
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true
            visible: dialog.phase === "enter"

            Controls.TextField {
                id: sourceField
                objectName: "repoSourceField"
                Kirigami.FormData.label: L10n.i18n("Address:", [])
                placeholderText: "https://dl.flathub.org/beta-repo/flathub-beta.flatpakrepo"
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
                Layout.fillWidth: true
                onAccepted: continueButton.clicked()
            }
            Controls.TextField {
                id: manualNameField
                objectName: "repoManualNameField"
                Kirigami.FormData.label: L10n.i18n("Name (optional):", [])
                placeholderText: "flathub-beta"
                Layout.fillWidth: true
            }
        }

        Controls.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            visible: dialog.phase === "loading"
            running: visible
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true
            visible: dialog.phase === "confirm"

            Controls.Label {
                Kirigami.FormData.label: L10n.i18n("Title:", [])
                visible: dialog.repoTitle !== ""
                text: dialog.repoTitle
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Controls.Label {
                Kirigami.FormData.label: L10n.i18n("URL:", [])
                text: dialog.repoUrl
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }
            Controls.TextField {
                id: repoFileNameField
                objectName: "repoFileNameField"
                Kirigami.FormData.label: L10n.i18n("Name:", [])
                placeholderText: "flathub-beta"
                Layout.fillWidth: true
            }
        }

        Controls.Label {
            Layout.fillWidth: true
            visible: dialog.phase === "confirm"
            text: L10n.i18n("Repository signing key SHA-256: %1", [RepoModel.repo_key_sha256])
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dialog.phase === "confirm"
            type: Kirigami.MessageType.Warning
            text: L10n.i18n("Applications from this repository will be offered in Kiosque. Only add repositories you trust.", [])
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dialog.phase === "failed"
            type: Kirigami.MessageType.Error
            text: dialog.errorText
        }

        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: Kirigami.Units.smallSpacing

            Controls.Button {
                id: continueButton
                objectName: "repoContinueButton"
                visible: dialog.phase === "enter"
                enabled: sourceField.text.trim().length > 0
                text: L10n.i18n("Continue", [])
                icon.name: "go-next"
                onClicked: {
                    if (enabled) {
                        dialog.inspect(sourceField.text.trim(), manualNameField.text.trim());
                    }
                }
            }
            Controls.Button {
                objectName: "repoAddButton"
                visible: dialog.phase === "confirm"
                enabled: /^[A-Za-z0-9_][A-Za-z0-9._-]*$/.test(repoFileNameField.text)
                text: L10n.i18n("Add", [])
                icon.name: "list-add"
                onClicked: {
                    RepoModel.addInspectedRemote(repoFileNameField.text);
                    dialog.phase = "done";
                    dialog.close();
                }
            }
            Controls.Button {
                objectName: "repoCancelButton"
                text: dialog.phase === "failed" ? L10n.i18n("Close", []) : L10n.i18n("Cancel", [])
                icon.name: dialog.phase === "failed" ? "dialog-close" : "dialog-cancel"
                onClicked: {
                    RepoModel.discardRepoFile();
                    dialog.close();
                }
            }
        }
    }
}
