// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

pragma Singleton
import QtQuick
import com.kiosque
import org.kde.kirigami as Kirigami

// Centralized visual identity for the primitive-built storefront components.
// Keeps typography, kinetics and shadow defaults in one place so the custom
// look stays consistent and is tuned from a single file.
QtObject {
    id: style

    // Optional bundled font; an empty source uses the desktop's system font.
    // Register a resource before configuring this (see qml/fonts/README.md).
    readonly property url fontSource: ""
    readonly property FontLoader _loader: FontLoader {
        source: style.fontSource
    }
    readonly property string fontFamily:
        _loader.status === FontLoader.Ready ? _loader.name
                                            : Kirigami.Theme.defaultFont.family

    // Explicit weights so callers never rely on font defaults.
    readonly property int weightRegular:  Font.Normal
    readonly property int weightMedium:   Font.Medium
    readonly property int weightDemiBold: Font.DemiBold
    readonly property int weightBold:     Font.Bold

    // QtRendering (vector) is chosen over NativeRendering because the cards
    // scale on hover; native glyphs blur under a transform, vector glyphs stay
    // crisp.
    readonly property int textRenderType: Text.QtRendering

    // Kinetics — premium, tactile hover.
    readonly property int  hoverDuration: 250
    readonly property int  hoverEasing:   Easing.OutExpo
    readonly property real hoverScale:    1.02

    // Subtle default tint used when the model supplies no per-app color.
    readonly property color defaultShadow: Qt.rgba(0, 0, 0, 0.35)
}
