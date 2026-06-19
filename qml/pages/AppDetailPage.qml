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
            StoreController.loadAppDetails(appId)
        }
    }

    Connections {
        target: StoreController
        function onDetailsLoaded() {
            populateModel(developerAppsModel, StoreController.detail_developerAppsJson);
            populateModel(similarAppsModel, StoreController.detail_similar_apps_json);
            populateReviewsModel(reviewsModel, StoreController.detail_reviews_json);
            
            try {
                ratingData = JSON.parse(StoreController.detail_ratings_json);
            } catch(e) {
                ratingData = {};
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
        if (!jsonStr || jsonStr === "" || jsonStr === "[]") return;
        try {
            let list = JSON.parse(jsonStr);
            for (let i = 0; i < list.length; i++) {
                let item = list[i];
                let dateObj = new Date(item.date_created * 1000); // ODRS date is unix timestamp in seconds
                listModel.append({
                    userName: item.user_display || i18n("Anonymous"),
                    dateStr: dateObj.toLocaleDateString(Qt.locale(), Locale.ShortFormat),
                    rating: item.rating || 0,
                    summary: item.summary || "",
                    description: item.description || "",
                    version: item.version || ""
                });
            }
        } catch (e) {
            console.error("Error parsing reviews:", e);
        }
    }

    // Helper to format permissions list to HTML for the tooltip
    function formatPermissions(jsonStr) {
        if (!jsonStr || jsonStr === "{}" || jsonStr === "[]") {
            return i18n("No special permissions required (Fully Sandboxed)");
        }
        try {
            let perms = JSON.parse(jsonStr);
            let html = "";
            let hasAny = false;
            
            if (perms.shared && perms.shared.length > 0) {
                html += "<b>" + i18n("Shared:") + "</b> " + perms.shared.join(", ") + "<br/>";
                hasAny = true;
            }
            if (perms.sockets && perms.sockets.length > 0) {
                html += "<b>" + i18n("Sockets:") + "</b> " + perms.sockets.join(", ") + "<br/>";
                hasAny = true;
            }
            if (perms.devices && perms.devices.length > 0) {
                html += "<b>" + i18n("Devices:") + "</b> " + perms.devices.join(", ") + "<br/>";
                hasAny = true;
            }
            if (perms.filesystems && perms.filesystems.length > 0) {
                html += "<b>" + i18n("Filesystems:") + "</b><br/>";
                perms.filesystems.forEach(fs => {
                    html += " • " + fs + "<br/>";
                });
                hasAny = true;
            }
            if (perms.persistent && perms.persistent.length > 0) {
                html += "<b>" + i18n("Persistent Paths:") + "</b> " + perms.persistent.join(", ") + "<br/>";
                hasAny = true;
            }
            if (perms["session-bus"] && (perms["session-bus"].talk || perms["session-bus"].own)) {
                html += "<b>" + i18n("Session Bus:") + "</b><br/>";
                if (perms["session-bus"].talk) {
                    html += " • Talk: " + perms["session-bus"].talk.join(", ") + "<br/>";
                }
                if (perms["session-bus"].own) {
                    html += " • Own: " + perms["session-bus"].own.join(", ") + "<br/>";
                }
                hasAny = true;
            }
            
            if (!hasAny) {
                return i18n("No special permissions required (Fully Sandboxed)");
            }
            return html;
        } catch(e) {
            return i18n("Fully Sandboxed or permissions unavailable");
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
                icon.name: "security-high"
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
                        shadow.size: screenshotHover.hovered ? 18 : 8
                        shadow.color: screenshotHover.hovered
                            ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.22)
                            : Qt.rgba(0, 0, 0, 0.12)
                        shadow.yOffset: screenshotHover.hovered ? 4 : 2

                        border.width: 1
                        border.color: screenshotHover.hovered
                            ? Kirigami.Theme.highlightColor
                            : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

                        scale: screenshotHover.hovered ? 1.015 : 1.0

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

                        HoverHandler {
                            id: screenshotHover
                            cursorShape: Qt.PointingHandCursor
                        }

                        TapHandler {
                            onTapped: {
                                largeScreenshotPopup.imageUrl = modelData;
                                largeScreenshotPopup.open();
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
            visible: !StoreController.loading && (ratingData.total > 0 || reviewsModel.count > 0)
        }

        // ── Reviews & Ratings ───────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.largeSpacing
            visible: !StoreController.loading && (ratingData.total > 0 || reviewsModel.count > 0)

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
                    onClicked: reviewDialog.open()
                }
            }

            // Rating Summary Card
            Components.RatingSummary {
                Layout.fillWidth: true
                visible: ratingData.total > 0
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
                delegate: Components.ReviewCard {
                    Layout.fillWidth: true
                    userName: model.userName
                    dateStr: model.dateStr
                    rating: model.rating
                    summary: model.summary
                    description: model.description
                    version: model.version
                }
            }
        }
    }

    // ── Mock Review Submission Dialog ──────────────────────────────────
    Kirigami.PromptDialog {
        id: reviewDialog
        title: i18n("Write a Review")
        standardButtons: Kirigami.Dialog.Close

        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing

            Kirigami.Icon {
                source: "document-edit"
                width: Kirigami.Units.iconSizes.huge
                height: width
                Layout.alignment: Qt.AlignHCenter
                color: Kirigami.Theme.highlightColor
                opacity: 0.8
            }

            Controls.Label {
                text: i18n("Submitting reviews requires generating a unique identity hash with ODRS. This feature is currently in development and will be available in a future update.")
                wrapMode: Text.WordWrap
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }

    // ── Fullscreen Screenshot Zoom Popup ───────────────────────────────
    Controls.Popup {
        id: largeScreenshotPopup
        anchors.centerIn: parent
        width: Math.min(parent.width * 0.9, Kirigami.Units.gridUnit * 50)
        height: Math.min(parent.height * 0.85, Kirigami.Units.gridUnit * 35)
        modal: true
        focus: true
        closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutside

        background: Kirigami.ShadowedRectangle {
            color: Qt.rgba(0, 0, 0, 0.9)
            radius: 16
            border.color: Kirigami.Theme.highlightColor
            border.width: 1
            shadow.size: 24
            shadow.color: Qt.rgba(0, 0, 0, 0.5)
        }

        property string imageUrl: ""

        Image {
            id: largeScreenshotImg
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            source: largeScreenshotPopup.imageUrl
            fillMode: Image.PreserveAspectFit
            asynchronous: true

            Controls.BusyIndicator {
                anchors.centerIn: parent
                running: parent.status === Image.Loading
                visible: parent.status === Image.Loading
            }
        }

        Controls.ToolButton {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: Kirigami.Units.mediumSpacing
            icon.name: "window-close"
            onClicked: largeScreenshotPopup.close()
        }
    }

    // ── Sandbox Permissions Dialog ─────────────────────────────────────
    Kirigami.PromptDialog {
        id: permissionsDialog
        title: i18n("Application Permissions")
        standardButtons: Kirigami.Dialog.Close

        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing

            Kirigami.Icon {
                source: "security-high"
                width: Kirigami.Units.iconSizes.huge
                height: width
                Layout.alignment: Qt.AlignHCenter
                color: Kirigami.Theme.highlightColor
                opacity: 0.8
            }

            Controls.Label {
                text: formatPermissions(StoreController.detail_permissions_json)
                textFormat: Text.RichText
                wrapMode: Text.WordWrap
                Layout.maximumWidth: Kirigami.Units.gridUnit * 25
                horizontalAlignment: Text.AlignLeft
            }
        }
    }
}
