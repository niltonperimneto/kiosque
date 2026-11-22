// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import org.kde.ki18n

// Single source of the storefront's category tree. Labels are translated
// here, so consumers display `text` as-is and never pass it through i18n().
// Instantiated once by Main.qml (not a singleton) so the translations are
// evaluated in the application's localization context.
QtObject {
    id: categories

    // Top-level AppStream categories; `category` is the Flathub identifier,
    // empty for "everything".
    readonly property var all: [
        { text: i18n("All Categories"), icon: "applications-all-symbolic", category: "" },
        { text: i18n("Audio & Video"), icon: "applications-multimedia-symbolic", category: "AudioVideo" },
        { text: i18n("Games"), icon: "applications-games-symbolic", category: "Game" },
        { text: i18n("Office"), icon: "applications-office-symbolic", category: "Office" },
        { text: i18n("Graphics"), icon: "applications-graphics-symbolic", category: "Graphics" },
        { text: i18n("Development"), icon: "applications-development-symbolic", category: "Development" },
        { text: i18n("Internet"), icon: "applications-internet-symbolic", category: "Network" },
        { text: i18n("Utilities"), icon: "applications-utilities-symbolic", category: "Utility" },
        { text: i18n("Education"), icon: "applications-education-symbolic", category: "Education" },
        { text: i18n("System"), icon: "applications-system-symbolic", category: "System" },
        { text: i18n("Science"), icon: "applications-science-symbolic", category: "Science" }
    ]

    // Only categories that name a Flathub category (no "All Categories").
    readonly property var concrete: all.filter(c => c.category !== "")

    readonly property var _subcategories: ({
        "Game": [
            { text: i18n("Games"), icon: "applications-games-symbolic", categoryId: "Game-Game" },
            { text: i18n("Emulators"), icon: "input-gamepad-symbolic", categoryId: "Game-Emulator" },
            { text: i18n("Game Launchers"), icon: "system-run-symbolic", categoryId: "Game-Launcher" },
            { text: i18n("Game Tools"), icon: "applications-utilities-symbolic", categoryId: "Game-Tool" }
        ],
        "AudioVideo": [
            { text: i18n("Audio & Video"), icon: "applications-multimedia-symbolic", categoryId: "AudioVideo-All" },
            { text: i18n("Players"), icon: "multimedia-player-symbolic", categoryId: "AudioVideo-Player" },
            { text: i18n("Recorders"), icon: "media-record-symbolic", categoryId: "AudioVideo-Recorder" },
            { text: i18n("Editors & Creators"), icon: "document-edit-symbolic", categoryId: "AudioVideo-Editing" }
        ],
        "Development": [
            { text: i18n("Developer Tools"), icon: "applications-development-symbolic", categoryId: "Development-All" },
            { text: i18n("IDEs"), icon: "code-context-symbolic", categoryId: "Development-IDE" },
            { text: i18n("Debuggers"), icon: "debug-run-symbolic", categoryId: "Development-Debugger" },
            { text: i18n("Web Development"), icon: "applications-internet-symbolic", categoryId: "Development-Web" }
        ],
        "Graphics": [
            { text: i18n("Graphics"), icon: "applications-graphics-symbolic", categoryId: "Graphics-All" },
            { text: i18n("3D Graphics"), icon: "draw-cuboid-symbolic", categoryId: "Graphics-3D" },
            { text: i18n("Vector Graphics"), icon: "draw-bezier-curves-symbolic", categoryId: "Graphics-Vector" },
            { text: i18n("Raster Graphics"), icon: "draw-brush-symbolic", categoryId: "Graphics-Raster" },
            { text: i18n("Photography"), icon: "camera-photo-symbolic", categoryId: "Graphics-Photography" }
        ],
        "Office": [
            { text: i18n("Office"), icon: "applications-office-symbolic", categoryId: "Office-All" },
            { text: i18n("Word Processors"), icon: "document-new-symbolic", categoryId: "Office-WordProcessor" },
            { text: i18n("Spreadsheets"), icon: "table-symbolic", categoryId: "Office-Spreadsheet" },
            { text: i18n("Presentations"), icon: "view-presentation-symbolic", categoryId: "Office-Presentation" },
            { text: i18n("Finance"), icon: "taxes-finances-symbolic", categoryId: "Office-Finance" }
        ]
    })

    // Subcategories of a top-level category ("Game" or "Game-Emulator").
    function subcategories(categoryId) {
        const parent = (categoryId || "").split("-")[0];
        return _subcategories[parent] || [];
    }

    // Translated display name for a category or subcategory identifier.
    function displayName(categoryId) {
        for (const c of all) {
            if (c.category === categoryId)
                return c.text;
        }
        for (const s of subcategories(categoryId)) {
            if (s.categoryId === categoryId)
                return s.text;
        }
        return categoryId;
    }
}
