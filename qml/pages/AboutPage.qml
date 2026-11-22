// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import org.kde.ki18n
import org.kde.kirigamiaddons.formcard as FormCard

// Standard KDE About page fed with a KAboutData-shaped object, so Kiosque does
// not need KCoreAddons' C++ KAboutData to present version, license, links and
// the libraries in use.
FormCard.AboutPage {
    id: page
    objectName: "aboutPage"

    aboutData: ({
        displayName: "Kiosque",
        productName: "kiosque",
        componentName: "kiosque",
        programLogo: "qrc:/qml/images/logo.svg",
        shortDescription: i18n("Flatpak storefront for KDE Plasma"),
        homepage: "https://github.com/niltonperimneto/kiosque",
        bugAddress: "https://github.com/niltonperimneto/kiosque/issues",
        version: Qt.application.version,
        otherText: i18n("Kiosque combines a Rust backend for Flathub, ODRS reviews and Flatpak operations with a Qt 6 and Kirigami interface."),
        authors: [
            { name: i18n("Kiosque Contributors"), task: i18n("Development"), emailAddress: "", webAddress: "https://github.com/niltonperimneto/kiosque/graphs/contributors", ocsUsername: "" }
        ],
        credits: [],
        translators: [],
        licenses: [
            {
                name: i18n("GNU General Public License v3 or later"),
                text: i18n("This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.\n\nThis program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details: https://www.gnu.org/licenses/gpl-3.0.html"),
                spdx: "GPL-3.0-or-later"
            }
        ],
        copyrightStatement: i18n("© 2026 Kiosque Contributors"),
        desktopFileName: "org.kiosque.Kiosque"
    })
}
