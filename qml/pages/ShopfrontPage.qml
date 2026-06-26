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

    title: i18n("Storefront")
    supportsRefreshing: true

    // Search lives in the toolbar so it stays accessible while scrolling.
    titleDelegate: Kirigami.SearchField {
        id: searchField
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        placeholderText: i18n("Search applications…")
        onAccepted: {
            if (text.length > 0) {
                appListModel.search(text);
            } else {
                appListModel.refresh();
            }
        }
    }

    property var appListModel: applicationWindow().appListModel
    property var featuredModel: applicationWindow().featuredModel
    
    property AppListModel popularModel: AppListModel {}
    property AppListModel newModel: AppListModel {}
    property AppListModel updatedModel: AppListModel {}

    property string activeSearchQuery: typeof searchField !== 'undefined' && searchField ? searchField.text : ""

    property bool isDefaultView: page.activeSearchQuery.length === 0

    // Cap content width on large displays and centre it, like a modern storefront.
    readonly property real contentMaxWidth: Kirigami.Units.gridUnit * 70

    Component.onCompleted: {
        popularModel.loadPopular();
        newModel.loadNew();
        updatedModel.loadUpdated();
    }

    onRefreshingChanged: {
        if (refreshing) {
            featuredModel.refresh();
            popularModel.loadPopular();
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
        spacing: Kirigami.Units.largeSpacing * 2
        // No animation to prevent blank screen issues

        // ── Hero carousel ───────────────────────────────────────────────
        Components.HeroCarousel {
            // The carousel insets its own card (for shadow room), so it aligns
            // with the other bands without extra side margins here.
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.largeSpacing
            model: page.featuredModel
            visible: page.isDefaultView
        }

        // ── App of the Day spotlight ────────────────────────────────────
        // Full-width feature surface; only the first featured entry (the
        // Flathub App of the Day) is shown, others collapse to take no space.
        Repeater {
            model: page.featuredModel
            delegate: Components.AppOfTheDayCard {
                Layout.fillWidth: true
                Layout.maximumWidth: index === 0 ? page.contentMaxWidth : 0
                Layout.maximumHeight: index === 0 ? -1 : 0
                Layout.alignment: Qt.AlignHCenter
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing

                visible: index === 0 && page.isDefaultView

                cardAppId: typeof appId !== 'undefined' ? appId : ""
                cardName: typeof name !== 'undefined' ? name : ""
                cardSummary: typeof summary !== 'undefined' ? summary : ""
                cardIconUrl: typeof iconUrl !== 'undefined' ? iconUrl : ""

                onClicked: applicationWindow().pushAppDetail(this.cardAppId)
            }
        }

        // ── Browse by Category ──────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            spacing: Kirigami.Units.smallSpacing
            visible: page.isDefaultView

            Kirigami.Heading {
                level: 2
                text: i18n("Browse by Category")
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
                        label: i18n(categoryDelegate.text)
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
            spacing: Kirigami.Units.largeSpacing * 2
            visible: page.isDefaultView

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: i18n("Popular Applications")
                appModel: page.popularModel
            }

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: i18n("New & Noteworthy")
                appModel: page.newModel
            }

            Components.HorizontalAppList {
                Layout.fillWidth: true
                title: i18n("Recently Updated")
                appModel: page.updatedModel
            }
        }

        // ── Section: Search / Category Applications ──────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.maximumWidth: page.contentMaxWidth
            Layout.alignment: Qt.AlignHCenter
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.mediumSpacing
            visible: !page.isDefaultView

            Kirigami.Heading {
                level: 2
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: appGrid.count > 0
                    ? i18np("%1 result for “%2”", "%1 results for “%2”", appGrid.count, page.activeSearchQuery)
                    : i18n("Search Results")
            }

            // Loading state
            Controls.BusyIndicator {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.gridUnit * 4
                Layout.preferredWidth: Kirigami.Units.gridUnit * 4
                Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                running: page.appListModel && page.appListModel.loading
                visible: running
            }

            // Empty state
            Kirigami.PlaceholderMessage {
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 8
                icon.name: "edit-find"
                text: i18n("No Applications Found")
                explanation: i18n("Try a different search term or category")
                visible: (!page.appListModel || !page.appListModel.loading) && appGrid.count === 0
            }

            // ── App grid ────────────────────────────────────────────────
            GridLayout {
                id: appGrid

                Layout.fillWidth: true

                property int count: appGridRepeater.count
                property int minCellWidth: Kirigami.Units.gridUnit * 12
                columns: Math.max(1, Math.floor(width / minCellWidth))
                columnSpacing: Kirigami.Units.largeSpacing
                rowSpacing: Kirigami.Units.largeSpacing

                visible: count > 0

                Repeater {
                    id: appGridRepeater
                    model: page.appListModel

                    delegate: Item {
                        id: delegateWrapper
                        Layout.fillWidth: true
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 12

                        required property string appId
                        required property string name
                        required property string summary
                        required property string iconUrl

                        Components.AppCard {
                            anchors.fill: parent
                            appId: delegateWrapper.appId
                            name: delegateWrapper.name
                            summary: delegateWrapper.summary
                            iconUrl: delegateWrapper.iconUrl

                            opacity: 0
                            Component.onCompleted: opacity = 1.0
                            Behavior on opacity {
                                NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic }
                            }
                        }
                    }
                }
            }
        }

        // Bottom spacing
        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing * 2
        }
    }
}
