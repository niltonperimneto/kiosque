// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque
import "../components" as Components

Kirigami.ScrollablePage {
    id: page
    property string appId
    property bool odrsLoading: false
    readonly property bool isNarrow: width < Kirigami.Units.gridUnit * 32

    title: StoreController.detail_name !== "" ? StoreController.detail_name : appId

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]

    Component.onCompleted: {
        if (appId !== "") {
            odrsLoading = true;
            StoreController.loadAppDetails(appId)
        }
    }

    Connections {
        target: StoreController
        function onDetailsLoaded() {
            populateModel(developerAppsModel, StoreController.detail_developerAppsJson);
            populateModel(similarAppsModel, StoreController.detail_similar_apps_json);
        }

        function onReviewsLoaded() {
            populateReviewsModel(reviewsModel, StoreController.detail_reviews_json);
            try {
                ratingData = JSON.parse(StoreController.detail_ratings_json);
            } catch(e) {
                ratingData = {};
            }
            page.odrsLoading = false;
        }

        function onReviewSubmitted(success, errorMsg) {
            if (success) {
                page.odrsLoading = true;
                StoreController.loadAppDetails(page.appId)
            } else {
                console.error("Review submission failed:", errorMsg)
            }
        }

        function onReviewActionFinished(success, errorMsg) {
            if (success) {
                page.odrsLoading = true;
                StoreController.loadAppDetails(page.appId)
            } else {
                console.error("Review action failed:", errorMsg)
            }
        }
    }

    // ── Local models for related apps ──────────────────────────────────
    ListModel {
        id: developerAppsModel
    }

    ListModel {
        id: similarAppsModel
    }

    ListModel {
        id: reviewsModel
    }

    property var ratingData: ({})
    property var allReviewsList: []

    // Helper to populate ListModel from JSON string
    function populateModel(listModel, jsonStr) {
        listModel.clear();
        if (!jsonStr || jsonStr === "" || jsonStr === "[]") return;
        try {
            let list = JSON.parse(jsonStr);
            for (let i = 0; i < list.length; i++) {
                let item = list[i];
                listModel.append({
                    appId: item.app_id || "",
                    name: item.name || "",
                    summary: item.summary || "",
                    iconUrl: item.icon || ""
                });
            }
        } catch (e) {
            console.error("Error parsing related apps:", e);
        }
    }

    // Helper to populate reviews model
    function populateReviewsModel(listModel, jsonStr) {
        listModel.clear();
        page.allReviewsList = [];
        if (!jsonStr || jsonStr === "" || jsonStr === "[]") return;
        try {
            page.allReviewsList = JSON.parse(jsonStr);
            loadMoreReviews();
        } catch (e) {
            console.error("Error parsing reviews:", e);
        }
    }

    // Load next batch of reviews
    function loadMoreReviews() {
        let currentCount = reviewsModel.count;
        let totalCount = page.allReviewsList.length;
        let batchSize = 10;
        let limit = Math.min(totalCount, currentCount + batchSize);
        for (let i = currentCount; i < limit; i++) {
            let item = page.allReviewsList[i];
            let dateObj = new Date(item.date_created * 1000); // ODRS date is unix timestamp in seconds
            reviewsModel.append({
                userName: item.user_display || i18n("Anonymous"),
                dateStr: dateObj.toLocaleDateString(Qt.locale(), Locale.ShortFormat),
                rating: item.rating || 0,
                summary: item.summary || "",
                description: item.description || "",
                version: item.version || "",
                reviewId: item.review_id || 0,
                userHash: item.user_hash || "",
                karmaUp: item.karma_up || 0,
                karmaDown: item.karma_down || 0
            });
        }
    }

    // Helper to get structured list of permissions for the HIG display
    function getPermissionItems(jsonStr) {
        if (!jsonStr || jsonStr === "{}" || jsonStr === "[]") {
            return [{
                icon: "security-high-symbolic",
                title: i18n("No special permissions required (Fully Sandboxed)"),
                description: i18n("This application is fully sandboxed and isolated from your files, network, and devices.")
            }];
        }
        try {
            let perms = JSON.parse(jsonStr);
            let items = [];
            
            let shared = perms.shared || [];
            let sockets = perms.sockets || [];
            let devices = perms.devices || [];
            let filesystems = perms.filesystems || [];
            let persistent = perms.persistent || [];
            let sessionBus = perms["session-bus"] || {};
            let systemBus = perms["system-bus"] || {};
            
            // 1. Network
            if (shared.indexOf("network") !== -1 || sockets.indexOf("network") !== -1) {
                items.push({
                    icon: "network-wired-symbolic",
                    title: i18n("Network Access"),
                    description: i18n("Can access the internet or local network")
                });
            }
            
            // 2. Display System
            let hasWayland = sockets.indexOf("wayland") !== -1;
            let hasX11 = (sockets.indexOf("x11") !== -1) || (sockets.indexOf("fallback-x11") !== -1);
            if (hasWayland || hasX11) {
                let sys = [];
                if (hasWayland) sys.push("Wayland");
                if (hasX11) sys.push("X11");
                items.push({
                    icon: "preferences-desktop-display-symbolic",
                    title: i18n("Display System"),
                    description: i18n("Can show windows and access the screen (%1)").arg(sys.join(" / "))
                });
            }
            
            // 3. Audio Access
            if (sockets.indexOf("pulseaudio") !== -1 || shared.indexOf("pulseaudio") !== -1) {
                items.push({
                    icon: "audio-card-symbolic",
                    title: i18n("Audio Access"),
                    description: i18n("Can play audio or record sound using the sound card")
                });
            }
            
            // 4. Inter-process Communication (IPC)
            if (shared.indexOf("ipc") !== -1) {
                items.push({
                    icon: "preferences-system-network-symbolic",
                    title: i18n("Inter-process Communication"),
                    description: i18n("Can communicate directly with other running processes")
                });
            }
            
            // 5. Hardware Devices
            if (devices.length > 0) {
                items.push({
                    icon: "device-notifier-symbolic",
                    title: i18n("Hardware Access"),
                    description: i18n("Direct access to hardware devices: %1").arg(devices.join(", "))
                });
            }
            
            // 6. File System Access
            if (filesystems.length > 0) {
                let fsDetails = [];
                filesystems.forEach(function(fs) {
                    let parts = fs.split(":");
                    let path = parts[0];
                    let mode = parts[1] || "";
                    let modeText = mode === "ro" ? i18n("read-only") : (mode === "create" ? i18n("read/write/create") : i18n("read/write"));
                    
                    let pathName = path;
                    if (path === "host") {
                        pathName = i18n("All system files");
                    } else if (path === "home") {
                        pathName = i18n("Home directory");
                    } else if (path === "xdg-desktop") {
                        pathName = i18n("Desktop folder");
                    } else if (path === "xdg-documents") {
                        pathName = i18n("Documents folder");
                    } else if (path === "xdg-download") {
                        pathName = i18n("Downloads folder");
                    } else if (path === "xdg-music") {
                        pathName = i18n("Music folder");
                    } else if (path === "xdg-pictures") {
                        pathName = i18n("Pictures folder");
                    } else if (path === "xdg-public-share") {
                        pathName = i18n("Public share folder");
                    } else if (path === "xdg-templates") {
                        pathName = i18n("Templates folder");
                    } else if (path === "xdg-videos") {
                        pathName = i18n("Videos folder");
                    } else if (path === "xdg-run/keyring") {
                        pathName = i18n("System keyring");
                    } else if (path.startsWith("xdg-")) {
                        let folder = path.substring(4);
                        pathName = folder.charAt(0).toUpperCase() + folder.slice(1) + " " + i18n("folder");
                    }
                    
                    fsDetails.push(pathName + " (" + modeText + ")");
                });
                
                items.push({
                    icon: "folder-symbolic",
                    title: i18n("File System Access"),
                    description: i18n("Access to files: %1").arg(fsDetails.join(", "))
                });
            }
            
            // 7. Persistent Storage
            if (persistent.length > 0) {
                items.push({
                    icon: "drive-harddisk-symbolic",
                    title: i18n("Persistent Storage"),
                    description: i18n("Saves persistent data in: %1").arg(persistent.join(", "))
                });
            }
            
            // 8. D-Bus System Integration
            let sBus = perms["session-bus"] || {};
            let sysBus = perms["system-bus"] || {};
            let hasSession = (sBus.talk && sBus.talk.length > 0) || (sBus.own && sBus.own.length > 0);
            let hasSystem = (sysBus.talk && sysBus.talk.length > 0) || (sysBus.own && sysBus.own.length > 0);
            if (hasSession || hasSystem) {
                let busDetails = [];
                if (sBus.talk && sBus.talk.length > 0) {
                    busDetails.push(i18n("Talks to session services: %1").arg(sBus.talk.join(", ")));
                }
                if (sBus.own && sBus.own.length > 0) {
                    busDetails.push(i18n("Owns session services: %1").arg(sBus.own.join(", ")));
                }
                if (sysBus.talk && sysBus.talk.length > 0) {
                    busDetails.push(i18n("Talks to system services: %1").arg(sysBus.talk.join(", ")));
                }
                if (sysBus.own && sysBus.own.length > 0) {
                    busDetails.push(i18n("Owns system services: %1").arg(sysBus.own.join(", ")));
                }
                
                items.push({
                    icon: "applications-system-symbolic",
                    title: i18n("System Integration"),
                    description: i18n("Access to D-Bus services: %1").arg(busDetails.join("; "))
                });
            }
            
            // 9. SSH / GPG Agent
            if (sockets.indexOf("ssh-auth") !== -1 || sockets.indexOf("gpg-agent") !== -1) {
                let agents = [];
                if (sockets.indexOf("ssh-auth") !== -1) agents.push("SSH");
                if (sockets.indexOf("gpg-agent") !== -1) agents.push("GPG");
                items.push({
                    icon: "security-high-symbolic",
                    title: i18n("Security Agent Access"),
                    description: i18n("Access to authentication agent sockets: %1").arg(agents.join(", "))
                });
            }
            
            // 10. Other Sockets
            let knownSockets = ["x11", "wayland", "fallback-x11", "pulseaudio", "ssh-auth", "gpg-agent", "network"];
            let otherSockets = sockets.filter(function(s) { return knownSockets.indexOf(s) === -1; });
            if (otherSockets.length > 0) {
                items.push({
                    icon: "preferences-system-network-symbolic",
                    title: i18n("Other System Connections"),
                    description: i18n("Accesses system sockets: %1").arg(otherSockets.join(", "))
                });
            }
            
            if (items.length === 0) {
                return [{
                    icon: "security-high-symbolic",
                    title: i18n("No special permissions required (Fully Sandboxed)"),
                    description: i18n("This application is fully sandboxed and isolated from your files, network, and devices.")
                }];
            }
            return items;
        } catch(e) {
            return [{
                icon: "security-medium-symbolic",
                title: i18n("Fully Sandboxed or permissions unavailable"),
                description: i18n("The application is either fully sandboxed or its permissions could not be loaded.")
            }];
        }
    }

    // Helper to identify wayland/x11/cli support from permissions sockets
    function getWindowingSystem(jsonStr) {
        if (!jsonStr || jsonStr === "{}" || jsonStr === "[]") return "";
        try {
            let perms = JSON.parse(jsonStr);
            let sockets = perms.sockets || [];
            
            let hasWayland = sockets.indexOf("wayland") !== -1;
            let hasX11 = (sockets.indexOf("x11") !== -1) || (sockets.indexOf("fallback-x11") !== -1);
            
            if (hasWayland && hasX11) {
                return "Wayland / X11";
            } else if (hasWayland) {
                return "Wayland";
            } else if (hasX11) {
                return "X11";
            } else {
                return "CLI";
            }
        } catch(e) {
            return "";
        }
    }

    // Helper to check if any URLs are available
    function hasLinks() {
        if (!StoreController.detail_urls_json || StoreController.detail_urls_json === "{}") return false;
        try {
            let urls = JSON.parse(StoreController.detail_urls_json);
            return !!(urls.homepage || urls.vcs_browser || urls.manifest || urls.bugtracker || urls.help || urls.donation);
        } catch(e) {
            return false;
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing * 1.5
        width: parent.width

        // ── Loading indicator ───────────────────────────────────────────
        Controls.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: StoreController.loading
            visible: StoreController.loading
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 4
            Layout.topMargin: Kirigami.Units.gridUnit * 5
        }



        // ── App header ──────────────────────────────────────────────────
        GridLayout {
            columns: page.isNarrow ? 1 : 2
            columnSpacing: Kirigami.Units.largeSpacing * 1.5
            rowSpacing: Kirigami.Units.largeSpacing * 1.5
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading

            // App Icon with Rounded Glow Card
            Kirigami.ShadowedRectangle {
                Layout.alignment: page.isNarrow ? Qt.AlignHCenter : Qt.AlignVCenter
                Layout.preferredWidth: 128
                Layout.preferredHeight: 128
                radius: 20
                color: Kirigami.Theme.alternateBackgroundColor
                shadow.size: iconHover.hovered ? 12 : 6
                shadow.color: iconHover.hovered
                    ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.18)
                    : Qt.rgba(0, 0, 0, 0.08)
                border.width: 1
                border.color: iconHover.hovered ? Kirigami.Theme.highlightColor : Qt.rgba(0, 0, 0, 0.1)
                scale: iconHover.hovered ? 1.02 : 1.0

                Behavior on scale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                Behavior on shadow.size { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

                Image {
                    id: appIconImg
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing
                    source: StoreController.detail_icon_url
                    asynchronous: true
                    fillMode: Image.PreserveAspectFit

                    // Placeholder while icon loads
                    Kirigami.Icon {
                        anchors.centerIn: parent
                        width: 64
                        height: 64
                        source: "application-x-executable"
                        visible: parent.status !== Image.Ready
                        opacity: 0.4
                    }
                }

                HoverHandler {
                    id: iconHover
                }
            }

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.alignment: page.isNarrow ? Qt.AlignHCenter : Qt.AlignVCenter

                GridLayout {
                    columns: page.isNarrow ? 1 : 2
                    columnSpacing: Kirigami.Units.mediumSpacing
                    rowSpacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true

                    Kirigami.Heading {
                        level: 1
                        text: StoreController.detail_name
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        horizontalAlignment: page.isNarrow ? Text.AlignHCenter : Text.AlignLeft
                    }

                    RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.alignment: page.isNarrow ? Qt.AlignHCenter : (Qt.AlignVCenter | Qt.AlignRight)

                        // License Badge
                        Rectangle {
                            id: licenseBadge
                            visible: StoreController.detail_license !== ""
                            radius: 6
                            color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.12)
                            border.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.3)
                            border.width: 1
                            implicitWidth: licenseLabel.implicitWidth + Kirigami.Units.mediumSpacing
                            implicitHeight: licenseLabel.implicitHeight + Kirigami.Units.smallSpacing

                            Controls.Label {
                                id: licenseLabel
                                anchors.centerIn: parent
                                text: StoreController.detail_license
                                font.bold: true
                                font.pointSize: Kirigami.Theme.smallFont.pointSize * 0.9
                                color: Kirigami.Theme.highlightColor
                            }
                        }

                        // Windowing System Badge (Wayland/X11/CLI)
                        Rectangle {
                            id: envBadge
                            property string envText: getWindowingSystem(StoreController.detail_permissions_json)
                            visible: envText !== ""
                            radius: 6
                            color: {
                                let baseColor = Kirigami.Theme.disabledTextColor;
                                if (envText.indexOf("Wayland") !== -1) {
                                    baseColor = Kirigami.Theme.positiveTextColor;
                                } else if (envText === "X11") {
                                    baseColor = Kirigami.Theme.neutralTextColor;
                                }
                                return Qt.rgba(baseColor.r, baseColor.g, baseColor.b, 0.12);
                            }
                            border.color: {
                                let baseColor = Kirigami.Theme.disabledTextColor;
                                if (envText.indexOf("Wayland") !== -1) {
                                    baseColor = Kirigami.Theme.positiveTextColor;
                                } else if (envText === "X11") {
                                    baseColor = Kirigami.Theme.neutralTextColor;
                                }
                                return Qt.rgba(baseColor.r, baseColor.g, baseColor.b, 0.3);
                            }
                            border.width: 1
                            implicitWidth: envLabel.implicitWidth + Kirigami.Units.mediumSpacing
                            implicitHeight: envLabel.implicitHeight + Kirigami.Units.smallSpacing

                            Controls.Label {
                                id: envLabel
                                anchors.centerIn: parent
                                text: parent.envText
                                font.bold: true
                                font.pointSize: Kirigami.Theme.smallFont.pointSize * 0.9
                                color: {
                                    if (parent.envText.indexOf("Wayland") !== -1) {
                                        return Kirigami.Theme.positiveTextColor;
                                    } else if (parent.envText === "X11") {
                                        return Kirigami.Theme.neutralTextColor;
                                    } else {
                                        return Kirigami.Theme.disabledTextColor;
                                    }
                                }
                            }
                        }
                    }
                }

                Controls.Label {
                    text: StoreController.detail_developer !== "" ? StoreController.detail_developer : i18n("Unknown Developer")
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    font.bold: true
                    horizontalAlignment: page.isNarrow ? Text.AlignHCenter : Text.AlignLeft
                }

                Controls.Label {
                    text: StoreController.detail_summary
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    opacity: 0.85
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.05
                    horizontalAlignment: page.isNarrow ? Text.AlignHCenter : Text.AlignLeft
                }
            }
        }

        // ── Action buttons & Permissions ─────────────────────────────────
        GridLayout {
            columns: page.isNarrow ? 1 : 2
            rowSpacing: Kirigami.Units.mediumSpacing
            columnSpacing: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading

            StackLayout {
                currentIndex: StoreController.install_progress > 0.0 && StoreController.install_progress < 1.0 ? 1 : 0
                Layout.fillWidth: page.isNarrow
                Layout.preferredWidth: page.isNarrow ? -1 : 260
                Layout.preferredHeight: 40

                // 0: Normal Button
                Controls.Button {
                    text: {
                        if (StoreController.loading) return i18n("Loading…");
                        return StoreController.detail_is_installed ? i18n("Remove") : i18n("Install");
                    }
                    icon.name: StoreController.detail_is_installed ? "edit-delete-symbolic" : "download-symbolic"
                    enabled: !StoreController.loading
                    highlighted: !StoreController.detail_is_installed
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    onClicked: {
                        if (StoreController.detail_is_installed) {
                            StoreController.uninstallApp(page.appId)
                        } else {
                            StoreController.installApp(page.appId)
                        }
                    }
                }

                // 1: Subtle Progress Bar with Cancel
                RowLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: Kirigami.Units.mediumSpacing

                    Controls.ProgressBar {
                        id: subtleProgress
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignVCenter
                        from: 0.0
                        to: 1.0
                        value: StoreController.install_progress
                        indeterminate: StoreController.install_progress < 0.02

                        Behavior on value { NumberAnimation { duration: 300 } }
                    }

                    Controls.ToolButton {
                        icon.name: "dialog-cancel"
                        display: Controls.AbstractButton.IconOnly
                        text: i18n("Cancel")
                        Controls.ToolTip.text: text
                        Controls.ToolTip.visible: hovered
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: {
                            StoreController.cancelOperation()
                        }
                    }
                }
            }

            // Permissions Button
            Controls.Button {
                id: permissionsBtn
                text: i18n("Sandbox Permissions")
                icon.name: "security-high-symbolic"
                Layout.fillWidth: page.isNarrow
                Layout.preferredHeight: 40
                onClicked: permissionsDialog.open()
            }
        }

        // ── Separator ───────────────────────────────────────────────────
        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading
        }

        // ── Screenshots Carousel ─────────────────────────────────────────
        Item {
            id: screenshotsContainer
            Layout.fillWidth: true
            Layout.preferredHeight: !StoreController.loading && screenshotsListView.count > 0 ? Kirigami.Units.gridUnit * 13 : 0
            visible: !StoreController.loading && screenshotsListView.count > 0

            ListView {
                id: screenshotsListView
                anchors.fill: parent
                orientation: ListView.Horizontal
                spacing: Kirigami.Units.largeSpacing
                clip: true

                leftMargin: Kirigami.Units.largeSpacing
                rightMargin: Kirigami.Units.largeSpacing

                model: {
                    if (!StoreController.detail_screenshots_json || StoreController.detail_screenshots_json === "") return [];
                    try {
                        return JSON.parse(StoreController.detail_screenshots_json);
                    } catch(e) {
                        return [];
                    }
                }

                Behavior on contentX {
                    enabled: !screenshotsListView.moving && !screenshotsListView.dragging
                    NumberAnimation { duration: 300; easing.type: Easing.OutCubic }
                }

                delegate: Item {
                    width: Kirigami.Units.gridUnit * 22
                    height: screenshotsListView.height - Kirigami.Units.largeSpacing

                    Kirigami.ShadowedRectangle {
                        id: screenshotCard
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        radius: 12
                        color: Kirigami.Theme.alternateBackgroundColor

                        // Glowing soft shadow using highlight color on hover
                        shadow.size: screenshotMouseArea.containsMouse ? 18 : 8
                        shadow.color: screenshotMouseArea.containsMouse
                            ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.22)
                            : Qt.rgba(0, 0, 0, 0.12)
                        shadow.yOffset: screenshotMouseArea.containsMouse ? 4 : 2

                        border.width: 1
                        border.color: screenshotMouseArea.containsMouse
                            ? Kirigami.Theme.highlightColor
                            : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

                        scale: screenshotMouseArea.containsMouse ? 1.015 : 1.0

                        Behavior on scale { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                        Behavior on shadow.size { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                        Behavior on shadow.yOffset { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                        Behavior on border.color { ColorAnimation { duration: 150 } }

                        Image {
                            id: screenshotImg
                            anchors.fill: parent
                            anchors.margins: 1
                            source: modelData
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            clip: true

                            Controls.BusyIndicator {
                                anchors.centerIn: parent
                                running: parent.status === Image.Loading
                                visible: parent.status === Image.Loading
                            }
                        }

                        // Overlay to enforce rounded corners visual on images
                        Rectangle {
                            anchors.fill: parent
                            radius: parent.radius
                            color: "transparent"
                            border.width: screenshotCard.border.width
                            border.color: screenshotCard.border.color
                            z: 1
                        }

                        MouseArea {
                            id: screenshotMouseArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                try {
                                    let list = JSON.parse(StoreController.detail_screenshots_json);
                                    let idx = list.indexOf(modelData);
                                    largeScreenshotPopup.openWithIndex(idx >= 0 ? idx : 0);
                                } catch(e) {
                                    largeScreenshotPopup.openWithIndex(0);
                                }
                            }
                        }
                    }
                }
            }

            // Left Navigation Arrow Button
            Item {
                id: leftScreenshotArrow
                anchors.left: parent.left
                anchors.leftMargin: Kirigami.Units.mediumSpacing
                anchors.verticalCenter: parent.verticalCenter
                width: 38
                height: 38
                z: 10
                visible: screenshotsListView.contentX > 10

                Rectangle {
                    anchors.fill: parent
                    radius: 19
                    color: leftScreenshotArrowMouse.containsMouse
                        ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25)
                        : Qt.rgba(Kirigami.Theme.backgroundColor.r, Kirigami.Theme.backgroundColor.g, Kirigami.Theme.backgroundColor.b, 0.7)
                    border.width: 1
                    border.color: leftScreenshotArrowMouse.containsMouse
                        ? Kirigami.Theme.highlightColor
                        : Qt.rgba(Kirigami.Theme.disabledTextColor.r, Kirigami.Theme.disabledTextColor.g, Kirigami.Theme.disabledTextColor.b, 0.3)

                    Behavior on color { ColorAnimation { duration: 150 } }
                    Behavior on border.color { ColorAnimation { duration: 150 } }

                    Kirigami.Icon {
                        anchors.centerIn: parent
                        width: 20
                        height: 20
                        source: "go-previous-symbolic"
                        color: Kirigami.Theme.textColor
                    }
                }

                MouseArea {
                    id: leftScreenshotArrowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        let step = Kirigami.Units.gridUnit * 24;
                        screenshotsListView.contentX = Math.max(screenshotsListView.contentX - step, 0);
                    }
                }

                scale: leftScreenshotArrowMouse.containsMouse ? 1.05 : 1.0
                Behavior on scale { NumberAnimation { duration: 150 } }
            }

            // Right Navigation Arrow Button
            Item {
                id: rightScreenshotArrow
                anchors.right: parent.right
                anchors.rightMargin: Kirigami.Units.mediumSpacing
                anchors.verticalCenter: parent.verticalCenter
                width: 38
                height: 38
                z: 10
                visible: screenshotsListView.contentX < (screenshotsListView.contentWidth - screenshotsListView.width - 10)

                Rectangle {
                    anchors.fill: parent
                    radius: 19
                    color: rightScreenshotArrowMouse.containsMouse
                        ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25)
                        : Qt.rgba(Kirigami.Theme.backgroundColor.r, Kirigami.Theme.backgroundColor.g, Kirigami.Theme.backgroundColor.b, 0.7)
                    border.width: 1
                    border.color: rightScreenshotArrowMouse.containsMouse
                        ? Kirigami.Theme.highlightColor
                        : Qt.rgba(Kirigami.Theme.disabledTextColor.r, Kirigami.Theme.disabledTextColor.g, Kirigami.Theme.disabledTextColor.b, 0.3)

                    Behavior on color { ColorAnimation { duration: 150 } }
                    Behavior on border.color { ColorAnimation { duration: 150 } }

                    Kirigami.Icon {
                        anchors.centerIn: parent
                        width: 20
                        height: 20
                        source: "go-next-symbolic"
                        color: Kirigami.Theme.textColor
                    }
                }

                MouseArea {
                    id: rightScreenshotArrowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        let step = Kirigami.Units.gridUnit * 24;
                        let maxContentX = screenshotsListView.contentWidth - screenshotsListView.width;
                        screenshotsListView.contentX = Math.min(screenshotsListView.contentX + step, maxContentX);
                    }
                }

                scale: rightScreenshotArrowMouse.containsMouse ? 1.05 : 1.0
                Behavior on scale { NumberAnimation { duration: 150 } }
            }
        }

        // ── Description ─────────────────────────────────────────────────
        Kirigami.ShadowedRectangle {
            radius: 12
            color: Kirigami.Theme.alternateBackgroundColor
            border.width: 1
            border.color: Qt.rgba(0, 0, 0, 0.05)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading && StoreController.detail_description !== ""
            implicitHeight: descText.implicitHeight + Kirigami.Units.largeSpacing * 2

            Controls.Label {
                id: descText
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                text: StoreController.detail_description
                textFormat: Text.RichText
                wrapMode: Text.WordWrap
                lineHeight: 1.2
                onLinkActivated: (link) => Qt.openUrlExternally(link)
            }
        }

        // ── Links & Resources ───────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.mediumSpacing
            visible: !StoreController.loading && hasLinks()

            Kirigami.Heading {
                level: 3
                text: i18n("Links & Resources")
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }

            GridLayout {
                columns: page.width > Kirigami.Units.gridUnit * 32 ? 3 : (page.width > Kirigami.Units.gridUnit * 20 ? 2 : 1)
                rowSpacing: Kirigami.Units.mediumSpacing
                columnSpacing: Kirigami.Units.mediumSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing

                function getLink(key) {
                    if (!StoreController.detail_urls_json || StoreController.detail_urls_json === "{}") return "";
                    try {
                        let urls = JSON.parse(StoreController.detail_urls_json);
                        return urls[key] || "";
                    } catch(e) {
                        return "";
                    }
                }

                // Homepage Button
                Controls.Button {
                    text: i18n("Website")
                    icon.name: "internet-services"
                    visible: parent.getLink("homepage") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("homepage"))
                }

                // Source Repository Button
                Controls.Button {
                    text: i18n("Source Code")
                    icon.name: "code-context"
                    visible: parent.getLink("vcs_browser") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("vcs_browser"))
                }

                // Manifest Button
                Controls.Button {
                    text: i18n("Flatpak Manifest")
                    icon.name: "text-x-qml"
                    visible: parent.getLink("manifest") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("manifest"))
                }

                // Bug Tracker Button
                Controls.Button {
                    text: i18n("Report an Issue")
                    icon.name: "tools-report-bug"
                    visible: parent.getLink("bugtracker") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("bugtracker"))
                }

                // Help Button
                Controls.Button {
                    text: i18n("Documentation")
                    icon.name: "help-browser"
                    visible: parent.getLink("help") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("help"))
                }

                // Donation Button
                Controls.Button {
                    text: i18n("Donate / Support")
                    icon.name: "love"
                    visible: parent.getLink("donation") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("donation"))
                }
            }
        }

        // ── Developer's other apps ──────────────────────────────────────
        Components.HorizontalAppList {
            id: developerAppsList
            title: i18n("More by %1", StoreController.detail_developer !== "" ? StoreController.detail_developer : i18n("Developer"))
            appModel: developerAppsModel
            visible: !StoreController.loading && developerAppsModel.count > 0
            Layout.fillWidth: true
        }

        // ── Similar applications ────────────────────────────────────────
        Components.HorizontalAppList {
            id: similarAppsList
            title: i18n("Similar Applications")
            appModel: similarAppsModel
            visible: !StoreController.loading && similarAppsModel.count > 0
            Layout.fillWidth: true
        }

        // ── Separator ───────────────────────────────────────────────────
        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading
        }

        // ── Reviews & Ratings ───────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading

            RowLayout {
                Layout.fillWidth: true
                Kirigami.Heading {
                    level: 2
                    text: i18n("Ratings & Reviews")
                    Layout.fillWidth: true
                }

                Controls.Button {
                    text: i18n("Write a Review")
                    icon.name: "document-edit"
                    enabled: !page.odrsLoading
                    onClicked: reviewDialog.open()
                }
            }

            // ODRS Loading State
            Controls.BusyIndicator {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.largeSpacing
                running: page.odrsLoading
                visible: page.odrsLoading
            }

            // No Reviews Placeholder
            Kirigami.PlaceholderMessage {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.largeSpacing
                text: i18n("No Reviews Yet")
                explanation: i18n("Be the first to write a review for this application!")
                icon.name: "document-edit"
                visible: !page.odrsLoading && ratingData.total === 0 && reviewsModel.count === 0
            }

            // Rating Summary Card
            Components.RatingSummary {
                Layout.fillWidth: true
                visible: !page.odrsLoading && ratingData.total > 0
                star0: ratingData.star0 || 0
                star1: ratingData.star1 || 0
                star2: ratingData.star2 || 0
                star3: ratingData.star3 || 0
                star4: ratingData.star4 || 0
                star5: ratingData.star5 || 0
                total: ratingData.total || 0
            }

            // Reviews List
            Repeater {
                model: reviewsModel
                visible: !page.odrsLoading && reviewsModel.count > 0
                delegate: Components.ReviewCard {
                    Layout.fillWidth: true
                    userName: model.userName
                    dateStr: model.dateStr
                    rating: model.rating
                    summary: model.summary
                    description: model.description
                    version: model.version
                    reviewId: model.reviewId
                    userHash: model.userHash
                    karmaUp: model.karmaUp
                    karmaDown: model.karmaDown
                }
            }

            Controls.Button {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.mediumSpacing
                Layout.bottomMargin: Kirigami.Units.largeSpacing
                text: i18n("Show More Reviews")
                icon.name: "arrow-down"
                visible: !page.odrsLoading && reviewsModel.count < page.allReviewsList.length
                onClicked: loadMoreReviews()
            }
        }
    }

    // ── Write a Review Dialog ──────────────────────────────────
    Kirigami.Dialog {
        id: reviewDialog
        title: i18n("Write a Review")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        width: Math.min(parent.width * 0.95, Kirigami.Units.gridUnit * 38)
        height: Math.min(parent.height * 0.85, Kirigami.Units.gridUnit * 25)

        onAccepted: {
            if (summaryInput.text !== "" && descriptionInput.text !== "") {
                StoreController.submitReview(
                    page.appId,
                    ratingRow.selectedRating,
                    summaryInput.text,
                    descriptionInput.text,
                    "", // auto-detect version on backend
                    "", // auto-detect distro on backend
                    "", // auto-detect locale on backend
                    anonymousToggle.checked
                )
                summaryInput.text = ""
                descriptionInput.text = ""
                ratingRow.selectedRating = 5
            }
        }

        Kirigami.FormLayout {
            id: formLayout
            Layout.fillWidth: true
            Layout.fillHeight: true

            // Rating Stars
            RowLayout {
                id: ratingRow
                Kirigami.FormData.label: i18n("Rating:")
                spacing: Kirigami.Units.smallSpacing
                property int selectedRating: 5

                Repeater {
                    model: 5
                    Kirigami.Icon {
                        source: "rating"
                        width: Kirigami.Units.iconSizes.medium
                        height: width
                        color: index < ratingRow.selectedRating ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                        
                        MouseArea {
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: ratingRow.selectedRating = index + 1
                        }
                    }
                }
            }

            // Summary Field
            Controls.TextField {
                id: summaryInput
                Kirigami.FormData.label: i18n("Title:")
                placeholderText: i18n("One-line title of your review")
                Layout.fillWidth: true
            }

            // Description Scrollable Area
            Controls.ScrollView {
                id: scrollView
                Kirigami.FormData.label: i18n("Review:")
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 12
                
                Controls.TextArea {
                    id: descriptionInput
                    placeholderText: i18n("Detailed review description")
                    wrapMode: TextEdit.WordWrap
                    width: scrollView.width
                }
            }

            // Publish Anonymously Toggle
            Controls.Switch {
                id: anonymousToggle
                Kirigami.FormData.label: i18n("Anonymous:")
                checked: !SettingsController.is_authenticated
                enabled: SettingsController.is_authenticated
            }
        }
    }

    // ── Fullscreen Screenshot Zoom Popup ───────────────────────────────
    Controls.Popup {
        id: largeScreenshotPopup
        parent: Controls.Overlay.overlay
        x: 0
        y: 0
        margins: 0
        width: parent ? parent.width : root.width
        height: parent ? parent.height : root.height
        modal: true
        focus: true
        closePolicy: Controls.Popup.CloseOnEscape
        padding: 0

        enter: Transition {
            NumberAnimation { property: "opacity"; from: 0.0; to: 1.0; duration: 250; easing.type: Easing.OutQuad }
        }
        exit: Transition {
            NumberAnimation { property: "opacity"; from: 1.0; to: 0.0; duration: 200; easing.type: Easing.OutQuad }
        }

        background: Rectangle {
            color: "#f5000000" // Immersive dark backdrop
        }

        property var screenshotsList: []

        function openWithIndex(index) {
            try {
                screenshotsList = JSON.parse(StoreController.detail_screenshots_json || "[]");
            } catch(e) {
                screenshotsList = [];
            }
            screenshotSwipeView.currentIndex = index;
            open();
        }

        Item {
            anchors.fill: parent
            focus: true

            // Handle keyboard navigation inside the gallery
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Left) {
                    if (screenshotSwipeView.currentIndex > 0) {
                        screenshotSwipeView.currentIndex--;
                        event.accepted = true;
                    }
                } else if (event.key === Qt.Key_Right) {
                    if (screenshotSwipeView.currentIndex < screenshotSwipeView.count - 1) {
                        screenshotSwipeView.currentIndex++;
                        event.accepted = true;
                    }
                }
            }

            // Carousel Gallery SwipeView
            Controls.SwipeView {
                id: screenshotSwipeView
                anchors.fill: parent
                currentIndex: 0
                interactive: {
                    let currentItem = screenshotSwipeView.currentItem;
                    if (currentItem && typeof currentItem.findChildFlickable === "function") {
                        let flickable = currentItem.findChildFlickable();
                        if (flickable) return flickable.zoomScale <= 1.0;
                    }
                    return true;
                }

                Repeater {
                    model: largeScreenshotPopup.screenshotsList

                    delegate: Item {
                        id: delegateItem
                        width: screenshotSwipeView.width
                        height: screenshotSwipeView.height

                        // Helper to expose the flickable to SwipeView interactive property
                        function findChildFlickable() {
                            return imageFlickable;
                        }

                        Flickable {
                            id: imageFlickable
                            anchors.fill: parent
                            clip: true
                            contentWidth: zoomImage.width
                            contentHeight: zoomImage.height
                            boundsBehavior: Flickable.StopAtBounds

                            property real zoomScale: 1.0

                            onWidthChanged: centerImage()
                            onHeightChanged: centerImage()
                            onZoomScaleChanged: centerImage()

                            function centerImage() {
                                if (zoomScale <= 1.0) {
                                    contentX = 0;
                                    contentY = 0;
                                }
                            }

                            Image {
                                id: zoomImage
                                property real fitWidth: {
                                    if (implicitWidth <= 0 || implicitHeight <= 0) return imageFlickable.width;
                                    let imageRatio = implicitWidth / implicitHeight;
                                    let flickableRatio = imageFlickable.width / imageFlickable.height;
                                    if (flickableRatio > imageRatio) {
                                        return imageFlickable.height * imageRatio;
                                    } else {
                                        return imageFlickable.width;
                                    }
                                }
                                property real fitHeight: {
                                    if (implicitWidth <= 0 || implicitHeight <= 0) return imageFlickable.height;
                                    let imageRatio = implicitWidth / implicitHeight;
                                    let flickableRatio = imageFlickable.width / imageFlickable.height;
                                    if (flickableRatio > imageRatio) {
                                        return imageFlickable.height;
                                    } else {
                                        return imageFlickable.width / imageRatio;
                                    }
                                }

                                width: fitWidth * imageFlickable.zoomScale
                                height: fitHeight * imageFlickable.zoomScale
                                x: Math.max(0, (imageFlickable.width - width) / 2)
                                y: Math.max(0, (imageFlickable.height - height) / 2)
                                source: modelData
                                fillMode: Image.PreserveAspectFit
                                asynchronous: true

                                Behavior on width { NumberAnimation { duration: 150; easing.type: Easing.OutQuad } }
                                Behavior on height { NumberAnimation { duration: 150; easing.type: Easing.OutQuad } }
                                Behavior on x { NumberAnimation { duration: 150; easing.type: Easing.OutQuad } }
                                Behavior on y { NumberAnimation { duration: 150; easing.type: Easing.OutQuad } }

                                Controls.BusyIndicator {
                                    anchors.centerIn: parent
                                    running: parent.status === Image.Loading
                                    visible: parent.status === Image.Loading
                                }
                            }

                            // Double tap/click to zoom
                            TapHandler {
                                onDoubleTapped: {
                                    if (imageFlickable.zoomScale > 1.0) {
                                        imageFlickable.zoomScale = 1.0
                                        imageFlickable.contentX = 0
                                        imageFlickable.contentY = 0
                                    } else {
                                        imageFlickable.zoomScale = 2.0
                                    }
                                }
                            }

                            // Ctrl + Scroll Wheel Zoom
                            WheelHandler {
                                acceptedModifiers: Qt.ControlModifier
                                onWheel: (event) => {
                                    let factor = event.angleDelta.y > 0 ? 1.15 : 0.85
                                    let targetScale = imageFlickable.zoomScale * factor
                                    imageFlickable.zoomScale = Math.max(1.0, Math.min(targetScale, 4.0))
                                }
                            }
                        }
                    }
                }
            }

            // HUD Top Bar
            RowLayout {
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: Kirigami.Units.largeSpacing
                height: Kirigami.Units.gridUnit * 2
                z: 10

                Controls.Label {
                    text: (screenshotSwipeView.currentIndex + 1) + " / " + screenshotSwipeView.count
                    color: "white"
                    font.bold: true
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.1
                    Layout.alignment: Qt.AlignVCenter
                    Layout.leftMargin: Kirigami.Units.gridUnit
                }

                Item {
                    Layout.fillWidth: true
                }

                Controls.ToolButton {
                    icon.name: "window-close"
                    icon.color: "white"
                    display: Controls.AbstractButton.IconOnly
                    Layout.alignment: Qt.AlignVCenter

                    background: Rectangle {
                        color: parent.hovered ? Qt.rgba(255, 255, 255, 0.15) : "transparent"
                        radius: width / 2
                    }

                    onClicked: largeScreenshotPopup.close()
                }
            }

            // Left Navigation Button
            Controls.RoundButton {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                anchors.margins: Kirigami.Units.largeSpacing
                icon.name: "go-previous-symbolic"
                icon.color: "white"
                visible: screenshotSwipeView.currentIndex > 0
                z: 10

                background: Rectangle {
                    radius: width / 2
                    color: parent.hovered ? Qt.rgba(255, 255, 255, 0.25) : Qt.rgba(0, 0, 0, 0.4)
                    border.color: parent.hovered ? "white" : "transparent"
                    border.width: 1
                }

                onClicked: screenshotSwipeView.currentIndex--
            }

            // Right Navigation Button
            Controls.RoundButton {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.margins: Kirigami.Units.largeSpacing
                icon.name: "go-next-symbolic"
                icon.color: "white"
                visible: screenshotSwipeView.currentIndex < screenshotSwipeView.count - 1
                z: 10

                background: Rectangle {
                    radius: width / 2
                    color: parent.hovered ? Qt.rgba(255, 255, 255, 0.25) : Qt.rgba(0, 0, 0, 0.4)
                    border.color: parent.hovered ? "white" : "transparent"
                    border.width: 1
                }

                onClicked: screenshotSwipeView.currentIndex++
            }

            // Carousel Page Indicators at Bottom
            Controls.PageIndicator {
                anchors.bottom: parent.bottom
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.margins: Kirigami.Units.largeSpacing
                count: screenshotSwipeView.count
                currentIndex: screenshotSwipeView.currentIndex
                visible: count > 1
                z: 10
            }
        }
    }

    // ── Sandbox Permissions Dialog ─────────────────────────────────────
    Kirigami.PromptDialog {
        id: permissionsDialog
        title: i18n("Application Permissions")
        standardButtons: Kirigami.Dialog.Close
        width: Math.min(parent.width * 0.95, Kirigami.Units.gridUnit * 28)

        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing
            Layout.fillWidth: true

            Controls.Label {
                text: i18n("This application has permissions to access the following:")
                color: Kirigami.Theme.disabledTextColor
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                visible: {
                    let items = getPermissionItems(StoreController.detail_permissions_json);
                    if (items.length === 0) return false;
                    if (items.length === 1 && (items[0].icon === "security-high-symbolic" || items[0].icon === "security-medium-symbolic")) return false;
                    return true;
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.mediumSpacing
                spacing: Kirigami.Units.smallSpacing

                Repeater {
                    model: getPermissionItems(StoreController.detail_permissions_json)

                    delegate: ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.mediumSpacing

                            Kirigami.Icon {
                                source: modelData.icon
                                width: Kirigami.Units.iconSizes.small
                                height: width
                                Layout.alignment: Qt.AlignVCenter
                                color: Kirigami.Theme.textColor
                                isMask: true
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1

                                Controls.Label {
                                    text: modelData.title
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                }

                                Controls.Label {
                                    text: modelData.description
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    color: Kirigami.Theme.disabledTextColor
                                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                                }
                            }
                        }

                        Kirigami.Separator {
                            Layout.fillWidth: true
                            visible: index < getPermissionItems(StoreController.detail_permissions_json).length - 1
                            Layout.topMargin: Kirigami.Units.smallSpacing
                            Layout.bottomMargin: Kirigami.Units.smallSpacing
                        }
                    }
                }
            }
        }
    }
}
