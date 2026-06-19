import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque

Kirigami.ScrollablePage {
    id: page
    title: i18n("Settings")

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]

    Component.onCompleted: {
        SettingsController.loadSettings()
        RepoModel.refresh()
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing * 2
        width: parent.width
        
        // Add margins to make the cards breathe nicely on the page
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        Layout.rightMargin: Kirigami.Units.largeSpacing * 2
        Layout.topMargin: Kirigami.Units.largeSpacing

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
                        onCurrentIndexChanged: saveSettings()
                    }

                    Controls.TextField {
                        id: timeField
                        Kirigami.FormData.label: i18n("Time (HH:MM):")
                        text: SettingsController.update_time
                        placeholderText: "02:00"
                        enabled: autoUpdateSwitch.checked
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
                    
                    delegate: Kirigami.SwipeListItem {
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
}
