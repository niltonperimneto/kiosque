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
    property alias categoriesModel: categoriesModel

    // Expose search query for page bindings (no longer bound to a header text field)
    property string searchQuery: ""
    property string currentCategory: ""

    // ── Detail page component (created fresh each time) ─────────────────
    Component {
        id: appDetailComponent
        Pages.AppDetailPage {}
    }

    function switchToPage(pagePath, properties) {
        // Collapse back to the root page before switching sections.
        while (pageStack.depth > 1) {
            pageStack.pop();
        }
        if (pagePath !== "qrc:/qml/pages/ShopfrontPage.qml") {
            pageStack.push(pagePath, properties || {});
        }
    }

    function pushAppDetail(appId) {
        // Find if there is already an AppDetailPage in the pageStack
        let existingPage = null;
        for (let i = 0; i < pageStack.depth; i++) {
            let page = pageStack.get(i);
            if (page && page.appId !== undefined) {
                existingPage = page;
                break;
            }
        }

        if (existingPage !== null) {
            // If the existing page has the same appId, we just pop back to it and we're done
            if (existingPage.appId === appId) {
                pageStack.pop(existingPage);
                return;
            }
            
            // Otherwise, we pop back to the page preceding the existing AppDetailPage,
            // and then push the new AppDetailPage
            let precedingPage = null;
            let index = -1;
            for (let i = 0; i < pageStack.depth; i++) {
                if (pageStack.get(i) === existingPage) {
                    index = i;
                    break;
                }
            }
            if (index > 0) {
                precedingPage = pageStack.get(index - 1);
            }
            
            if (precedingPage !== null) {
                pageStack.pop(precedingPage);
            } else {
                while (pageStack.depth > 1) {
                    pageStack.pop();
                }
            }
        }

        let newPage = appDetailComponent.createObject(null, { appId: appId });
        pageStack.push(newPage);
    }

    // ── Navigation button grouping ──────────────────────────────────────
    Controls.ButtonGroup {
        id: navigationGroup
    }

    // ── Global Navigation Drawer ────────────────────────────────────────
    globalDrawer: Kirigami.GlobalDrawer {
        id: mainDrawer
        isMenu: false
        // Desktop: persistent collapsible sidebar. Mobile: modal overlay drawer.
        modal: Kirigami.Settings.isMobile
        collapsible: !Kirigami.Settings.isMobile

        // Start retracted to the icon-only rail on desktop, giving content the
        // full width; the user can expand it via the handle.
        Component.onCompleted: if (!Kirigami.Settings.isMobile) collapsed = true;

        // Standard fixed sidebar width; the built-in handle collapses it to an
        // icon-only rail (collapsedSize) rather than allowing free resizing.
        width: !collapsed ? Kirigami.Units.gridUnit * 16 : collapsedSize

        header: Controls.ItemDelegate {
            id: headerDelegate
            implicitWidth: parent ? parent.width : 0
            implicitHeight: mainDrawer.collapsed ? Kirigami.Units.gridUnit * 3 : Kirigami.Units.gridUnit * 4.5
            padding: Kirigami.Units.largeSpacing

            // Inherit the drawer's palette and keep the rest state transparent so
            // the header blends with the drawer background instead of painting a
            // mismatched View/Button-coloured block.
            Kirigami.Theme.inherit: true

            background: Rectangle {
                color: headerDelegate.pressed
                    ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2)
                    : headerDelegate.hovered
                        ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.1)
                        : "transparent"
                Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
            }

            Controls.ToolTip.text: i18n("Open Settings")
            Controls.ToolTip.visible: mainDrawer.collapsed && hovered
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay

            onClicked: {
                if (Kirigami.Settings.isMobile) {
                    mainDrawer.close();
                }
                root.currentSection = "settings";
                root.currentCategory = "";
                root.switchToPage("qrc:/qml/pages/SettingsPage.qml");
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.mediumSpacing

                KirigamiAddons.Avatar {
                    id: userAvatar
                    Layout.preferredWidth: mainDrawer.collapsed ? Kirigami.Units.iconSizes.medium : Kirigami.Units.iconSizes.large
                    Layout.preferredHeight: Layout.preferredWidth
                    Layout.alignment: Qt.AlignVCenter
                    source: SettingsController.is_authenticated && SettingsController.oauth_avatar_url !== "" ? SettingsController.oauth_avatar_url : ""
                    name: SettingsController.is_authenticated ? SettingsController.oauth_username : ""
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: !mainDrawer.collapsed
                    spacing: 0
                    Layout.alignment: Qt.AlignVCenter

                    Controls.Label {
                        text: SettingsController.is_authenticated ? SettingsController.oauth_username : i18n("Sign In")
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }

                    Controls.Label {
                        text: SettingsController.is_authenticated ? i18n("Settings & Reviews") : i18n("Local Profile")
                        font: Kirigami.Theme.smallFont
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
                id: categoriesAction
                text: i18n("Categories")
                icon.name: "applications-all-symbolic"
                checked: root.currentSection === "category" || root.currentSection === "categories"
                onTriggered: {
                    if (mainDrawer.collapsed) {
                        mainDrawer.collapsed = false;
                    }
                }

                // Helper used by every child to navigate to a category listing.
                function openCategory(catId, catName) {
                    root.currentSection = "category";
                    root.currentCategory = catId;
                    root.switchToPage("qrc:/qml/pages/CategoryAppListPage.qml", { categoryId: catId, categoryName: catName });
                }

                Kirigami.Action {
                    text: i18n("All Categories")
                    icon.name: "applications-all-symbolic"
                    checked: root.currentSection === "categories"
                    onTriggered: {
                        root.currentSection = "categories";
                        root.currentCategory = "";
                        root.switchToPage("qrc:/qml/pages/CategoriesPage.qml");
                    }
                }
                Kirigami.Action {
                    text: i18n("Audio & Video")
                    icon.name: "applications-multimedia-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "AudioVideo"
                    onTriggered: categoriesAction.openCategory("AudioVideo", i18n("Audio & Video"))
                }
                Kirigami.Action {
                    text: i18n("Games")
                    icon.name: "applications-games-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Game"
                    onTriggered: categoriesAction.openCategory("Game", i18n("Games"))
                }
                Kirigami.Action {
                    text: i18n("Office")
                    icon.name: "applications-office-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Office"
                    onTriggered: categoriesAction.openCategory("Office", i18n("Office"))
                }
                Kirigami.Action {
                    text: i18n("Graphics")
                    icon.name: "applications-graphics-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Graphics"
                    onTriggered: categoriesAction.openCategory("Graphics", i18n("Graphics"))
                }
                Kirigami.Action {
                    text: i18n("Development")
                    icon.name: "applications-development-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Development"
                    onTriggered: categoriesAction.openCategory("Development", i18n("Development"))
                }
                Kirigami.Action {
                    text: i18n("Internet")
                    icon.name: "applications-internet-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Network"
                    onTriggered: categoriesAction.openCategory("Network", i18n("Internet"))
                }
                Kirigami.Action {
                    text: i18n("Utilities")
                    icon.name: "applications-utilities-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Utility"
                    onTriggered: categoriesAction.openCategory("Utility", i18n("Utilities"))
                }
                Kirigami.Action {
                    text: i18n("Education")
                    icon.name: "applications-education-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Education"
                    onTriggered: categoriesAction.openCategory("Education", i18n("Education"))
                }
                Kirigami.Action {
                    text: i18n("System")
                    icon.name: "applications-system-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "System"
                    onTriggered: categoriesAction.openCategory("System", i18n("System"))
                }
                Kirigami.Action {
                    text: i18n("Science")
                    icon.name: "applications-science-symbolic"
                    checked: root.currentSection === "category" && root.currentCategory === "Science"
                    onTriggered: categoriesAction.openCategory("Science", i18n("Science"))
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




    // ── Startup ─────────────────────────────────────────────────────────
    Component.onCompleted: {
        SettingsController.loadSettings();
        featuredModel.refresh();
        appListModel.refresh();
        installedModel.refresh();
    }

}
