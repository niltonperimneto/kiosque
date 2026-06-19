// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque
import "pages" as Pages
import "components" as Components

Kirigami.ApplicationWindow {
    id: root

    title: "Kiosque"
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 38
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 24

    pageStack.initialPage: "qrc:/qml/pages/ShopfrontPage.qml"

    property string currentSection: "shopfront"

    // ── Backend models ──────────────────────────────────────────────────
    property AppListModel appListModel: AppListModel {}
    property FeaturedModel featuredModel: FeaturedModel {}
    property InstalledModel installedModel: InstalledModel {}

    // Expose search query for page bindings (no longer bound to a header text field)
    property string searchQuery: ""
    property string currentCategory: ""

    // ── Detail page component (created fresh each time) ─────────────────
    Component {
        id: appDetailComponent
        Pages.AppDetailPage {}
    }

    function switchToPage(pagePath, properties) {
        console.warn("switchToPage called with:", pagePath, "current depth:", pageStack.depth);
        if (pageStack.depth === 0) {
            console.warn("Pushing base ShopfrontPage");
            pageStack.push("qrc:/qml/pages/TestPage.qml");
        }
        while (pageStack.depth > 1) {
            pageStack.pop();
        }
        if (pagePath !== "qrc:/qml/pages/ShopfrontPage.qml") {
            console.warn("Pushing new page:", pagePath);
            pageStack.push(pagePath, properties || {});
        }
        console.warn("Finished switchToPage. Depth is now:", pageStack.depth, "currentItem:", pageStack.currentItem);
    }

    function pushAppDetail(appId) {
        let page = appDetailComponent.createObject(null, { appId: appId });
        pageStack.push(page);
    }

    // ── Navigation button grouping ──────────────────────────────────────
    Controls.ButtonGroup {
        id: navigationGroup
    }

    // ── Global Navigation Drawer ────────────────────────────────────────
    globalDrawer: Kirigami.GlobalDrawer {
        isMenu: false

        actions: [
            Kirigami.Action {
                text: i18n("Home")
                icon.name: "go-home-symbolic"
                checked: root.currentSection === "shopfront"
                onTriggered: {
                    root.currentSection = "shopfront";
                    root.currentCategory = "";
                    root.switchToPage("qrc:/qml/pages/ShopfrontPage.qml");
                    appListModel.refresh();
                }
            },
            Kirigami.Action {
                text: i18n("Categories")
                icon.name: "applications-all-symbolic"

                Kirigami.Action {
                    text: i18n("All Categories")
                    icon.name: "applications-all-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === ""
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "", categoryName: "All Categories" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Audio & Video")
                    icon.name: "applications-multimedia-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "AudioVideo"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "AudioVideo";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "AudioVideo", categoryName: "Audio & Video" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Games")
                    icon.name: "applications-games-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Game"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Game";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Game", categoryName: "Games" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Office")
                    icon.name: "applications-office-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Office"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Office";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Office", categoryName: "Office" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Graphics")
                    icon.name: "applications-graphics-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Graphics"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Graphics";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Graphics", categoryName: "Graphics" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Development")
                    icon.name: "applications-development-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Development"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Development";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Development", categoryName: "Development" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Internet")
                    icon.name: "applications-internet-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Network"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Network";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Network", categoryName: "Internet" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Utilities")
                    icon.name: "applications-utilities-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Utility"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Utility";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Utility", categoryName: "Utilities" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Education")
                    icon.name: "applications-education-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Education"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Education";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Education", categoryName: "Education" });
                    }
                }
                Kirigami.Action {
                    text: i18n("System")
                    icon.name: "applications-system-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "System"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "System";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "System", categoryName: "System" });
                    }
                }
                Kirigami.Action {
                    text: i18n("Science")
                    icon.name: "applications-science-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Science"
                    onTriggered: {
                        root.currentSection = "category";
                        root.currentCategory = "Science";
                        root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: "Science", categoryName: "Science" });
                    }
                }
            },
            Kirigami.Action {
                text: i18n("Installed")
                icon.name: "view-list-details-symbolic"
                checked: root.currentSection === "installed"
                onTriggered: {
                    root.currentSection = "installed";
                    root.currentCategory = "";
                    root.switchToPage("qrc:/qml/pages/InstalledPage.qml");
                    installedModel.refresh();
                }
            },
            Kirigami.Action {
                text: i18n("Settings")
                icon.name: "settings-configure-symbolic"
                checked: root.currentSection === "settings"
                onTriggered: {
                    root.currentSection = "settings";
                    root.currentCategory = "";
                    root.switchToPage("qrc:/qml/pages/SettingsPage.qml");
                }
            },
            Kirigami.Action {
                text: i18n("About Kiosque")
                icon.name: "help-about-symbolic"
                onTriggered: aboutDialog.open()
            }
        ]
    }

    // ── Local categories list model ─────────────────────────────────────
    ListModel {
        id: categoriesModel
        ListElement { text: "All Categories"; icon: "applications-all-symbolic"; category: "" }
        ListElement { text: "Audio & Video"; icon: "applications-multimedia-symbolic"; category: "AudioVideo" }
        ListElement { text: "Games"; icon: "applications-games-symbolic"; category: "Game" }
        ListElement { text: "Office"; icon: "applications-office-symbolic"; category: "Office" }
        ListElement { text: "Graphics"; icon: "applications-graphics-symbolic"; category: "Graphics" }
        ListElement { text: "Development"; icon: "applications-development-symbolic"; category: "Development" }
        ListElement { text: "Internet"; icon: "applications-internet-symbolic"; category: "Network" }
        ListElement { text: "Utilities"; icon: "applications-utilities-symbolic"; category: "Utility" }
        ListElement { text: "Education"; icon: "applications-education-symbolic"; category: "Education" }
        ListElement { text: "System"; icon: "applications-system-symbolic"; category: "System" }
        ListElement { text: "Science"; icon: "applications-science-symbolic"; category: "Science" }
    }

    function getSubcategories(category) {
        if (category === "Game") {
            return [
                { text: "Games", icon: "applications-games-symbolic", categoryId: "Game-Game" },
                { text: "Emulators", icon: "input-gamepad-symbolic", categoryId: "Game-Emulator" },
                { text: "Game Launchers", icon: "system-run-symbolic", categoryId: "Game-Launcher" },
                { text: "Game Tools", icon: "applications-utilities-symbolic", categoryId: "Game-Tool" }
            ];
        } else if (category === "AudioVideo") {
            return [
                { text: "Audio & Video", icon: "applications-multimedia-symbolic", categoryId: "AudioVideo-All" },
                { text: "Players", icon: "multimedia-player-symbolic", categoryId: "AudioVideo-Player" },
                { text: "Recorders", icon: "media-record-symbolic", categoryId: "AudioVideo-Recorder" },
                { text: "Editors & Creators", icon: "document-edit-symbolic", categoryId: "AudioVideo-Editing" }
            ];
        } else if (category === "Development") {
            return [
                { text: "Developer Tools", icon: "applications-development-symbolic", categoryId: "Development-All" },
                { text: "IDEs", icon: "code-context-symbolic", categoryId: "Development-IDE" },
                { text: "Debuggers", icon: "debug-run-symbolic", categoryId: "Development-Debugger" },
                { text: "Web Development", icon: "applications-internet-symbolic", categoryId: "Development-Web" }
            ];
        } else if (category === "Graphics") {
            return [
                { text: "Graphics", icon: "applications-graphics-symbolic", categoryId: "Graphics-All" },
                { text: "3D Graphics", icon: "draw-cuboid-symbolic", categoryId: "Graphics-3D" },
                { text: "Vector Graphics", icon: "draw-bezier-curves-symbolic", categoryId: "Graphics-Vector" },
                { text: "Raster Graphics", icon: "draw-brush-symbolic", categoryId: "Graphics-Raster" },
                { text: "Photography", icon: "camera-photo-symbolic", categoryId: "Graphics-Photography" }
            ];
        } else if (category === "Office") {
            return [
                { text: "Office", icon: "applications-office-symbolic", categoryId: "Office-All" },
                { text: "Word Processors", icon: "document-new-symbolic", categoryId: "Office-WordProcessor" },
                { text: "Spreadsheets", icon: "table-symbolic", categoryId: "Office-Spreadsheet" },
                { text: "Presentations", icon: "view-presentation-symbolic", categoryId: "Office-Presentation" },
                { text: "Finance", icon: "taxes-finances-symbolic", categoryId: "Office-Finance" }
            ];
        }
        return [];
    }



    // ── Add Repository overlay sheet ────────────────────────────────────
    Kirigami.OverlaySheet {
        id: addRepoSheet
        
        title: i18n("Add Flatpak Repository")

        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing
            implicitWidth: Kirigami.Units.gridUnit * 18

            Controls.Label {
                text: i18n("Add a new Flatpak remote repository. User-scoped repositories do not require administrator privileges.")
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Controls.TextField {
                    id: repoNameField
                    Kirigami.FormData.label: i18n("Name:")
                    placeholderText: "flathub-beta"
                    Layout.fillWidth: true
                }

                Controls.TextField {
                    id: repoUrlField
                    Kirigami.FormData.label: i18n("URL:")
                    placeholderText: "https://dl.flathub.org/beta-repo/flathub-beta.flatpakrepo"
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: Kirigami.Units.mediumSpacing

                Controls.Button {
                    text: i18n("Cancel")
                    onClicked: addRepoSheet.close()
                }

                Controls.Button {
                    text: i18n("Add")
                    highlighted: true
                    enabled: repoNameField.text.length > 0 && repoUrlField.text.length > 0
                    onClicked: {
                        StoreController.addRepository(repoNameField.text, repoUrlField.text);
                        addRepoSheet.close();
                    }
                }
            }
        }
    }

    // ── Repository addition status dialog ──────────────────────────────
    Controls.Dialog {
        id: statusDialog
        anchors.centerIn: parent
        title: i18n("Repository Status")
        standardButtons: Controls.Dialog.Ok

        property string messageText: ""

        Controls.Label {
            text: statusDialog.messageText
            wrapMode: Text.WordWrap
            width: Kirigami.Units.gridUnit * 12
        }
    }

    Connections {
        target: StoreController
        function onRepositoryAdded(success, message) {
            statusDialog.messageText = message;
            statusDialog.open();
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

            Kirigami.Icon {
                id: aboutLogo
                source: "application-x-executable"
                implicitWidth: Kirigami.Units.iconSizes.huge * 1.5
                implicitHeight: Kirigami.Units.iconSizes.huge * 1.5
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.largeSpacing
            }

            Kirigami.Heading {
                text: "Kiosque"
                level: 1
                Layout.alignment: Qt.AlignHCenter
            }

            Controls.Label {
                text: i18n("Version 0.1.0")
                font.weight: Font.DemiBold
                color: Kirigami.Theme.highlightColor
                Layout.alignment: Qt.AlignHCenter
                Layout.bottomMargin: Kirigami.Units.largeSpacing
            }

            Controls.Label {
                text: i18n("A modern, lightweight Flatpak software center for KDE Plasma.")
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            }

            Controls.Label {
                text: i18n("Built with Rust and Kirigami, designed to provide a fast and seamless experience for discovering and managing your applications.")
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                color: Kirigami.Theme.disabledTextColor
                font.pointSize: Kirigami.Theme.smallFont.pointSize
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
                    onClicked: Qt.openUrlExternally("https://github.com/Kiosque/kiosque")
                }
                Controls.Button {
                    icon.name: "tools-report-bug"
                    text: i18n("Report Bug")
                    flat: true
                    onClicked: Qt.openUrlExternally("https://github.com/Kiosque/kiosque/issues")
                }
            }
        }
    }


    // ── Startup ─────────────────────────────────────────────────────────
    Component.onCompleted: {
        featuredModel.refresh();
        appListModel.refresh();
        installedModel.refresh();
    }
}
