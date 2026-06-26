import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque
import org.kde.kirigamiaddons.components as KirigamiAddons
import QtCore

Kirigami.ScrollablePage {
    id: page
    title: i18n("Settings")

    leftPadding: Kirigami.Units.largeSpacing * 2
    rightPadding: Kirigami.Units.largeSpacing * 2
    topPadding: Kirigami.Units.largeSpacing * 1.5
    bottomPadding: Kirigami.Units.largeSpacing * 2

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]

    Settings {
        id: localSettings
        category: "SettingsPage"
        property bool showAdvanced: false
    }

    Component.onCompleted: {
        SettingsController.loadSettings()
        RepoModel.refresh()
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing * 2
        width: parent.width

        // ── Settings Header ────────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.alignment: Qt.AlignHCenter
            
            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                
                Kirigami.Heading {
                    text: i18n("Preferences")
                    level: 2
                }
                Controls.Label {
                    text: i18n("Configure updates, authentication, and flatpak repositories")
                    color: Kirigami.Theme.disabledTextColor
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                }
            }
            
            Controls.Switch {
                id: advancedToggle
                text: i18n("Advanced Settings")
                checked: localSettings.showAdvanced
                onCheckedChanged: localSettings.showAdvanced = checked
                Layout.alignment: Qt.AlignVCenter
            }
        }

        // ── Automatic Updates Card ─────────────────────────────────────────
        Controls.Pane {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.alignment: Qt.AlignHCenter
            padding: Kirigami.Units.largeSpacing * 1.5

            background: Kirigami.ShadowedRectangle {
                radius: 12
                color: Kirigami.Theme.backgroundColor
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                border.width: 1
                shadow.size: 10
                shadow.color: Qt.rgba(0, 0, 0, 0.05)
            }

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "system-software-update"
                        width: Kirigami.Units.iconSizes.medium
                        height: width
                        color: Kirigami.Theme.highlightColor
                        isMask: true
                    }
                    Kirigami.Heading {
                        text: i18n("Automatic Updates")
                        level: 3
                        Layout.fillWidth: true
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }

                Kirigami.FormLayout {
                    Layout.fillWidth: true

                    Controls.Switch {
                        id: autoUpdateSwitch
                        Kirigami.FormData.label: i18n("Enable Automatic Updates:")
                        checked: SettingsController.auto_update
                        onCheckedChanged: saveSettings()
                    }

                    Controls.ComboBox {
                        id: frequencyCombo
                        Kirigami.FormData.label: i18n("Frequency:")
                        model: [i18n("Daily"), i18n("Weekly")]
                        currentIndex: SettingsController.update_frequency.toLowerCase() === "weekly" ? 1 : 0
                        enabled: autoUpdateSwitch.checked
                        visible: advancedToggle.checked
                        onCurrentIndexChanged: saveSettings()
                    }

                    Controls.TextField {
                        id: timeField
                        Kirigami.FormData.label: i18n("Time (HH:MM):")
                        text: SettingsController.update_time
                        placeholderText: "02:00"
                        enabled: autoUpdateSwitch.checked
                        visible: advancedToggle.checked
                        validator: RegularExpressionValidator { regularExpression: /^([01]?[0-9]|2[0-3]):[0-5][0-9]$/ }
                        onTextChanged: {
                            if (acceptableInput) {
                                saveSettings()
                            }
                        }
                    }
                }
            }
        }

        // ── Repositories Card ──────────────────────────────────────────────
        Controls.Pane {
            visible: advancedToggle.checked
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.alignment: Qt.AlignHCenter
            padding: Kirigami.Units.largeSpacing * 1.5

            background: Kirigami.ShadowedRectangle {
                radius: 12
                color: Kirigami.Theme.backgroundColor
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                border.width: 1
                shadow.size: 10
                shadow.color: Qt.rgba(0, 0, 0, 0.05)
            }

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "folder-download"
                        width: Kirigami.Units.iconSizes.medium
                        height: width
                        color: Kirigami.Theme.highlightColor
                        isMask: true
                    }
                    Kirigami.Heading {
                        text: i18n("Repositories")
                        level: 3
                        Layout.fillWidth: true
                    }
                    Controls.Button {
                        text: page.width < Kirigami.Units.gridUnit * 28 ? "" : i18n("Add Repository")
                        icon.name: "list-add"
                        onClicked: addRepoDialog.open()

                        Controls.ToolTip {
                            text: i18n("Add Repository")
                            visible: parent.hovered && page.width < Kirigami.Units.gridUnit * 28
                        }
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }

                ListView {
                    id: repoList
                    Layout.fillWidth: true
                    Layout.preferredHeight: contentHeight
                    interactive: false
                    model: RepoModel
                    clip: true
                    
                    delegate: Column {
                        width: repoList.width
                        spacing: 0

                        Kirigami.SwipeListItem {
                            width: parent.width
                            contentItem: ColumnLayout {
                                spacing: Kirigami.Units.smallSpacing

                                Kirigami.Heading {
                                    level: 4
                                    text: model.title !== "" ? model.title : model.name
                                    Layout.fillWidth: true
                                }

                                Controls.Label {
                                    text: model.url
                                    color: Kirigami.Theme.disabledTextColor
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                }
                            }

                            actions: [
                                Kirigami.Action {
                                    text: i18n("Remove")
                                    icon.name: "edit-delete"
                                    onTriggered: {
                                        RepoModel.removeRemote(model.name)
                                    }
                                }
                            ]
                        }

                        Kirigami.Separator {
                            width: parent.width
                            visible: index < repoList.count - 1
                        }
                    }
                }
            }
        }

        // ── ODRS Accounts Card ──────────────────────────────────────────────
        Controls.Pane {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.alignment: Qt.AlignHCenter
            padding: Kirigami.Units.largeSpacing * 1.5

            background: Kirigami.ShadowedRectangle {
                radius: 12
                color: Kirigami.Theme.backgroundColor
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                border.width: 1
                shadow.size: 10
                shadow.color: Qt.rgba(0, 0, 0, 0.05)
            }

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "user"
                        width: Kirigami.Units.iconSizes.medium
                        height: width
                        color: Kirigami.Theme.highlightColor
                        isMask: true
                    }
                    Kirigami.Heading {
                        text: i18n("ODRS Review Account")
                        level: 3
                        Layout.fillWidth: true
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    // Case 1: Authenticated State
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing
                        visible: SettingsController.is_authenticated

                        Item {
                            id: settingsAvatarContainer
                            Layout.preferredWidth: Kirigami.Units.iconSizes.huge + 8
                            Layout.preferredHeight: Layout.preferredWidth
                            Layout.alignment: Qt.AlignVCenter

                            Kirigami.ShadowedRectangle {
                                anchors.fill: parent
                                radius: width / 2
                                color: "transparent"

                                border.width: 1.5
                                border.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.4)

                                shadow.size: 8
                                shadow.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25)
                                shadow.yOffset: 2
                            }

                            KirigamiAddons.Avatar {
                                id: settingsUserAvatar
                                anchors.centerIn: parent
                                width: parent.width - 8
                                height: width
                                source: SettingsController.oauth_avatar_url !== "" ? SettingsController.oauth_avatar_url : ""
                                name: SettingsController.oauth_username
                            }
                        }

                        ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignVCenter

                            Kirigami.Heading {
                                level: 3
                                text: SettingsController.oauth_username
                                Layout.fillWidth: true
                            }

                            Controls.Label {
                                text: i18n("Logged in via %1").arg(
                                    SettingsController.oauth_provider === "github" ? "GitHub" :
                                    SettingsController.oauth_provider === "gitlab" ? "GitLab" :
                                    SettingsController.oauth_provider === "gnome_gitlab" ? "GNOME GitLab" :
                                    SettingsController.oauth_provider === "kde_gitlab" ? "KDE GitLab" : SettingsController.oauth_provider
                                )
                                color: Kirigami.Theme.disabledTextColor
                                Layout.fillWidth: true
                            }
                        }

                        Controls.Button {
                            text: i18n("Log Out")
                            icon.name: "log-out"
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: SettingsController.logout()
                        }
                    }

                    // Case 2: Unauthenticated State
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing
                        visible: !SettingsController.is_authenticated

                        Controls.Label {
                            text: i18n("To write reviews and submit application ratings, you must authenticate using an open-source development provider. Reviews are submitted pseudonymously using a secure privacy hash.")
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            color: Kirigami.Theme.textColor
                        }

                        Flow {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.mediumSpacing

                            Controls.Button {
                                text: "GitHub"
                                icon.name: "git-brand"
                                onClicked: SettingsController.login("github")
                            }

                            Controls.Button {
                                text: "GitLab"
                                icon.name: "git-brand"
                                onClicked: SettingsController.login("gitlab")
                            }

                            Controls.Button {
                                text: "KDE GitLab"
                                icon.name: "kde"
                                onClicked: SettingsController.login("kde_gitlab")
                            }

                            Controls.Button {
                                text: "GNOME GitLab"
                                icon.name: "gnome"
                                onClicked: SettingsController.login("gnome_gitlab")
                            }
                        }
                    }
                }
            }
        }

        // ── Privacy & Security Card ─────────────────────────────────────────
        Controls.Pane {
            visible: advancedToggle.checked
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.alignment: Qt.AlignHCenter
            padding: Kirigami.Units.largeSpacing * 1.5

            background: Kirigami.ShadowedRectangle {
                radius: 12
                color: Kirigami.Theme.backgroundColor
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                border.width: 1
                shadow.size: 10
                shadow.color: Qt.rgba(0, 0, 0, 0.05)
            }

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "security-high-symbolic"
                        width: Kirigami.Units.iconSizes.medium
                        height: width
                        color: Kirigami.Theme.highlightColor
                        isMask: true
                    }
                    Kirigami.Heading {
                        text: i18n("Privacy & Security")
                        level: 3
                        Layout.fillWidth: true
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }

                Kirigami.FormLayout {
                    Layout.fillWidth: true

                    RowLayout {
                        Kirigami.FormData.label: i18n("Privacy User Hash:")
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Controls.TextField {
                            id: hashField
                            text: SettingsController.odrs_user_hash
                            readOnly: true
                            Layout.fillWidth: true
                            font.family: "monospace"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.9
                            
                            Controls.Button {
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.bottom: parent.bottom
                                icon.name: "edit-copy"
                                flat: true
                                Controls.ToolTip {
                                    text: i18n("Copy user hash to clipboard")
                                    visible: parent.hovered
                                }
                                onClicked: {
                                    hashField.selectAll()
                                    hashField.copy()
                                    hashField.deselect()
                                }
                            }
                        }
                    }

                    Controls.Label {
                        Layout.fillWidth: true
                        text: i18n("This hash is computed from your login identity combined with a secure local salt. It is used pseudonymously by ODRS so you can manage your reviews without disclosing your IP address or credentials.")
                        wrapMode: Text.WordWrap
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
                        color: Kirigami.Theme.disabledTextColor
                    }

                    RowLayout {
                        Kirigami.FormData.label: i18n("Privacy Actions:")
                        spacing: Kirigami.Units.mediumSpacing

                        Controls.Button {
                            text: i18n("Regenerate Salt")
                            icon.name: "view-refresh"
                            onClicked: regenerateSaltDialog.open()
                        }

                        Controls.Button {
                            text: i18n("Clear Cache")
                            icon.name: "edit-clear"
                            onClicked: SettingsController.clearCache()
                        }
                    }
                }
            }
        }

        // ── About Card ─────────────────────────────────────────────────────
        Controls.Pane {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.alignment: Qt.AlignHCenter
            padding: Kirigami.Units.largeSpacing * 1.5

            background: Kirigami.ShadowedRectangle {
                radius: 12
                color: Kirigami.Theme.backgroundColor
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                border.width: 1
                shadow.size: 10
                shadow.color: Qt.rgba(0, 0, 0, 0.05)
            }

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "help-about-symbolic"
                        width: Kirigami.Units.iconSizes.medium
                        height: width
                        color: Kirigami.Theme.highlightColor
                        isMask: true
                    }
                    Kirigami.Heading {
                        text: i18n("About Kiosque")
                        level: 3
                        Layout.fillWidth: true
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    Image {
                        source: "qrc:/qml/images/logo.svg"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.large * 1.5
                        Layout.preferredHeight: Kirigami.Units.iconSizes.large * 1.5
                        fillMode: Image.PreserveAspectFit
                        smooth: true
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Controls.Label {
                            text: "Kiosque"
                            font.bold: true
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.1
                        }

                        Controls.Label {
                            text: i18n("Version %1").arg(Qt.application.version)
                            color: Kirigami.Theme.disabledTextColor
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.9
                        }
                    }

                    Controls.Button {
                        text: i18n("More Information…")
                        icon.name: "help-about-symbolic"
                        onClicked: aboutDialog.open()
                        Layout.alignment: Qt.AlignVCenter
                    }
                }
            }
        }
    }

    function saveSettings() {
        if (!autoUpdateSwitch.checked || timeField.acceptableInput) {
            SettingsController.saveSettings(
                autoUpdateSwitch.checked,
                frequencyCombo.currentText,
                timeField.text
            )
        }
    }

    // Add Repository Dialog
    Kirigami.Dialog {
        id: addRepoDialog
        title: i18n("Add Repository")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel

        onAccepted: {
            if (repoNameField.text !== "" && repoUrlField.text !== "") {
                RepoModel.addRemote(repoNameField.text, repoUrlField.text)
                repoNameField.text = ""
                repoUrlField.text = ""
            }
        }

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            
            Controls.TextField {
                id: repoNameField
                placeholderText: i18n("Name (e.g. flathub)")
                Layout.fillWidth: true
            }

            Controls.TextField {
                id: repoUrlField
                placeholderText: i18n("URL (e.g. https://dl.flathub.org/repo/flathub.flatpakrepo)")
                Layout.fillWidth: true
            }
        }
    }

    // Regenerate Salt Confirm Dialog
    Kirigami.Dialog {
        id: regenerateSaltDialog
        title: i18n("Regenerate Privacy Salt?")
        standardButtons: Kirigami.Dialog.Yes | Kirigami.Dialog.No
        
        onAccepted: {
            SettingsController.regenerateSalt()
        }

        Controls.Label {
            text: i18n("Regenerating your local cryptographic salt will immediately assign you a completely new pseudonymous identity on the ODRS review server. Your existing reviews and votes will still exist, but they will be permanently unlinked from this machine and you will no longer be able to modify or delete them. Are you sure you want to proceed?")
            wrapMode: Text.WordWrap
            width: Kirigami.Units.gridUnit * 25
        }
    }

    Connections {
        target: SettingsController
        function onLoginFailed(error) {
            loginErrorDialog.errorMessage = error;
            loginErrorDialog.open();
        }
    }

    // Login Error Dialog
    Kirigami.Dialog {
        id: loginErrorDialog
        title: i18n("Login Failed")
        standardButtons: Kirigami.Dialog.Close
        
        property string errorMessage: ""

        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing
            width: Kirigami.Units.gridUnit * 20

            Kirigami.Icon {
                source: "dialog-warning"
                width: Kirigami.Units.iconSizes.huge
                height: width
                color: Kirigami.Theme.negativeTextColor
                Layout.alignment: Qt.AlignHCenter
            }

            Controls.Label {
                text: i18n("Could not complete OAuth login. Please check your network connection or the credentials configuration.")
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }

            Controls.Label {
                text: loginErrorDialog.errorMessage
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                font.family: "monospace"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
                color: Kirigami.Theme.negativeTextColor
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }

    // ── About dialog ────────────────────────────────────────────────────
    Controls.Dialog {
        id: aboutDialog
        anchors.centerIn: parent
        title: i18n("About Kiosque")
        standardButtons: Controls.Dialog.Close
        modal: true

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.largeSpacing

            Image {
                id: aboutLogo
                source: "qrc:/qml/images/logo.svg"
                Layout.preferredWidth: Kirigami.Units.iconSizes.huge * 1.5
                Layout.preferredHeight: Kirigami.Units.iconSizes.huge * 1.5
                sourceSize.width: 512
                sourceSize.height: 512
                fillMode: Image.PreserveAspectFit
                smooth: true
                mipmap: true
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.largeSpacing
            }

            Kirigami.Heading {
                text: "Kiosque"
                level: 1
                Layout.alignment: Qt.AlignHCenter
            }

            Controls.Label {
                text: i18n("Version %1", Qt.application.version)
                font.weight: Font.DemiBold
                color: Kirigami.Theme.highlightColor
                Layout.alignment: Qt.AlignHCenter
                Layout.bottomMargin: Kirigami.Units.largeSpacing
            }

            Controls.Label {
                text: i18n("A beautiful, fast, and modern Flatpak storefront for the KDE Plasma desktop, inspired by GNOME's software curation aesthetics.")
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            }

            Controls.Label {
                text: i18n("Kiosque was built to bring a curation-focused storefront to the Qt/KDE ecosystem, showcasing applications in their best light. It integrates a high-performance Rust backend with a modern Qt6/Kirigami frontend.")
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                color: Kirigami.Theme.disabledTextColor
                font: Kirigami.Theme.smallFont
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                Layout.topMargin: Kirigami.Units.smallSpacing
            }

            Kirigami.Separator {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.largeSpacing
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: Kirigami.Units.largeSpacing

                Controls.Button {
                    icon.name: "code-context"
                    text: i18n("Source Code")
                    flat: true
                    onClicked: Qt.openUrlExternally("https://github.com/niltonperimneto/kiosque")
                }
                Controls.Button {
                    icon.name: "tools-report-bug"
                    text: i18n("Report Bug")
                    flat: true
                    onClicked: Qt.openUrlExternally("https://github.com/niltonperimneto/kiosque/issues")
                }
            }
        }
    }
}
