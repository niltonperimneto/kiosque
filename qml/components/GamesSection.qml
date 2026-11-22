// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

// Self-contained "Games" zone: one level-2 heading over semantic level-3
// subsections, each a horizontal row of unified app cards. Owns its models;
// the page only calls reload() on pull-to-refresh. "Top Rated" is absent by
// design — Flathub has no bulk ratings endpoint.
ColumnLayout {
    id: root
    objectName: "gamesSection"

    spacing: Kirigami.Units.largeSpacing * 2

    property AppListModel newReleasesModel: AppListModel {}
    property AppListModel trendingModel: AppListModel {}
    property AppListModel popularModel: AppListModel {}

    function reload() {
        newReleasesModel.loadCategorySorted("Game", "newest");
        trendingModel.loadCategorySorted("Game", "trending");
        popularModel.loadCategorySorted("Game", "installs_last_month");
    }

    Component.onCompleted: reload()

    RowLayout {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            source: "applications-games-symbolic"
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            level: 2
            text: L10n.i18n("Games", [])
        }
    }

    HorizontalAppList {
        Layout.fillWidth: true
        headingLevel: 3
        title: L10n.i18n("New Releases", [])
        appModel: root.newReleasesModel
    }

    HorizontalAppList {
        Layout.fillWidth: true
        headingLevel: 3
        title: L10n.i18n("Trending", [])
        appModel: root.trendingModel
    }

    HorizontalAppList {
        Layout.fillWidth: true
        headingLevel: 3
        title: L10n.i18n("Most Popular", [])
        appModel: root.popularModel
    }
}
