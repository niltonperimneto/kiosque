// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import org.kde.ki18n
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque
import org.kde.kirigamiaddons.components as KirigamiAddons
import org.kde.kirigamiaddons.formcard as FormCard

// Settings in the standard KDE FormCard layout: one scrolling page with a
// card per topic, like other Plasma applications.
FormCard.FormCardPage {
    id: page

    // KDE Addons resolve these functions in their creation context.
    KI18nContext { id: kdeTranslations; translationDomain: "kiosque" }
    function i18nd() { return kdeTranslations.i18nd.apply(kdeTranslations, arguments); }
    function i18ndc() { return kdeTranslations.i18ndc.apply(kdeTranslations, arguments); }
    function i18ndp() { return kdeTranslations.i18ndp.apply(kdeTranslations, arguments); }
    function i18ndcp() { return kdeTranslations.i18ndcp.apply(kdeTranslations, arguments); }
    RemoteImage { id: userAvatar; url: SettingsController.oauth_avatar_url; kind: "avatar" }
    objectName: "settingsPage"
    title: L10n.i18n("Settings", [])

    // Set once the controls reflect the stored settings, so the initial
    // binding updates never write settings back.
    property bool readyForSaving: false
    property string settingsError: ""


    Component.onCompleted: {
        SettingsController.loadSettings()
        RepoModel.refresh()
        readyForSaving = true
    }

    function saveSettings() {
        if (readyForSaving && !SettingsController.saving_settings && timeField.acceptableInput) {
            settingsError = ""
            SettingsController.saveSettings(
                autoUpdateSwitch.checked,
                frequencyCombo.currentIndex === 1 ? "Weekly" : "Daily",
                timeField.text
            )
        }
    }

    // Which notice the Automatic Updates section shows: "system" when a host
    // timer already updates Flatpaks, "missing", "outdated" or "unreachable"
    // when the Flatpak build cannot reach a usable companion, else "".
    function updatesNotice(sandboxed, companion, updater, autoUpdate) {
        if (updater !== "" && !autoUpdate)
            return "system";
        if (!sandboxed)
            return "";
        if (companion === "missing" || companion === "outdated" || companion === "unreachable")
            return companion;
        return "";
    }
    readonly property string updatesNoticeKind: updatesNotice(SettingsController.sandboxed,
        SettingsController.companion_state, SettingsController.system_updater, SettingsController.auto_update)
    // Inside Flatpak the companion performs the updates: only offer to turn
    // them on when it is ready (turning them off always works).
    readonly property bool canEnableAutoUpdates: !SettingsController.sandboxed
        || SettingsController.companion_state === "ready"

    function providerName(provider) {
        switch (provider) {
        case "github": return "GitHub"
        case "gitlab": return "GitLab"
        case "gnome_gitlab": return "GNOME GitLab"
        case "kde_gitlab": return "KDE GitLab"
        default: return provider
        }
    }

    Kirigami.InlineMessage {
        objectName: "settingsErrorMessage"
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Error
        text: page.settingsError
        visible: text !== ""
    }

    Kirigami.InlineMessage {
        objectName: "loginProgressMessage"
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Information
        visible: SettingsController.login_in_progress
        text: SettingsController.login_user_code !== ""
            ? L10n.i18n("Enter code %1 at %2 to sign in.", [SettingsController.login_user_code, SettingsController.login_verification_url])
            : L10n.i18n("Complete sign-in in your browser.", [])
        actions: Kirigami.Action {
            text: L10n.i18n("Cancel sign-in", [])
            onTriggered: SettingsController.cancelLogin()
        }
    }

    // ── Updates ─────────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: L10n.i18n("Automatic Updates", [])
    }

    Kirigami.InlineMessage {
        objectName: "updatesNoticeMessage"
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        Layout.alignment: Qt.AlignHCenter
        Layout.bottomMargin: Kirigami.Units.largeSpacing
        visible: page.updatesNoticeKind !== ""
        type: page.updatesNoticeKind === "system" ? Kirigami.MessageType.Information
                                                  : Kirigami.MessageType.Warning
        text: {
            switch (page.updatesNoticeKind) {
            case "system":
                return L10n.i18n("Your system already updates Flatpak applications automatically (%1), so Kiosque does not need to.", [SettingsController.system_updater])
            case "missing":
                return L10n.i18n("In the Flatpak version, automatic updates are run by the Kiosque companion, a small package on your computer. Install kiosque-companion from your distribution to turn them on.", [])
            case "outdated":
                return L10n.i18n("The Kiosque companion on your computer is too old for automatic updates. Update the kiosque-companion package.", [])
            case "unreachable":
                return L10n.i18n("The Kiosque companion on your computer could not be reached.", [])
            default:
                return ""
            }
        }
        actions: Kirigami.Action {
            text: L10n.i18n("Check Again", [])
            icon.name: "view-refresh"
            visible: page.updatesNoticeKind !== "system"
            onTriggered: SettingsController.refreshUpdateStatus()
        }
    }

    FormCard.FormCard {
        objectName: "updatesCard"

        FormCard.FormSwitchDelegate {
            id: autoUpdateSwitch
            objectName: "autoUpdateSwitch"
            text: L10n.i18n("Update applications automatically", [])
            description: L10n.i18n("Kiosque checks for and installs updates in the background, even when it is closed.", [])
            checked: SettingsController.auto_update
            enabled: !SettingsController.saving_settings && (checked || page.canEnableAutoUpdates)
            onToggled: page.saveSettings()
        }

        FormCard.FormDelegateSeparator { above: autoUpdateSwitch; below: frequencyCombo }

        FormCard.FormComboBoxDelegate {
            id: frequencyCombo
            objectName: "updateFrequencyCombo"
            text: L10n.i18n("Frequency", [])
            model: [L10n.i18n("Daily", []), L10n.i18n("Weekly", [])]
            currentIndex: SettingsController.update_frequency.toLowerCase() === "weekly" ? 1 : 0
            enabled: autoUpdateSwitch.checked && !SettingsController.saving_settings
            onActivated: page.saveSettings()
        }

        FormCard.FormDelegateSeparator { above: frequencyCombo; below: timeField }

        FormCard.FormTextFieldDelegate {
            id: timeField
            objectName: "updateTimeField"
            label: L10n.i18n("Time of day (HH:MM)", [])
            text: SettingsController.update_time
            placeholderText: "02:00"
            inputMethodHints: Qt.ImhTime
            enabled: autoUpdateSwitch.checked && !SettingsController.saving_settings
            validator: RegularExpressionValidator { regularExpression: /^([01]?[0-9]|2[0-3]):[0-5][0-9]$/ }
            statusMessage: acceptableInput || text === "" ? "" : L10n.i18n("Use 24-hour time, for example 02:00.", [])
            status: Kirigami.MessageType.Error
            onEditingFinished: {
                if (acceptableInput) {
                    page.saveSettings()
                }
            }
        }

        FormCard.FormDelegateSeparator { visible: SettingsController.sandboxed; above: timeField; below: selfUpdateNote }

        FormCard.FormTextDelegate {
            id: selfUpdateNote
            objectName: "selfUpdateNote"
            visible: SettingsController.sandboxed
            icon.name: "org.kiosque.Kiosque"
            text: L10n.i18n("Kiosque keeps itself up to date", [])
            description: L10n.i18n("New versions of Kiosque are installed through Flatpak in the background, with or without the companion.", [])
        }
    }

    // ── Repositories ────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: L10n.i18n("Repositories", [])
    }

    FormCard.FormCard {
        objectName: "repositoriesCard"

        Repeater {
            id: repoRepeater
            model: RepoModel

            delegate: FormCard.FormTextDelegate {
                id: repoDelegate

                required property int index
                required property string name
                required property string title
                required property string url

                text: title !== "" ? title : name
                description: url
                descriptionItem.elide: Text.ElideMiddle
                leading: Kirigami.Icon {
                    source: "folder-download-symbolic"
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: implicitWidth
                }
                trailing: Controls.ToolButton {
                    objectName: "removeRepository_" + repoDelegate.name
                    icon.name: "edit-delete-remove"
                    text: L10n.i18n("Remove %1", [repoDelegate.text])
                    display: Controls.AbstractButton.IconOnly
                    Accessible.name: text
                    Controls.ToolTip.text: text
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: RepoModel.removeRemote(repoDelegate.name)
                }
            }
        }

        FormCard.FormTextDelegate {
            visible: repoRepeater.count === 0
            text: RepoModel.loading ? L10n.i18n("Loading repositories…", []) : L10n.i18n("No repositories configured", [])
            description: RepoModel.loading ? "" : L10n.i18n("Add a repository such as Flathub to browse and install applications.", [])
        }

        FormCard.FormDelegateSeparator { below: addRepositoryButton }

        FormCard.FormButtonDelegate {
            id: addRepositoryButton
            objectName: "addRepositoryButton"
            icon.name: "list-add"
            text: L10n.i18n("Add Repository…", [])
            onClicked: applicationWindow().openAddRepository()
        }
    }

    // ── Review account ──────────────────────────────────────────────────
    FormCard.FormHeader {
        title: L10n.i18n("Review Account", [])
    }

    FormCard.FormCard {
        objectName: "accountCard"

        FormCard.FormTextDelegate {
            visible: SettingsController.is_authenticated
            text: SettingsController.oauth_username
            description: L10n.i18n("Signed in with %1", [page.providerName(SettingsController.oauth_provider)])
            leading: KirigamiAddons.Avatar {
                implicitWidth: Kirigami.Units.iconSizes.medium
                implicitHeight: implicitWidth
                source: userAvatar.source
                name: SettingsController.oauth_username
            }
        }

        FormCard.FormButtonDelegate {
            visible: SettingsController.is_authenticated
            icon.name: "system-log-out"
            text: L10n.i18n("Sign Out", [])
            onClicked: SettingsController.logout()
        }

        FormCard.FormTextDelegate {
            visible: !SettingsController.is_authenticated
            text: L10n.i18n("Not signed in", [])
            description: L10n.i18n("Sign in to rate applications and write reviews.", [])
        }

        Repeater {
            model: SettingsController.is_authenticated ? [] : ["github", "gitlab", "kde_gitlab", "gnome_gitlab"]

            delegate: FormCard.FormButtonDelegate {
                required property string modelData
                objectName: "signIn_" + modelData
                text: L10n.i18n("Sign in with %1", [page.providerName(modelData)])
                enabled: !SettingsController.login_in_progress
                onClicked: SettingsController.login(modelData)
                leading: Kirigami.Icon {
                    source: Qt.resolvedUrl("../images/" + modelData + "-symbolic.svg")
                    isMask: true
                    color: Kirigami.Theme.textColor
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: implicitWidth
                }
            }
        }
    }

    // ── Privacy ─────────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: L10n.i18n("Privacy", [])
    }

    FormCard.FormCard {
        objectName: "privacyCard"

        FormCard.FormTextDelegate {
            id: hashDelegate
            text: L10n.i18n("Review identity", [])
            description: SettingsController.odrs_user_hash
            descriptionItem.font.family: "monospace"
            descriptionItem.wrapMode: Text.WrapAnywhere
            trailing: Controls.ToolButton {
                icon.name: "edit-copy"
                text: L10n.i18n("Copy review identity", [])
                display: Controls.AbstractButton.IconOnly
                Accessible.name: text
                Controls.ToolTip.text: text
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: {
                    clipboardHelper.text = SettingsController.odrs_user_hash
                    clipboardHelper.selectAll()
                    clipboardHelper.copy()
                    clipboardHelper.text = ""
                    applicationWindow().showPassiveNotification(L10n.i18n("Copied to clipboard", []))
                }
            }
        }

        FormCard.FormTextDelegate {
            description: L10n.i18n("This pseudonymous identifier is derived from a local secret. The ODRS review server uses it to let you manage your reviews without your IP address or credentials.", [])
        }

        FormCard.FormDelegateSeparator {}

        FormCard.FormButtonDelegate {
            icon.name: "view-refresh"
            text: L10n.i18n("Regenerate Review Identity…", [])
            description: L10n.i18n("Unlinks your existing reviews from this computer", [])
            onClicked: regenerateSaltDialog.open()
        }

        FormCard.FormDelegateSeparator {}

        FormCard.FormButtonDelegate {
            icon.name: "edit-clear"
            text: L10n.i18n("Clear Cache", [])
            description: L10n.i18n("Removes downloaded catalog data and images", [])
            onClicked: {
                SettingsController.clearCache()
                applicationWindow().showPassiveNotification(L10n.i18n("Cache cleared", []))
            }
        }
    }

    // ── About ───────────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: L10n.i18n("About", [])
    }

    FormCard.FormCard {
        objectName: "aboutCard"
        Layout.bottomMargin: Kirigami.Units.gridUnit

        FormCard.FormButtonDelegate {
            objectName: "aboutKiosqueButton"
            icon.name: "help-about"
            text: L10n.i18n("About Kiosque", [])
            description: Qt.application.version !== "" ? L10n.i18n("Version %1", [Qt.application.version]) : ""
            onClicked: applicationWindow().pageStack.push("qrc:/qml/pages/AboutPage.qml")
        }
    }

    // Off-screen helper for clipboard access from QML.
    TextEdit {
        id: clipboardHelper
        visible: false
    }

    Kirigami.PromptDialog {
        id: regenerateSaltDialog
        title: L10n.i18n("Regenerate Review Identity?", [])
        subtitle: L10n.i18n("You will get a new pseudonymous identity on the ODRS review server. Your existing reviews and votes remain, but they are permanently unlinked from this computer and you can no longer edit or delete them.", [])
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: L10n.i18n("Regenerate", [])
                icon.name: "view-refresh"
                onTriggered: {
                    SettingsController.regenerateSalt()
                    regenerateSaltDialog.close()
                }
            },
            Kirigami.Action {
                text: L10n.i18n("Cancel", [])
                icon.name: "dialog-cancel"
                onTriggered: regenerateSaltDialog.close()
            }
        ]
    }

    Kirigami.PromptDialog {
        id: loginErrorDialog
        objectName: "loginErrorDialog"
        property string errorMessage: ""
        title: L10n.i18n("Sign-in Failed", [])
        subtitle: errorMessage !== ""
            ? L10n.i18n("Could not complete sign-in: %1", [errorMessage])
            : L10n.i18n("Could not complete sign-in. Check your network connection and try again.", [])
        standardButtons: Kirigami.Dialog.Close
    }

    Connections {
        target: SettingsController
        function onSettingsSaved(success, error) {
            page.settingsError = success ? "" : L10n.i18n("Could not apply update settings: %1", [error])
            if (!success) {
                // Restore the binding after a user toggle rejected by the portal.
                autoUpdateSwitch.checked = Qt.binding(function() { return SettingsController.auto_update })
            }
        }
        // FormTextFieldDelegate writes its text back on creation, replacing the
        // binding; reapply stored values unless the user is editing the field.
        function onUpdate_timeChanged() {
            if (!timeField.fieldActiveFocus) {
                timeField.text = SettingsController.update_time
            }
        }
        function onLoginFailed(error) {
            loginErrorDialog.errorMessage = error
            loginErrorDialog.open()
        }
    }
}
