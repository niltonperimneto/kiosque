// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque
import "../components" as Components

Kirigami.ScrollablePage {
    id: page
    property string appId
    readonly property bool isNarrow: width < Kirigami.Units.gridUnit * 32

    title: details.name !== "" ? details.name : appId

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: L10n.i18n("Close", [])
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]

    // This page's own state; nothing is shared with other detail pages.
    AppDetails {
        id: details
        appId: page.appId
        onReviewsLoaded: Qt.callLater(page.resetReviews)
        onReviewSubmitted: (success, error) => {
            if (!success)
                console.warn("Review submission failed:", error)
        }
        onReviewActionFinished: (success, error) => {
            if (!success)
                console.warn("Review action failed:", error)
        }
    }
    Component.onDestruction: details.dispose()

    readonly property bool odrsLoading: details.loading || details.reviewsLoading
    readonly property var ratingData: details.ratings
    readonly property var allReviewsList: details.reviews
    readonly property var screenshotUrls: details.screenshots
    readonly property var urlsData: details.links
    readonly property var permissionItems: getPermissionItems(details.permissions)
    readonly property string windowingSystem: getWindowingSystem(details.permissions)

    ListModel {
        id: reviewsModel
    }

    function resetReviews() {
        reviewsModel.clear();
        loadMoreReviews();
    }

    // Load next batch of reviews
    function loadMoreReviews() {
        let currentCount = reviewsModel.count;
        let totalCount = page.allReviewsList.length;
        let batchSize = 10;
        let limit = Math.min(totalCount, currentCount + batchSize);
        for (let i = currentCount; i < limit; i++) {
            let item = page.allReviewsList[i];
            let dateObj = new Date(item.date * 1000); // Unix seconds
            reviewsModel.append({
                userName: item.user_name || L10n.i18n("Anonymous", []),
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
    function getPermissionItems(perms) {
        if (!perms || Object.keys(perms).length === 0) {
            return [{
                icon: "security-high-symbolic",
                title: L10n.i18n("No special permissions required (Fully Sandboxed)", []),
                description: L10n.i18n("This application is fully sandboxed and isolated from your files, network, and devices.", [])
            }];
        }
        try {
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
                    title: L10n.i18n("Network Access", []),
                    description: L10n.i18n("Can access the internet or local network", [])
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
                    title: L10n.i18n("Display System", []),
                    description: L10n.i18n("Can show windows and access the screen (%1)", []).arg(sys.join(" / "))
                });
            }
            
            // 3. Audio Access
            if (sockets.indexOf("pulseaudio") !== -1 || shared.indexOf("pulseaudio") !== -1) {
                items.push({
                    icon: "audio-card-symbolic",
                    title: L10n.i18n("Audio Access", []),
                    description: L10n.i18n("Can play audio or record sound using the sound card", [])
                });
            }
            
            // 4. Inter-process Communication (IPC)
            if (shared.indexOf("ipc") !== -1) {
                items.push({
                    icon: "preferences-system-network-symbolic",
                    title: L10n.i18n("Inter-process Communication", []),
                    description: L10n.i18n("Can communicate directly with other running processes", [])
                });
            }
            
            // 5. Hardware Devices
            if (devices.length > 0) {
                items.push({
                    icon: "device-notifier-symbolic",
                    title: L10n.i18n("Hardware Access", []),
                    description: L10n.i18n("Direct access to hardware devices: %1", []).arg(devices.join(", "))
                });
            }
            
            // 6. File System Access
            if (filesystems.length > 0) {
                let fsDetails = [];
                filesystems.forEach(function(fs) {
                    let parts = fs.split(":");
                    let path = parts[0];
                    let mode = parts[1] || "";
                    let modeText = mode === "ro" ? L10n.i18n("read-only", []) : (mode === "create" ? L10n.i18n("read/write/create", []) : L10n.i18n("read/write", []));
                    
                    let pathName = path;
                    if (path === "host") {
                        pathName = L10n.i18n("All system files", []);
                    } else if (path === "home") {
                        pathName = L10n.i18n("Home directory", []);
                    } else if (path === "xdg-desktop") {
                        pathName = L10n.i18n("Desktop folder", []);
                    } else if (path === "xdg-documents") {
                        pathName = L10n.i18n("Documents folder", []);
                    } else if (path === "xdg-download") {
                        pathName = L10n.i18n("Downloads folder", []);
                    } else if (path === "xdg-music") {
                        pathName = L10n.i18n("Music folder", []);
                    } else if (path === "xdg-pictures") {
                        pathName = L10n.i18n("Pictures folder", []);
                    } else if (path === "xdg-public-share") {
                        pathName = L10n.i18n("Public share folder", []);
                    } else if (path === "xdg-templates") {
                        pathName = L10n.i18n("Templates folder", []);
                    } else if (path === "xdg-videos") {
                        pathName = L10n.i18n("Videos folder", []);
                    } else if (path === "xdg-run/keyring") {
                        pathName = L10n.i18n("System keyring", []);
                    } else if (path.startsWith("xdg-")) {
                        let folder = path.substring(4);
                        pathName = folder.charAt(0).toUpperCase() + folder.slice(1) + " " + L10n.i18n("folder", []);
                    }
                    
                    fsDetails.push(pathName + " (" + modeText + ")");
                });
                
                items.push({
                    icon: "folder-symbolic",
                    title: L10n.i18n("File System Access", []),
                    description: L10n.i18n("Access to files: %1", []).arg(fsDetails.join(", "))
                });
            }
            
            // 7. Persistent Storage
            if (persistent.length > 0) {
                items.push({
                    icon: "drive-harddisk-symbolic",
                    title: L10n.i18n("Persistent Storage", []),
                    description: L10n.i18n("Saves persistent data in: %1", []).arg(persistent.join(", "))
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
                    busDetails.push(L10n.i18n("Talks to session services: %1", []).arg(sBus.talk.join(", ")));
                }
                if (sBus.own && sBus.own.length > 0) {
                    busDetails.push(L10n.i18n("Owns session services: %1", []).arg(sBus.own.join(", ")));
                }
                if (sysBus.talk && sysBus.talk.length > 0) {
                    busDetails.push(L10n.i18n("Talks to system services: %1", []).arg(sysBus.talk.join(", ")));
                }
                if (sysBus.own && sysBus.own.length > 0) {
                    busDetails.push(L10n.i18n("Owns system services: %1", []).arg(sysBus.own.join(", ")));
                }
                
                items.push({
                    icon: "applications-system-symbolic",
                    title: L10n.i18n("System Integration", []),
                    description: L10n.i18n("Access to D-Bus services: %1", []).arg(busDetails.join("; "))
                });
            }
            
            // 9. SSH / GPG Agent
            if (sockets.indexOf("ssh-auth") !== -1 || sockets.indexOf("gpg-agent") !== -1) {
                let agents = [];
                if (sockets.indexOf("ssh-auth") !== -1) agents.push("SSH");
                if (sockets.indexOf("gpg-agent") !== -1) agents.push("GPG");
                items.push({
                    icon: "security-high-symbolic",
                    title: L10n.i18n("Security Agent Access", []),
                    description: L10n.i18n("Access to authentication agent sockets: %1", []).arg(agents.join(", "))
                });
            }
            
            // 10. Other Sockets
            let knownSockets = ["x11", "wayland", "fallback-x11", "pulseaudio", "ssh-auth", "gpg-agent", "network"];
            let otherSockets = sockets.filter(function(s) { return knownSockets.indexOf(s) === -1; });
            if (otherSockets.length > 0) {
                items.push({
                    icon: "preferences-system-network-symbolic",
                    title: L10n.i18n("Other System Connections", []),
                    description: L10n.i18n("Accesses system sockets: %1", []).arg(otherSockets.join(", "))
                });
            }
            
            if (items.length === 0) {
                return [{
                    icon: "security-high-symbolic",
                    title: L10n.i18n("No special permissions required (Fully Sandboxed)", []),
                    description: L10n.i18n("This application is fully sandboxed and isolated from your files, network, and devices.", [])
                }];
            }
            return items;
        } catch(e) {
            return [{
                icon: "security-medium-symbolic",
                title: L10n.i18n("Fully Sandboxed or permissions unavailable", []),
                description: L10n.i18n("The application is either fully sandboxed or its permissions could not be loaded.", [])
            }];
        }
    }

    // Helper to identify wayland/x11/cli support from permissions sockets
    function getWindowingSystem(perms) {
        if (!perms || Object.keys(perms).length === 0) return "";
        try {
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
        let urls = page.urlsData;
        return !!(urls.homepage || urls.vcs_browser || urls.manifest || urls.bugtracker || urls.help || urls.donation);
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing * 1.5
        width: parent.width

        // ── Loading indicator ───────────────────────────────────────────
        Controls.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: details.loading
            visible: details.loading
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
            visible: !details.loading

            // App Icon
            Image {
                id: appIconImg
                Layout.alignment: page.isNarrow ? Qt.AlignHCenter : Qt.AlignVCenter
                Layout.preferredWidth: Kirigami.Units.iconSizes.enormous
                Layout.preferredHeight: Kirigami.Units.iconSizes.enormous
                source: appIconRemote.source
                RemoteImage { id: appIconRemote; url: details.iconUrl; kind: "icon:256" }
                asynchronous: true
                fillMode: Image.PreserveAspectFit

                // Placeholder while icon loads or if it fails
                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: Kirigami.Units.iconSizes.huge
                    height: width
                    source: "application-x-executable"
                    visible: appIconImg.status !== Image.Ready
                    opacity: 0.4
                }
            }

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.alignment: page.isNarrow ? Qt.AlignHCenter : Qt.AlignVCenter

                Kirigami.Heading {
                    level: 1
                    text: details.name
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    horizontalAlignment: page.isNarrow ? Text.AlignHCenter : Text.AlignLeft
                }

                Controls.Label {
                    text: details.developer !== "" ? details.developer : L10n.i18n("Unknown Developer", [])
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    horizontalAlignment: page.isNarrow ? Text.AlignHCenter : Text.AlignLeft
                }

                Controls.Label {
                    text: details.summary
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    horizontalAlignment: page.isNarrow ? Text.AlignHCenter : Text.AlignLeft
                }

                // Metadata tags: license & windowing system
                Flow {
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.Chip {
                        visible: details.license !== ""
                        text: details.license
                        icon.name: "license"
                        checkable: false
                        closable: false
                    }

                    Kirigami.Chip {
                        property string envText: page.windowingSystem
                        visible: envText !== ""
                        text: envText
                        icon.name: envText.indexOf("Wayland") !== -1 ? "preferences-desktop-display"
                                 : envText === "X11" ? "preferences-desktop-display"
                                 : "utilities-terminal"
                        checkable: false
                        closable: false
                    }
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
            visible: !details.loading

            StackLayout {
                currentIndex: details.installProgress > 0.0 && details.installProgress < 1.0 ? 1 : 0
                Layout.fillWidth: page.isNarrow
                Layout.preferredWidth: page.isNarrow ? -1 : Kirigami.Units.gridUnit * 13
                Layout.preferredHeight: Kirigami.Units.gridUnit * 2

                // 0: Normal Button
                Controls.Button {
                    text: {
                        if (details.loading) return L10n.i18n("Loading…", []);
                        return details.isInstalled ? L10n.i18n("Remove", []) : L10n.i18n("Install", []);
                    }
                    icon.name: details.isInstalled ? "edit-delete-symbolic" : "download-symbolic"
                    enabled: !details.loading
                    highlighted: !details.isInstalled
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    onClicked: {
                        if (details.isInstalled) {
                            details.uninstall()
                        } else {
                            details.install()
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
                        value: details.installProgress
                        indeterminate: details.installProgress < 0.02

                        Behavior on value { NumberAnimation { duration: 300 } }
                    }

                    Controls.ToolButton {
                        icon.name: "dialog-cancel"
                        display: Controls.AbstractButton.IconOnly
                        text: L10n.i18n("Cancel", [])
                        Controls.ToolTip.text: text
                        Controls.ToolTip.visible: hovered
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: {
                            details.cancel()
                        }
                    }
                }
            }

            // Permissions Button
            Controls.Button {
                id: permissionsBtn
                text: L10n.i18n("Sandbox Permissions", [])
                icon.name: "security-high-symbolic"
                Layout.fillWidth: page.isNarrow
                Layout.preferredHeight: Kirigami.Units.gridUnit * 2
                onClicked: permissionsDialog.open()
            }
        }

        // ── Separator ───────────────────────────────────────────────────
        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            visible: !details.loading
        }

        // ── Screenshots Carousel ─────────────────────────────────────────
        Item {
            id: screenshotsContainer
            Layout.fillWidth: true
            Layout.preferredHeight: !details.loading && screenshotsListView.count > 0 ? Kirigami.Units.gridUnit * 13 : 0
            visible: !details.loading && screenshotsListView.count > 0

            ListView {
                id: screenshotsListView
                anchors.fill: parent
                orientation: ListView.Horizontal
                spacing: Kirigami.Units.largeSpacing
                clip: true

                leftMargin: Kirigami.Units.largeSpacing
                rightMargin: Kirigami.Units.largeSpacing

                model: page.screenshotUrls

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
                        radius: Kirigami.Units.cornerRadius
                        color: Kirigami.Theme.alternateBackgroundColor

                        shadow.size: Kirigami.Units.gridUnit / 2
                        shadow.color: Qt.rgba(0, 0, 0, 0.1)
                        shadow.yOffset: 1

                        border.width: 1
                        border.color: screenshotMouseArea.containsMouse
                            ? Kirigami.Theme.highlightColor
                            : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)

                        Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }

                        Image {
                            id: screenshotImg
                            anchors.fill: parent
                            anchors.margins: 1
                            source: screenshotRemote.source
                            RemoteImage { id: screenshotRemote; url: modelData; kind: "screenshot" }
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            clip: true

                            Controls.BusyIndicator {
                                anchors.centerIn: parent
                                running: parent.status === Image.Loading
                                visible: parent.status === Image.Loading
                            }
                        }

                        MouseArea {
                            id: screenshotMouseArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                let idx = page.screenshotUrls.indexOf(modelData);
                                largeScreenshotPopup.openWithIndex(idx >= 0 ? idx : 0);
                            }
                        }
                    }
                }
            }

            // Left Navigation Arrow Button
            Controls.RoundButton {
                id: leftScrollButton
                anchors.left: parent.left
                anchors.leftMargin: Kirigami.Units.largeSpacing
                anchors.verticalCenter: parent.verticalCenter
                z: 10
                focusPolicy: Qt.NoFocus
                hoverEnabled: true
                icon.name: "go-previous-symbolic"
                visible: screenshotsListView.contentX > 10
                background: Rectangle {
                    radius: height / 2
                    color: Qt.rgba(Kirigami.Theme.backgroundColor.r,
                                   Kirigami.Theme.backgroundColor.g,
                                   Kirigami.Theme.backgroundColor.b,
                                   leftScrollButton.hovered ? 0.85 : 0.55)
                    border.width: 1
                    border.color: leftScrollButton.hovered
                        ? Kirigami.Theme.highlightColor
                        : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                    Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                    Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                }
                onClicked: {
                    let step = Kirigami.Units.gridUnit * 24;
                    screenshotsListView.contentX = Math.max(screenshotsListView.contentX - step, 0);
                }
            }

            // Right Navigation Arrow Button
            Controls.RoundButton {
                id: rightScrollButton
                anchors.right: parent.right
                anchors.rightMargin: Kirigami.Units.largeSpacing
                anchors.verticalCenter: parent.verticalCenter
                z: 10
                focusPolicy: Qt.NoFocus
                hoverEnabled: true
                icon.name: "go-next-symbolic"
                visible: screenshotsListView.contentX < (screenshotsListView.contentWidth - screenshotsListView.width - 10)
                background: Rectangle {
                    radius: height / 2
                    color: Qt.rgba(Kirigami.Theme.backgroundColor.r,
                                   Kirigami.Theme.backgroundColor.g,
                                   Kirigami.Theme.backgroundColor.b,
                                   rightScrollButton.hovered ? 0.85 : 0.55)
                    border.width: 1
                    border.color: rightScrollButton.hovered
                        ? Kirigami.Theme.highlightColor
                        : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                    Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                    Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
                }
                onClicked: {
                    let step = Kirigami.Units.gridUnit * 24;
                    let maxContentX = screenshotsListView.contentWidth - screenshotsListView.width;
                    screenshotsListView.contentX = Math.min(screenshotsListView.contentX + step, maxContentX);
                }
            }
        }

        // ── Description ─────────────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing
            visible: !details.loading && details.description !== ""

            Kirigami.Heading {
                level: 3
                text: L10n.i18n("Description", [])
            }

            Controls.Label {
                id: descText
                Layout.fillWidth: true
                text: details.description
                textFormat: Text.RichText
                wrapMode: Text.WordWrap
                onLinkActivated: (link) => Qt.openUrlExternally(link)
            }
        }

        // ── Links & Resources ───────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.mediumSpacing
            visible: !details.loading && hasLinks()

            Kirigami.Heading {
                level: 3
                text: L10n.i18n("Links & Resources", [])
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
                    return page.urlsData[key] || "";
                }

                // Homepage Button
                Controls.Button {
                    text: L10n.i18n("Website", [])
                    icon.name: "internet-services"
                    visible: parent.getLink("homepage") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("homepage"))
                }

                // Source Repository Button
                Controls.Button {
                    text: L10n.i18n("Source Code", [])
                    icon.name: "code-context"
                    visible: parent.getLink("vcs_browser") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("vcs_browser"))
                }

                // Manifest Button
                Controls.Button {
                    text: L10n.i18n("Flatpak Manifest", [])
                    icon.name: "text-x-qml"
                    visible: parent.getLink("manifest") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("manifest"))
                }

                // Bug Tracker Button
                Controls.Button {
                    text: L10n.i18n("Report an Issue", [])
                    icon.name: "tools-report-bug"
                    visible: parent.getLink("bugtracker") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("bugtracker"))
                }

                // Help Button
                Controls.Button {
                    text: L10n.i18n("Documentation", [])
                    icon.name: "help-browser"
                    visible: parent.getLink("help") !== ""
                    Layout.fillWidth: true
                    onClicked: Qt.openUrlExternally(parent.getLink("help"))
                }

                // Donation Button
                Controls.Button {
                    text: L10n.i18n("Donate / Support", [])
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
            title: L10n.i18n("More by %1", [String(details.developer !== "" ? details.developer : L10n.i18n("Developer", []))])
            appModel: details.developerApps
            visible: !details.loading && details.developerApps.count > 0
            Layout.fillWidth: true
        }

        // ── Similar applications ────────────────────────────────────────
        Components.HorizontalAppList {
            id: similarAppsList
            title: L10n.i18n("Similar Applications", [])
            appModel: details.similarApps
            visible: !details.loading && details.similarApps.count > 0
            Layout.fillWidth: true
        }

        // ── Separator ───────────────────────────────────────────────────
        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.largeSpacing
            visible: !details.loading
        }

        // ── Reviews & Ratings ───────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.largeSpacing
            visible: !details.loading

            RowLayout {
                Layout.fillWidth: true
                Kirigami.Heading {
                    level: 2
                    text: L10n.i18n("Ratings & Reviews", [])
                    Layout.fillWidth: true
                }

                Controls.Button {
                    text: L10n.i18n("Write a Review", [])
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
                text: L10n.i18n("No Reviews Yet", [])
                explanation: L10n.i18n("Be the first to write a review for this application!", [])
                icon.name: "document-edit"
                visible: !page.odrsLoading && ratingData.total === 0 && reviewsModel.count === 0
            }

            // Rating Summary Card
            Components.RatingSummary {
                Layout.fillWidth: true
                visible: !page.odrsLoading && ratingData.total > 0
                star0: ratingData.stars ? ratingData.stars[0] : 0
                star1: ratingData.stars ? ratingData.stars[1] : 0
                star2: ratingData.stars ? ratingData.stars[2] : 0
                star3: ratingData.stars ? ratingData.stars[3] : 0
                star4: ratingData.stars ? ratingData.stars[4] : 0
                star5: ratingData.stars ? ratingData.stars[5] : 0
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
                    details: details
                }
            }

            Controls.Button {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.mediumSpacing
                Layout.bottomMargin: Kirigami.Units.largeSpacing
                text: L10n.i18n("Show More Reviews", [])
                icon.name: "arrow-down"
                visible: !page.odrsLoading && reviewsModel.count < page.allReviewsList.length
                onClicked: loadMoreReviews()
            }
        }
    }

    // ── Write a Review Dialog ──────────────────────────────────
    Kirigami.Dialog {
        id: reviewDialog
        title: L10n.i18n("Write a Review", [])
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        width: Math.min(parent.width * 0.95, Kirigami.Units.gridUnit * 38)
        height: Math.min(parent.height * 0.85, Kirigami.Units.gridUnit * 25)

        onAccepted: {
            if (summaryInput.text !== "" && descriptionInput.text !== "") {
                // Version, distribution and locale are filled in by Rust.
                details.submitReview(
                    ratingRow.selectedRating,
                    summaryInput.text,
                    descriptionInput.text,
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
                Kirigami.FormData.label: L10n.i18n("Rating:", [])
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
                Kirigami.FormData.label: L10n.i18n("Title:", [])
                placeholderText: L10n.i18n("One-line title of your review", [])
                Layout.fillWidth: true
            }

            // Description Scrollable Area
            Controls.ScrollView {
                id: scrollView
                Kirigami.FormData.label: L10n.i18n("Review:", [])
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 12
                
                Controls.TextArea {
                    id: descriptionInput
                    placeholderText: L10n.i18n("Detailed review description", [])
                    wrapMode: TextEdit.WordWrap
                    width: scrollView.width
                }
            }

            // Publish Anonymously Toggle
            Controls.Switch {
                id: anonymousToggle
                Kirigami.FormData.label: L10n.i18n("Anonymous:", [])
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
            screenshotsList = page.screenshotUrls;
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
                                source: zoomRemote.source
                                RemoteImage { id: zoomRemote; url: modelData; kind: "screenshot" }
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
        title: L10n.i18n("Application Permissions", [])
        standardButtons: Kirigami.Dialog.Close
        width: Math.min(parent.width * 0.95, Kirigami.Units.gridUnit * 28)

        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing
            Layout.fillWidth: true

            Controls.Label {
                text: L10n.i18n("This application has permissions to access the following:", [])
                color: Kirigami.Theme.disabledTextColor
                font: Kirigami.Theme.smallFont
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                visible: {
                    let items = page.permissionItems;
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
                    model: page.permissionItems

                    delegate: ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.mediumSpacing

                            Kirigami.Icon {
                                source: modelData.icon
                                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                                Layout.preferredHeight: Kirigami.Units.iconSizes.small
                                Layout.alignment: Qt.AlignTop
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
                                    font: Kirigami.Theme.smallFont
                                }
                            }
                        }

                        Kirigami.Separator {
                            Layout.fillWidth: true
                            visible: index < page.permissionItems.length - 1
                            Layout.topMargin: Kirigami.Units.smallSpacing
                            Layout.bottomMargin: Kirigami.Units.smallSpacing
                        }
                    }
                }
            }
        }
    }
}
