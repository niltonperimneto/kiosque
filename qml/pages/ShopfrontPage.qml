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

    property var appListModel: applicationWindow().appListModel
    property var featuredModel: applicationWindow().featuredModel
    
    property AppListModel popularModel: AppListModel {}
    property AppListModel newModel: AppListModel {}
    property AppListModel updatedModel: AppListModel {}

    property string activeSearchQuery: typeof searchField !== 'undefined' && searchField ? searchField.text : ""

    property bool isDefaultView: page.activeSearchQuery.length === 0

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

        // ── Search field ────────────────────────────────────────────────
        Kirigami.SearchField {
            id: searchField
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            placeholderText: i18n("Search applications…")
            onAccepted: {
                if (text.length > 0) {
                    appListModel.search(text);
                } else {
                    appListModel.refresh();
                }
            }
        }

        // ── Hero carousel ───────────────────────────────────────────────
        Components.HeroCarousel {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            model: page.featuredModel
            visible: page.isDefaultView
        }

        // ── Highlight Cards ─────────────────────────────────────────────
        GridLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            
            // On wide screens side-by-side, on narrow screens stacked
            columns: width > Kirigami.Units.gridUnit * 35 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing
            visible: page.isDefaultView

            Repeater {
                model: page.featuredModel
                delegate: Components.AppOfTheDayCard {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 15
                    
                    // Only show the first item; others have 0 dimensions so they take no space
                    visible: index === 0
                    Layout.maximumWidth: index === 0 ? -1 : 0
                    Layout.maximumHeight: index === 0 ? -1 : 0
                    
                    cardAppId: typeof appId !== 'undefined' ? appId : ""
                    cardName: typeof name !== 'undefined' ? name : ""
                    cardSummary: typeof summary !== 'undefined' ? summary : ""
                    cardIconUrl: typeof iconUrl !== 'undefined' ? iconUrl : ""

                    onClicked: {
                        applicationWindow().pushAppDetail(this.cardAppId)
                    }
                }
            }

            Components.WelcomeCard {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 15
            }
        }

        // ── Sections for Default View ───────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
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
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.mediumSpacing
            visible: !page.isDefaultView

            Kirigami.Heading {
                level: 3
                text: i18n("Search Results")
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
                            transform: Translate { y: 20; id: gridItemTranslate }
                            
                            Component.onCompleted: {
                                gridItemTranslate.y = 0;
                                opacity = 1.0;
                            }
                            
                            Behavior on opacity {
                                NumberAnimation { duration: 400; easing.type: Easing.OutCubic }
                            }
                            
                            Behavior on transform {
                                NumberAnimation { duration: 400; easing.type: Easing.OutBack }
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
