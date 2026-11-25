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

    title: L10n.i18n("Storefront", [])
    supportsRefreshing: true
    // Its title search field handles Ctrl+F (see Main.qml).
    readonly property bool hasOwnSearch: true

    // The toolbar field starts a catalog search on the Search page.
    titleDelegate: Kirigami.SearchField {
        id: searchField
        objectName: "storefrontSearchField"
        Layout.fillWidth: true
        placeholderText: L10n.i18n("Search applications…", [])
        autoAccept: false
        onTextEdited: handOffSearch()
        onAccepted: handOffSearch()

        function handOffSearch() {
            const query = text;
            if (query.trim().length > 0) {
                text = "";
                applicationWindow().openSearch(query);
            }
        }
    }

    property var appListModel: applicationWindow().appListModel
    property var featuredModel: applicationWindow().featuredModel
    
    property AppListModel popularModel: AppListModel {}
    property AppListModel newModel: AppListModel {}
    property AppListModel updatedModel: AppListModel {}
    property AppListModel mobileModel: AppListModel {}

    // Flathub's App of the Day, published by the featured model separately
    // from the carousel entries so the two never show the same app twice.
    readonly property var aotd: {
        try {
            return JSON.parse(featuredModel.aotd_json || "{}");
        } catch (e) {
            return ({});
        }
    }

    // Cap content width on large displays and centre it, like a modern storefront.
    readonly property real contentMaxWidth: Kirigami.Units.gridUnit * 70

    Component.onCompleted: {
        popularModel.loadPopular();
        mobileModel.loadMobile();
        newModel.loadNew();
        updatedModel.loadUpdated();
    }

    onRefreshingChanged: {
        if (refreshing) {
            featuredModel.refresh();
            popularModel.loadPopular();
            gamesSection.reload();
            mobileModel.loadMobile();
            newModel.loadNew();
            updatedModel.loadUpdated();
            appListModel.refresh();
            refreshing = false;
        }
    }

    // ── Page content ────────────────────────────────────────────────────
    ColumnLayout {
        id: pageContentLayout
        width: page.width
        // Dashboard-style air between content zones.
        spacing: Kirigami.Units.gridUnit * 2
        // No animation to prevent blank screen issues

        Kirigami.InlineMessage {
            objectName: "storefrontErrorMessage"
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            type: Kirigami.MessageType.Error
            visible: !page.popularModel.loading && page.popularModel.error_message !== ""
            text: L10n.i18n("Flathub could not be reached: %1", [page.popularModel.error_message])
            actions: Kirigami.Action {
                text: L10n.i18n("Try Again", [])
                icon.name: "view-refresh"
                onTriggered: page.refreshing = true
            }
        }

        // ── Hero carousel ───────────────────────────────────────────────
        Components.HeroCarousel {
            // The carousel insets its own card (for shadow room), so it aligns
            // with the other bands without extra side margins here.
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.largeSpacing
            model: page.featuredModel
        }

        // ── App of the Day spotlight ────────────────────────────────────
        // Flathub's daily pick as a standalone vertical card; the featured
        // model keeps this entry out of the carousel so it only appears here.
        // CardsLayout constrains the card to a prominent-but-not-edge-to-edge
        // column width.
        Kirigami.CardsLayout {
            visible: !!page.aotd.appId
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            maximumColumns: 1
            maximumColumnWidth: Kirigami.Units.gridUnit * 40

            Components.AppOfTheDayCard {
                appName: page.aotd.name || ""
                appSummary: page.aotd.summary || ""
                developer: page.aotd.developer || ""
                iconUrl: page.aotd.iconUrl || ""
                screenshotUrl: page.aotd.screenshotUrl || ""
                screenshots: page.aotd.screenshots || []

                onClicked: applicationWindow().pushAppDetail(page.aotd.appId)
            }
        }

        // ── Browse by Category ──────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                level: 2
                text: L10n.i18n("Browse by Category", [])
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
            }

            ListView {
                id: categoryStrip
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5

                orientation: ListView.Horizontal
                clip: true
                spacing: Kirigami.Units.smallSpacing
                leftMargin: Kirigami.Units.largeSpacing
                rightMargin: Kirigami.Units.largeSpacing

                model: applicationWindow().categoriesModel

                delegate: Item {
                    id: categoryDelegate

                    required property string text
                    required property string category

                    width: chip.implicitWidth
                    height: categoryStrip.height

                    Components.CategoryChip {
                        id: chip
                        anchors.verticalCenter: parent.verticalCenter
                        label: categoryDelegate.text
                        categoryId: categoryDelegate.category
                        onClicked: {
                            applicationWindow().currentSection = "categories";
                            applicationWindow().currentCategory = categoryDelegate.category;
                            applicationWindow().pageStack.push("qrc:/qml/pages/CategoryAppListPage.qml",
                                { categoryId: categoryDelegate.category, categoryName: categoryDelegate.text });
                        }
                    }
                }
            }
        }

        // ── Sections for Default View ───────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            spacing: Kirigami.Units.gridUnit * 2

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: L10n.i18n("Popular Applications", [])
                appModel: page.popularModel
            }

            Components.GamesSection {
                id: gamesSection
                Layout.fillWidth: true
            }

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: L10n.i18n("On the Go", [])
                subtitle: L10n.i18n("Apps that work great on phones and tablets", [])
                iconName: "phone-symbolic"
                appModel: page.mobileModel
            }

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: L10n.i18n("New & Noteworthy", [])
                appModel: page.newModel
            }

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: L10n.i18n("Recently Updated", [])
                appModel: page.updatedModel
            }
        }

        // Bottom spacing
        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing * 2
        }
    }
}
