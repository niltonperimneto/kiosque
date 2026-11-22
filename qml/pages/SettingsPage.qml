// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque
import org.kde.kirigamiaddons.components as KirigamiAddons
import org.kde.kirigamiaddons.formcard as FormCard

// Settings in the standard KDE FormCard layout: one scrolling page with a
// card per topic, like other Plasma applications.
FormCard.FormCardPage {
    id: page
    objectName: "settingsPage"
    title: i18n("Settings")

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
            ? i18n("Enter code %1 at %2 to sign in.", SettingsController.login_user_code, SettingsController.login_verification_url)
            : i18n("Complete sign-in in your browser.")
        actions: Kirigami.Action {
            text: i18n("Cancel sign-in")
            onTriggered: SettingsController.cancelLogin()
        }
    }

    // ── Updates ─────────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: i18n("Automatic Updates")
    }

    FormCard.FormCard {
        objectName: "updatesCard"

        FormCard.FormSwitchDelegate {
            id: autoUpdateSwitch
            objectName: "autoUpdateSwitch"
            text: i18n("Update applications automatically")
            description: i18n("Kiosque checks for and installs updates in the background, even when it is closed.")
            checked: SettingsController.auto_update
            enabled: !SettingsController.saving_settings
            onToggled: page.saveSettings()
        }

        FormCard.FormDelegateSeparator { above: autoUpdateSwitch; below: frequencyCombo }

        FormCard.FormComboBoxDelegate {
            id: frequencyCombo
            objectName: "updateFrequencyCombo"
            text: i18n("Frequency")
            model: [i18n("Daily"), i18n("Weekly")]
            currentIndex: SettingsController.update_frequency.toLowerCase() === "weekly" ? 1 : 0
            enabled: autoUpdateSwitch.checked && !SettingsController.saving_settings
            onActivated: page.saveSettings()
        }

        FormCard.FormDelegateSeparator { above: frequencyCombo; below: timeField }

        FormCard.FormTextFieldDelegate {
            id: timeField
            objectName: "updateTimeField"
            label: i18n("Time of day (HH:MM)")
            text: SettingsController.update_time
            placeholderText: "02:00"
            inputMethodHints: Qt.ImhTime
            enabled: autoUpdateSwitch.checked && !SettingsController.saving_settings
            validator: RegularExpressionValidator { regularExpression: /^([01]?[0-9]|2[0-3]):[0-5][0-9]$/ }
            statusMessage: acceptableInput || text === "" ? "" : i18n("Use 24-hour time, for example 02:00.")
            status: Kirigami.MessageType.Error
            onEditingFinished: {
                if (acceptableInput) {
                    page.saveSettings()
                }
            }
        }
    }

    // ── Repositories ────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: i18n("Repositories")
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
                    text: i18n("Remove %1", repoDelegate.text)
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
            text: RepoModel.loading ? i18n("Loading repositories…") : i18n("No repositories configured")
            description: RepoModel.loading ? "" : i18n("Add a repository such as Flathub to browse and install applications.")
        }

        FormCard.FormDelegateSeparator { below: addRepositoryButton }

        FormCard.FormButtonDelegate {
            id: addRepositoryButton
            objectName: "addRepositoryButton"
            icon.name: "list-add"
            text: i18n("Add Repository…")
            onClicked: applicationWindow().openAddRepository()
        }
    }

    // ── Review account ──────────────────────────────────────────────────
    FormCard.FormHeader {
        title: i18n("Review Account")
    }

    FormCard.FormCard {
        objectName: "accountCard"

        FormCard.FormTextDelegate {
            visible: SettingsController.is_authenticated
            text: SettingsController.oauth_username
            description: i18n("Signed in with %1", page.providerName(SettingsController.oauth_provider))
            leading: KirigamiAddons.Avatar {
                implicitWidth: Kirigami.Units.iconSizes.medium
                implicitHeight: implicitWidth
                source: SettingsController.oauth_avatar_url
                name: SettingsController.oauth_username
            }
        }

        FormCard.FormButtonDelegate {
            visible: SettingsController.is_authenticated
            icon.name: "system-log-out"
            text: i18n("Sign Out")
            onClicked: SettingsController.logout()
        }

        FormCard.FormTextDelegate {
            visible: !SettingsController.is_authenticated
            text: i18n("Not signed in")
            description: i18n("Sign in to rate applications and write reviews.")
        }

        // Breeze has no brand icons for these providers except KDE.
        Repeater {
            model: SettingsController.is_authenticated ? [] : [
                { provider: "github", icon: "user-identity" },
                { provider: "gitlab", icon: "user-identity" },
                { provider: "kde_gitlab", icon: "kde" },
                { provider: "gnome_gitlab", icon: "user-identity" }
            ]

            delegate: FormCard.FormButtonDelegate {
                required property var modelData
                objectName: "signIn_" + modelData.provider
                icon.name: modelData.icon
                text: i18n("Sign in with %1", page.providerName(modelData.provider))
                enabled: !SettingsController.login_in_progress
                onClicked: SettingsController.login(modelData.provider)
            }
        }
    }

    // ── Privacy ─────────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: i18n("Privacy")
    }

    FormCard.FormCard {
        objectName: "privacyCard"

        FormCard.FormTextDelegate {
            id: hashDelegate
            text: i18n("Review identity")
            description: SettingsController.odrs_user_hash
            descriptionItem.font.family: "monospace"
            descriptionItem.wrapMode: Text.WrapAnywhere
            trailing: Controls.ToolButton {
                icon.name: "edit-copy"
                text: i18n("Copy review identity")
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
                    applicationWindow().showPassiveNotification(i18n("Copied to clipboard"))
                }
            }
        }

        FormCard.FormTextDelegate {
            description: i18n("This pseudonymous identifier is derived from a local secret. The ODRS review server uses it to let you manage your reviews without your IP address or credentials.")
        }

        FormCard.FormDelegateSeparator {}

        FormCard.FormButtonDelegate {
            icon.name: "view-refresh"
            text: i18n("Regenerate Review Identity…")
            description: i18n("Unlinks your existing reviews from this computer")
            onClicked: regenerateSaltDialog.open()
        }

        FormCard.FormDelegateSeparator {}

        FormCard.FormButtonDelegate {
            icon.name: "edit-clear"
            text: i18n("Clear Cache")
            description: i18n("Removes downloaded catalog data and images")
            onClicked: {
                SettingsController.clearCache()
                applicationWindow().showPassiveNotification(i18n("Cache cleared"))
            }
        }
    }

    // ── About ───────────────────────────────────────────────────────────
    FormCard.FormHeader {
        title: i18n("About")
    }

    FormCard.FormCard {
        objectName: "aboutCard"
        Layout.bottomMargin: Kirigami.Units.gridUnit

        FormCard.FormButtonDelegate {
            objectName: "aboutKiosqueButton"
            icon.name: "help-about"
            text: i18n("About Kiosque")
            description: Qt.application.version !== "" ? i18n("Version %1", Qt.application.version) : ""
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
        title: i18n("Regenerate Review Identity?")
        subtitle: i18n("You will get a new pseudonymous identity on the ODRS review server. Your existing reviews and votes remain, but they are permanently unlinked from this computer and you can no longer edit or delete them.")
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Regenerate")
                icon.name: "view-refresh"
                onTriggered: {
                    SettingsController.regenerateSalt()
                    regenerateSaltDialog.close()
                }
            },
            Kirigami.Action {
                text: i18n("Cancel")
                icon.name: "dialog-cancel"
                onTriggered: regenerateSaltDialog.close()
            }
        ]
    }

    Kirigami.PromptDialog {
        id: loginErrorDialog
        objectName: "loginErrorDialog"
        property string errorMessage: ""
        title: i18n("Sign-in Failed")
        subtitle: errorMessage !== ""
            ? i18n("Could not complete sign-in: %1", errorMessage)
            : i18n("Could not complete sign-in. Check your network connection and try again.")
        standardButtons: Kirigami.Dialog.Close
    }

    Connections {
        target: SettingsController
        function onSettingsSaved(success, error) {
            page.settingsError = success ? "" : i18n("Could not apply update settings: %1", error)
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
