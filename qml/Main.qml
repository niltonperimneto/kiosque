// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import org.kde.ki18n
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque
import "pages" as Pages
import "components" as Components
import org.kde.kirigamiaddons.components as KirigamiAddons

Kirigami.ApplicationWindow {
    id: root

    // KDE Addons resolve these functions in their creation context.
    KI18nContext { id: kdeTranslations; translationDomain: "kiosque" }
    function i18nd() { return kdeTranslations.i18nd.apply(kdeTranslations, arguments); }
    function i18ndc() { return kdeTranslations.i18ndc.apply(kdeTranslations, arguments); }
    function i18ndp() { return kdeTranslations.i18ndp.apply(kdeTranslations, arguments); }
    function i18ndcp() { return kdeTranslations.i18ndcp.apply(kdeTranslations, arguments); }
    RemoteImage { id: userAvatar; url: SettingsController.oauth_avatar_url; kind: "avatar" }

    readonly property string pageTitle: pageStack.currentItem && pageStack.currentItem.title !== undefined
                                        ? pageStack.currentItem.title : ""
    title: pageTitle !== "" && pageStack.depth > 1 ? L10n.i18nc("@title:window %1 is the page", "%1 — Kiosque", [pageTitle])
                                                   : L10n.i18nc("@title:window", "Kiosque", [])
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
    // Every installation, removal and update, for the Tasks dialog.
    property JobsModel jobsModel: JobsModel { Component.onCompleted: refresh() }

    property string currentCategory: ""

    // ── Detail page component (created fresh each time) ─────────────────
    // Pushed as a Component so the page stack owns each page and destroys it
    // (and its AppDetails subscription) when it is popped.
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
        // At most one detail page lives in the stack. Reopening the same app
        // returns to it; another app replaces it (and anything above it).
        for (let i = 0; i < pageStack.depth; i++) {
            let page = pageStack.get(i);
            if (page && page.details !== undefined) {
                if (page.appId === appId) {
                    pageStack.pop(page);
                    return;
                }
                if (i > 0) {
                    pageStack.pop(pageStack.get(i - 1));
                } else {
                    while (pageStack.depth > 1) {
                        pageStack.pop();
                    }
                }
                break;
            }
        }
        pageStack.push(appDetailComponent, { appId: appId });
    }

    // Open the target requested on the command line (appstream:// URI,
    // --app <id>, a .flatpakref or a .flatpakrepo). The context properties are
    // always set by main.cpp, but are guarded so test harnesses without them
    // still load. Files are never installed or added without confirmation.
    function openLaunchTarget() {
        if (AppShell.initialAppId !== "") {
            root.pushAppDetail(AppShell.initialAppId);
        }
        if (AppShell.initialRefFile !== "") {
            root.openRefFile(AppShell.initialRefFile);
        }
        if (AppShell.initialRepoFile !== "") {
            root.openRepoFile(AppShell.initialRepoFile);
        }
    }

    function openRefFile(path) {
        refFileDialog.inspect(path);
    }

    function openRepoFile(path, name) {
        repoFileDialog.inspect(path, name);
    }

    // Manual "Add Repository": asks for a .flatpakrepo address, then inspects it.
    function openAddRepository() {
        repoFileDialog.openManual();
    }

    // ── Navigation ──────────────────────────────────────────────────────
    // Narrow windows and phones use a bottom tab bar instead of the sidebar.
    readonly property bool compact: Kirigami.Settings.isMobile || root.width < Kirigami.Units.gridUnit * 30

    // One page at a time: sections and details replace the view (with the
    // toolbar's back button) instead of squeezing previous pages into columns.
    pageStack.columnView.columnResizeMode: Kirigami.ColumnView.SingleColumn
    pageStack.globalToolBar.showNavigationButtons: Kirigami.ApplicationHeaderStyle.ShowBackButton

    readonly property int updatesCount: installedModel.updates_available_count

    // Top-level destinations shared by the sidebar and the tab bar.
    function navigate(section) {
        root.currentCategory = "";
        root.currentSection = section;
        switch (section) {
        case "shopfront":
            root.switchToPage("qrc:/qml/pages/ShopfrontPage.qml");
            // Home shows the shelves, not a search left open earlier.
            if (root.pageStack.currentItem && root.pageStack.currentItem.searchBar) {
                root.pageStack.currentItem.searchBar.close();
            }
            break;
        case "search":
            // Search lives on the storefront: go there with the bar down.
            root.openSearch("");
            break;
        case "categories":
            root.switchToPage("qrc:/qml/pages/CategoriesPage.qml");
            break;
        case "installed":
            root.switchToPage("qrc:/qml/pages/InstalledPage.qml");
            installedModel.refresh();
            break;
        case "updates":
            root.switchToPage("qrc:/qml/pages/UpdatesPage.qml");
            break;
        case "settings":
            root.switchToPage("qrc:/qml/pages/SettingsPage.qml");
            break;
        }
    }

    // Search the whole catalog: show the storefront and pull its search bar
    // down with `query` typed in.
    function openSearch(query) {
        root.currentCategory = "";
        root.currentSection = "shopfront";
        root.switchToPage("qrc:/qml/pages/ShopfrontPage.qml");
        Qt.callLater(function() {
            const page = root.pageStack.currentItem;
            if (page && page.openSearch) {
                page.openSearch(query || "");
            }
        });
    }

    // Search the current page in place when it can (storefront, a category,
    // Installed); otherwise search the whole catalog.
    function searchHere(query) {
        const page = root.pageStack.currentItem;
        if (page && page.hasOwnSearch === true && page.openSearch) {
            page.openSearch(query || "");
        } else {
            root.openSearch(query);
        }
    }

    // True while the storefront's search bar is pulled down.
    readonly property bool storefrontSearching: root.currentSection === "shopfront"
        && root.pageStack.currentItem !== null && root.pageStack.currentItem.searchActive === true

    function openCategories() {
        root.navigate("categories");
    }

    function signIn(provider) {
        root.navigate("settings");
        SettingsController.login(provider);
    }

    readonly property list<Kirigami.Action> navigationActions: [
        Kirigami.Action {
            objectName: "navHome"
            text: L10n.i18n("Home", [])
            icon.name: "go-home-symbolic"
            checked: root.currentSection === "shopfront" && !root.storefrontSearching
            onTriggered: root.navigate("shopfront")
        },
        Kirigami.Action {
            objectName: "navSearch"
            text: L10n.i18n("Search", [])
            icon.name: "search-symbolic"
            checked: root.storefrontSearching
            onTriggered: root.navigate("search")
        },
        Kirigami.Action {
            objectName: "navCategories"
            text: L10n.i18n("Categories", [])
            icon.name: "applications-all-symbolic"
            checked: root.currentSection === "categories" || root.currentSection === "category"
            onTriggered: root.navigate("categories")
        },
        Kirigami.Action {
            objectName: "navInstalled"
            text: L10n.i18n("Installed", [])
            icon.name: "view-list-details-symbolic"
            checked: root.currentSection === "installed"
            onTriggered: root.navigate("installed")
        },
        Kirigami.Action {
            objectName: "navUpdates"
            text: root.updatesCount > 0 ? L10n.i18n("Updates (%1)", [root.updatesCount]) : L10n.i18n("Updates", [])
            icon.name: root.updatesCount > 0 ? "update-none-symbolic" : "system-software-update-symbolic"
            checked: root.currentSection === "updates"
            onTriggered: root.navigate("updates")
        },
        Kirigami.Action {
            objectName: "navSettings"
            text: L10n.i18n("Settings", [])
            icon.name: "settings-configure-symbolic"
            checked: root.currentSection === "settings"
            onTriggered: root.navigate("settings")
        },
        // Shown once something was installed, removed or updated this session.
        Kirigami.Action {
            objectName: "navTasks"
            text: root.jobsModel.active_count > 0
                ? L10n.i18n("Tasks (%1)", [root.jobsModel.active_count]) : L10n.i18n("Tasks", [])
            icon.name: root.jobsModel.active_count > 0 ? "process-working-symbolic" : "view-task"
            visible: root.jobsModel.active_count + root.jobsModel.finished_count > 0
            checkable: false
            onTriggered: tasksDialog.open()
        }
    ]

    // Handle typing after the focused control has had a chance to use the key.
    pageStack.Keys.priority: Keys.AfterItem
    pageStack.Keys.onPressed: (event) => {
        const modifiers = Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier;
        if ((event.modifiers & modifiers) || event.text.trim() === ""
                || event.text.charCodeAt(0) < 32 || event.text === "\u007f") {
            return;
        }
        root.searchHere(event.text);
        event.accepted = true;
    }

    // Ctrl+F: pull the current page's search bar down, or search the catalog
    // from the storefront.
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: root.searchHere("")
    }

    // Ctrl+R / F5: reload the current page.
    Shortcut {
        sequences: [StandardKey.Refresh, "Ctrl+R"]
        onActivated: {
            const page = root.pageStack.currentItem;
            if (page && typeof page.reload === "function") {
                page.reload();
            } else if (page && page.supportsRefreshing) {
                page.refreshing = true;
            }
        }
    }

    footer: Kirigami.NavigationTabBar {
        objectName: "navigationTabBar"
        visible: root.compact
        // The drawer is sized from the footer height, visible or not.
        height: visible ? implicitHeight : 0
        // Categories stay reachable from Home and Search on small screens.
        actions: [root.navigationActions[0], root.navigationActions[1], root.navigationActions[3],
                  root.navigationActions[4], root.navigationActions[5], root.navigationActions[6]]
    }

    // ── Sidebar ─────────────────────────────────────────────────────────
    globalDrawer: Kirigami.GlobalDrawer {
        id: mainDrawer
        objectName: "mainDrawer"
        isMenu: false
        modal: false
        drawerOpen: !root.compact
        interactive: false
        handleVisible: false
        collapsible: !Kirigami.Settings.isMobile

        // Swap navigation at the breakpoint without sliding the page sideways.
        enter: Transition {}
        exit: Transition {}

        Connections {
            target: root
            function onCompactChanged() {
                mainDrawer.drawerOpen = !root.compact;
            }
        }

        // Expanded on roomy desktop windows, an icon rail otherwise; the user
        // can toggle it with the handle at the bottom.
        Component.onCompleted: {
        Qt.application.name = "Kiosque";
        Qt.application.displayName = "Kiosque";
        Qt.application.organization = "Kiosque";
        Qt.application.domain = "kiosque.app";
        Qt.application.version = AppShell.version;
        if (AppShell.checkUi) return;
            if (collapsible) {
                collapsed = root.width < Kirigami.Units.gridUnit * 44;
            }
        }

        width: root.compact ? 0 : (collapsed ? collapsedSize : Kirigami.Units.gridUnit * 14)

        header: Controls.ItemDelegate {
            id: headerDelegate
            objectName: "accountButton"
            implicitWidth: parent ? parent.width : 0
            implicitHeight: mainDrawer.collapsed ? Kirigami.Units.gridUnit * 3 : Kirigami.Units.gridUnit * 4
            padding: Kirigami.Units.largeSpacing

            Accessible.name: SettingsController.is_authenticated
                ? L10n.i18n("Account: %1", [SettingsController.oauth_username])
                : L10n.i18n("Sign in to write reviews", [])
            Controls.ToolTip.text: Accessible.name
            Controls.ToolTip.visible: mainDrawer.collapsed && hovered
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay

            onClicked: (SettingsController.is_authenticated ? accountMenu : signInMenu).popup(headerDelegate, 0, headerDelegate.height)

            contentItem: RowLayout {
                spacing: Kirigami.Units.mediumSpacing

                KirigamiAddons.Avatar {
                    Layout.preferredWidth: mainDrawer.collapsed ? Kirigami.Units.iconSizes.medium : Kirigami.Units.iconSizes.large
                    Layout.preferredHeight: Layout.preferredWidth
                    Layout.alignment: Qt.AlignVCenter
                    source: SettingsController.is_authenticated ? userAvatar.source : ""
                    name: SettingsController.is_authenticated ? SettingsController.oauth_username : ""
                    initialsMode: SettingsController.is_authenticated
                        ? KirigamiAddons.Avatar.InitialsMode.UseInitials
                        : KirigamiAddons.Avatar.InitialsMode.UseIcon
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: !mainDrawer.collapsed
                    spacing: 0
                    Layout.alignment: Qt.AlignVCenter

                    Controls.Label {
                        text: SettingsController.is_authenticated ? SettingsController.oauth_username : L10n.i18n("Sign In", [])
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }

                    Controls.Label {
                        text: SettingsController.is_authenticated ? L10n.i18n("Reviews account", []) : L10n.i18n("To rate and review apps", [])
                        font: Kirigami.Theme.smallFont
                        color: Kirigami.Theme.disabledTextColor
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }

                Kirigami.Icon {
                    visible: !mainDrawer.collapsed
                    source: "arrow-down-symbolic"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: implicitWidth
                }
            }

            Controls.Menu {
                id: signInMenu
                objectName: "signInMenu"
                Controls.MenuItem {
                    text: L10n.i18n("Sign in with GitHub", [])
                    icon.source: "qrc:/qml/images/github-symbolic.svg"
                    icon.color: pressed ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                    enabled: !SettingsController.login_in_progress
                    onTriggered: root.signIn("github")
                }
                Controls.MenuItem {
                    text: L10n.i18n("Sign in with GitLab", [])
                    icon.source: "qrc:/qml/images/gitlab-symbolic.svg"
                    icon.color: pressed ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                    enabled: !SettingsController.login_in_progress
                    onTriggered: root.signIn("gitlab")
                }
                Controls.MenuItem {
                    text: L10n.i18n("Sign in with KDE GitLab", [])
                    icon.source: "qrc:/qml/images/kde_gitlab-symbolic.svg"
                    icon.color: pressed ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                    enabled: !SettingsController.login_in_progress
                    onTriggered: root.signIn("kde_gitlab")
                }
                Controls.MenuItem {
                    text: L10n.i18n("Sign in with GNOME GitLab", [])
                    icon.source: "qrc:/qml/images/gnome_gitlab-symbolic.svg"
                    icon.color: pressed ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                    enabled: !SettingsController.login_in_progress
                    onTriggered: root.signIn("gnome_gitlab")
                }
            }

            Controls.Menu {
                id: accountMenu
                objectName: "accountMenu"
                Controls.MenuItem {
                    text: L10n.i18n("Account Settings", [])
                    icon.name: "configure"
                    onTriggered: root.navigate("settings")
                }
                Controls.MenuSeparator {}
                Controls.MenuItem {
                    text: L10n.i18n("Sign Out", [])
                    icon.name: "system-log-out"
                    onTriggered: SettingsController.logout()
                }
            }
        }

        actions: root.navigationActions
    }

    // ── Category tree (single source, translated) ───────────────────────
    property Components.Categories categories: Components.Categories {}
    // Kept for pages that list the top-level categories.
    readonly property var categoriesModel: categories.all

    function getSubcategories(category) {
        return categories.subcategories(category);
    }

    // ── Repository addition status dialog ──────────────────────────────
    Controls.Dialog {
        id: statusDialog
        objectName: "repositoryStatusDialog"
        anchors.centerIn: parent
        title: L10n.i18n("Repository Status", [])
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
            statusDialog.messageText = success
                ? L10n.i18n("Repository added successfully.", [])
                : L10n.i18n("Could not add the repository: %1", [message]);
            statusDialog.open();
        }
        function onRemoteRemoved(success, message) {
            statusDialog.messageText = success
                ? L10n.i18n("Repository removed successfully.", [])
                : L10n.i18n("Could not remove the repository: %1", [message]);
            statusDialog.open();
        }
    }

    // ── .flatpakref confirmation and install progress ──────────────────
    // phase: "loading" → "confirm" → "installing" → "done" | "failed".
    Kirigami.Dialog {
        id: refFileDialog
        objectName: "refFileDialog"

        property string phase: "loading"
        property string source: ""
        property string appTitle: ""
        property string appName: ""
        property string originUrl: ""
        property bool isRuntime: false
        property string errorText: ""

        function inspect(path) {
            source = path;
            appTitle = "";
            appName = "";
            originUrl = "";
            isRuntime = false;
            errorText = "";
            phase = "loading";
            open();
            StoreController.inspectRefFile(path);
        }

        title: phase === "installing" ? L10n.i18n("Installing…", [])
             : phase === "done" ? L10n.i18n("Installation Complete", [])
             : phase === "failed" ? L10n.i18n("Installation Failed", [])
             : isRuntime ? L10n.i18n("Install Runtime?", []) : L10n.i18n("Install Application?", [])
        closePolicy: phase === "installing" ? Controls.Popup.NoAutoClose
                                            : Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutside
        showCloseButton: phase !== "installing"
        preferredWidth: Kirigami.Units.gridUnit * 24
        padding: Kirigami.Units.largeSpacing

        onRejected: {
            if (phase === "loading" || phase === "confirm") {
                StoreController.discardRefFile();
            }
        }

        // The buttons change with the phase, so they live in the content: the
        // footer DialogButtonBox mis-positions buttons swapped after opening.

        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing

            Controls.BusyIndicator {
                Layout.alignment: Qt.AlignHCenter
                visible: refFileDialog.phase === "loading"
                running: visible
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: refFileDialog.phase === "loading"
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: L10n.i18n("Reading %1…", [refFileDialog.source])
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true
                visible: refFileDialog.phase === "confirm" || refFileDialog.phase === "installing"
                         || refFileDialog.phase === "done"

                Controls.Label {
                    objectName: "refTitleLabel"
                    Kirigami.FormData.label: L10n.i18n("Name:", [])
                    text: refFileDialog.appTitle
                    textFormat: Text.PlainText
                    font.weight: Font.Bold
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                Controls.Label {
                    Kirigami.FormData.label: L10n.i18n("Identifier:", [])
                    text: refFileDialog.appName
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    Layout.fillWidth: true
                }
                Controls.Label {
                    objectName: "refOriginLabel"
                    Kirigami.FormData.label: L10n.i18n("Source:", [])
                    text: refFileDialog.originUrl !== "" ? refFileDialog.originUrl : L10n.i18n("Unknown", [])
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    Layout.fillWidth: true
                }
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: refFileDialog.phase === "confirm"
                text: L10n.i18n("Repository signing key SHA-256: %1", [StoreController.ref_key_sha256])
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                visible: refFileDialog.phase === "confirm"
                type: Kirigami.MessageType.Warning
                text: L10n.i18n("This file was opened from outside Kiosque. Only install software from sources you trust. It will be installed for your user only, and its repository may be added.", [])
            }

            Controls.ProgressBar {
                objectName: "refInstallProgress"
                Layout.fillWidth: true
                visible: refFileDialog.phase === "installing"
                from: 0
                to: 1
                value: Math.max(0, StoreController.install_progress)
                indeterminate: StoreController.install_progress < 0.01
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                visible: refFileDialog.phase === "failed"
                type: Kirigami.MessageType.Error
                text: refFileDialog.errorText
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                visible: refFileDialog.phase === "done"
                type: Kirigami.MessageType.Positive
                text: L10n.i18n("%1 was installed.", [refFileDialog.appTitle])
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: Kirigami.Units.smallSpacing

                Controls.Button {
                    objectName: "refInstallButton"
                    visible: refFileDialog.phase === "confirm"
                    text: L10n.i18n("Install", [])
                    icon.name: "download"
                    onClicked: {
                        refFileDialog.phase = "installing";
                        StoreController.confirmRefFileInstall();
                    }
                }
                Controls.Button {
                    objectName: "refCancelButton"
                    visible: refFileDialog.phase === "loading" || refFileDialog.phase === "confirm"
                    text: L10n.i18n("Cancel", [])
                    icon.name: "dialog-cancel"
                    onClicked: {
                        StoreController.discardRefFile();
                        refFileDialog.close();
                    }
                }
                Controls.Button {
                    objectName: "refBackgroundButton"
                    visible: refFileDialog.phase === "installing"
                    text: L10n.i18n("Continue in Background", [])
                    icon.name: "window-minimize"
                    onClicked: refFileDialog.close()
                }
                Controls.Button {
                    objectName: "refCancelInstallButton"
                    visible: refFileDialog.phase === "installing"
                    text: L10n.i18n("Cancel Installation", [])
                    icon.name: "dialog-cancel"
                    onClicked: StoreController.cancelOperation()
                }
                Controls.Button {
                    objectName: "refCloseButton"
                    visible: refFileDialog.phase === "done" || refFileDialog.phase === "failed"
                    text: L10n.i18n("Close", [])
                    icon.name: "dialog-close"
                    onClicked: refFileDialog.close()
                }
            }
        }
    }

    Connections {
        target: StoreController
        function onRefFileInspected(success, title, name, url, isRuntime, error) {
            if (refFileDialog.phase !== "loading") {
                return;
            }
            if (success) {
                refFileDialog.appTitle = title !== "" ? title : name;
                refFileDialog.appName = name;
                refFileDialog.originUrl = url;
                refFileDialog.isRuntime = isRuntime;
                refFileDialog.phase = "confirm";
            } else {
                refFileDialog.errorText = L10n.i18n("Could not open the file: %1", [error]);
                refFileDialog.phase = "failed";
            }
        }
        function onInstallFinished(success) {
            if (refFileDialog.phase !== "installing") {
                return;
            }
            if (success) {
                refFileDialog.phase = "done";
            } else if (StoreController.error_message !== "") {
                refFileDialog.errorText = StoreController.error_message;
                refFileDialog.phase = "failed";
            } else {
                // Cancelled by the user.
                refFileDialog.close();
            }
        }
    }

    // ── Background tasks ────────────────────────────────────────────────
    Components.TasksDialog {
        id: tasksDialog
        jobs: root.jobsModel
    }

    function openTasks() {
        tasksDialog.open();
    }

    // Report finished work in the window, or on the desktop when Kiosque is
    // in the background. Cancellations were requested by the user and are
    // not announced.
    Connections {
        target: root.jobsModel
        function onJobFinished(title, kind, state, error) {
            let message = "";
            if (state === "done") {
                message = kind === "uninstall" ? L10n.i18n("%1 was removed.", [title])
                        : kind === "update" ? L10n.i18n("%1 was updated.", [title])
                        : L10n.i18n("%1 was installed.", [title]);
            } else if (state === "failed") {
                message = kind === "uninstall" ? L10n.i18n("Could not remove %1: %2", [title, error])
                        : kind === "update" ? L10n.i18n("Could not update %1: %2", [title, error])
                        : L10n.i18n("Could not install %1: %2", [title, error]);
            } else {
                return;
            }
            if (root.active) {
                root.showPassiveNotification(message, state === "failed" ? "long" : "short",
                                             L10n.i18n("Details", []), root.openTasks);
            } else {
                Qt.callLater(root.jobsModel.notifyDesktop, state === "failed" ? L10n.i18n("Operation Failed", []) : L10n.i18n("Kiosque", []),
                                             message,
                                             state === "failed" ? "dialog-error" : "system-software-install");
            }
        }
    }

    // ── Repository addition (.flatpakrepo or manual URL) ────────────────
    Components.AddRepositoryDialog {
        id: repoFileDialog
    }

    // ── Auto-check for updates ──────────────────────────────────────────
    property bool initialUpdateCheckDone: false

    Timer {
        id: autoUpdateCheckTimer
        interval: 14400000 // 4 hours in milliseconds
        running: true
        repeat: true
        triggeredOnStart: false
        onTriggered: {
            console.log("[kiosque] Auto check: periodic check for updates");
            Qt.callLater(installedModel.checkForUpdates);
        }
    }

    Connections {
        target: installedModel
        ignoreUnknownSignals: true

        function onLoadingChanged() {
            if (!installedModel.loading && !root.initialUpdateCheckDone) {
                root.initialUpdateCheckDone = true;
                console.log("[kiosque] Auto check: initial check for updates on startup");
                Qt.callLater(installedModel.checkForUpdates);
            }
        }
    }

    Image {
        id: packagedLogo
        visible: false
        source: AppShell.checkUi ? "qrc:/qml/images/logo.svg" : ""
        onStatusChanged: {
            if (!AppShell.checkUi || status === Image.Loading || status === Image.Null) return;
            Qt.callLater(function() { AppShell.uiReady(packagedLogo.status === Image.Ready); Qt.quit(); });
        }
    }
    // ── Startup ─────────────────────────────────────────────────────────
    Component.onCompleted: {
        Qt.application.name = "Kiosque";
        Qt.application.displayName = "Kiosque";
        Qt.application.organization = "Kiosque";
        Qt.application.domain = "kiosque.app";
        Qt.application.version = AppShell.version;
        if (AppShell.checkUi) return;
        SettingsController.loadSettings();
        featuredModel.refresh();
        appListModel.refresh();
        installedModel.refresh();
        // Defer until the page stack has created its initial page.
        Qt.callLater(root.openLaunchTarget);
    }
}
