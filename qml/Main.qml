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
import org.kde.kirigamiaddons.components as KirigamiAddons

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
        id: mainDrawer
        isMenu: false

        property real customWidth: Kirigami.Units.gridUnit * 16
        width: !collapsed ? customWidth : collapsedSize

        header: MouseArea {
            id: headerArea
            implicitWidth: parent.width
            implicitHeight: mainDrawer.collapsed ? Kirigami.Units.gridUnit * 3 : Kirigami.Units.gridUnit * 4.5
            hoverEnabled: true
            
            onClicked: {
                mainDrawer.close();
                root.currentSection = "settings";
                root.currentCategory = "";
                root.switchToPage("qrc:/qml/pages/SettingsPage.qml");
            }

            Rectangle {
                anchors.fill: parent
                color: headerArea.containsMouse ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.1) : "transparent"
            }

            RowLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                Item {
                    id: avatarContainer
                    Layout.preferredWidth: (mainDrawer.collapsed ? Kirigami.Units.iconSizes.medium : Kirigami.Units.iconSizes.large) + 8
                    Layout.preferredHeight: Layout.preferredWidth
                    Layout.alignment: Qt.AlignVCenter

                    Kirigami.ShadowedRectangle {
                        id: avatarGlowRing
                        anchors.fill: parent
                        radius: width / 2
                        color: "transparent"

                        border.width: 1.5
                        border.color: headerArea.containsMouse
                            ? Kirigami.Theme.highlightColor
                            : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.3)

                        shadow.size: headerArea.containsMouse ? 10 : 4
                        shadow.color: headerArea.containsMouse
                            ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.4)
                            : Qt.rgba(0, 0, 0, 0.15)
                        shadow.yOffset: headerArea.containsMouse ? 2 : 1

                        Behavior on border.color { ColorAnimation { duration: 150 } }
                        Behavior on shadow.size { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                        Behavior on shadow.color { ColorAnimation { duration: 150 } }
                    }

                    KirigamiAddons.Avatar {
                        id: userAvatar
                        anchors.centerIn: parent
                        width: parent.width - 8
                        height: width
                        source: SettingsController.is_authenticated && SettingsController.oauth_avatar_url !== "" ? SettingsController.oauth_avatar_url : ""
                        name: SettingsController.is_authenticated ? SettingsController.oauth_username : ""
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: !mainDrawer.collapsed
                    spacing: 0
                    Layout.alignment: Qt.AlignVCenter

                    Controls.Label {
                        text: SettingsController.is_authenticated ? SettingsController.oauth_username : i18n("Sign In")
                        font.bold: true
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }

                    Controls.Label {
                        text: SettingsController.is_authenticated ? i18n("Settings & Reviews") : i18n("Local Profile")
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        color: Kirigami.Theme.disabledTextColor
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
            }
        }

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
                        RepoModel.addRemote(repoNameField.text, repoUrlField.text);
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
        target: RepoModel
        function onRemoteAdded(success, message) {
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
                text: i18n("Version 0.1.0")
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


    // ── Startup ─────────────────────────────────────────────────────────
    Component.onCompleted: {
        SettingsController.loadSettings();
        featuredModel.refresh();
        appListModel.refresh();
        installedModel.refresh();
    }

    // ── Resizable Sidebar Handle ────────────────────────────────────────
    Rectangle {
        id: drawerResizeHandle
        parent: root.overlay
        x: mainDrawer.position * mainDrawer.width - width / 2
        y: 0
        width: Kirigami.Units.smallSpacing * 2
        height: parent.height
        color: Kirigami.Theme.highlightColor
        opacity: handleMouseArea.containsMouse || handleMouseArea.drag.active ? 0.3 : 0.0
        visible: !mainDrawer.collapsed && mainDrawer.position > 0 && !mainDrawer.modal
        z: 9999

        MouseArea {
            id: handleMouseArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.SplitHCursor
            drag.target: dummyDragItem
            drag.axis: Drag.XAxis
            
            property real startWidth: 0
            
            onPressed: {
                startWidth = mainDrawer.customWidth
            }
            onPositionChanged: {
                if (drag.active) {
                    let newWidth = startWidth + dummyDragItem.x
                    if (newWidth > Kirigami.Units.gridUnit * 12 && newWidth < Kirigami.Units.gridUnit * 40) {
                        mainDrawer.customWidth = newWidth
                    }
                }
            }
            onReleased: {
                dummyDragItem.x = 0
            }
        }
        Item {
            id: dummyDragItem
        }
    }
}
