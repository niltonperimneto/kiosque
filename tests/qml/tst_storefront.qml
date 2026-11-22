import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "StorefrontRedesign"
    when: windowShown
    visible: true
    width: 1000
    height: 700
    SignalSpy { id: clickSpy; signalName: "clicked" }
    Component { id: featuredFactory; FeaturedModel {} }

    function component(name, properties) {
        let definition = Qt.createComponent("qrc:/qml/components/" + name + ".qml")
        compare(definition.status, Component.Ready, definition.errorString())
        let object = createTemporaryObject(definition, this, properties || {})
        verify(object !== null)
        return object
    }
    function cleanup() { clickSpy.target = null; clickSpy.clear() }
    function test_cards_support_keyboard_activation_data() {
        return [{tag: "card", type: "StorefrontAppCard"},
                {tag: "hero", type: "HeroBanner"},
                {tag: "daily", type: "AppOfTheDayCard"}]
    }
    function test_cards_support_keyboard_activation(data) {
        let card = component(data.type, {width: 400, height: 300, appName: "Fixture"})
        clickSpy.target = card
        card.forceActiveFocus()
        tryCompare(card, "activeFocus", true)
        keyClick(Qt.Key_Return)
        compare(clickSpy.count, 1)
        keyClick(Qt.Key_Space)
        compare(clickSpy.count, 2)
    }
    function test_storefront_contains_new_sections() {
        let definition = Qt.createComponent("qrc:/qml/Main.qml")
        compare(definition.status, Component.Ready, definition.errorString())
        let root = createTemporaryObject(definition, null)
        verify(root !== null)
        let page = root.pageStack.currentItem
        tryCompare(page.mobileModel, "loading", false, 10000)
        tryCompare(page.featuredModel, "loading", false, 10000)
        let games = findChild(page, "gamesSection")
        verify(games !== null)
        tryCompare(games.newReleasesModel, "loading", false, 10000)
        tryCompare(games.trendingModel, "loading", false, 10000)
        tryCompare(games.popularModel, "loading", false, 10000)
        verify(waitForRendering(root.contentItem))
        if (testArtifactDir !== "")
            grabImage(root.contentItem).save(testArtifactDir + "/storefront.png")
        root.switchToPage("qrc:/qml/pages/SettingsPage.qml")
        let settings = root.pageStack.currentItem
        // Every settings topic is a FormCard on the same page.
        for (let name of ["updatesCard", "repositoriesCard", "accountCard", "privacyCard", "aboutCard"]) {
            let card = findChild(settings, name)
            verify(card !== null, name)
            verify(card.visible, name)
        }
        verify(waitForRendering(root.contentItem))
        if (testArtifactDir !== "")
            grabImage(root.contentItem).save(testArtifactDir + "/settings.png")
    }

    function test_daily_image_and_spotlight() {
        let card = component("AppOfTheDayCard", {width: 800, appName: "Daily", screenshotUrl: "qrc:/qml/images/logo.svg"})
        let picture = findChild(card, "dailyScreenshot")
        let light = findChild(card, "dailySpotlight")
        tryCompare(picture, "status", Image.Ready)
        compare(picture.fillMode, Image.PreserveAspectFit)
        verify(picture.width <= picture.implicitWidth)
        mouseMove(light, 100, 80)
        tryVerify(function() { return Math.abs(light.lightX - 100) < 2 })
        if (testArtifactDir !== "")
            grabImage(card).save(testArtifactDir + "/daily-spotlight.png")
    }

    function test_branding_and_screenshot_fallbacks() {
        let hero = component("HeroBanner", {width: 800, height: 300, appName: "Fixture"})
        compare(hero.showScreenshot, false)
        verify(hero.brandColor.toString().length > 0)
        hero.brandColorLight = "#123456"
        hero.brandColorDark = "#654321"
        compare(hero.brandColor.toString(), hero.darkScheme ? "#654321" : "#123456")
    }
    function test_carousel_pauses_when_hovered() {
        let model = createTemporaryObject(featuredFactory, this)
        model.refresh()
        tryCompare(model, "loading", false, 10000)
        let carousel = component("HeroCarousel", {width: 900, height: 400, model: model})
        let timer = findChild(carousel, "carouselAdvanceTimer")
        verify(timer !== null)
        tryVerify(function() { return carousel.swipeView !== null && carousel.swipeView.count > 1 })
        mouseMove(carousel, 100, 100)
        tryCompare(timer, "running", false)
        mouseMove(this, 990, 690)
        tryCompare(timer, "running", true)
    }
    // Not in PageRegressions: Kirigami Addons 1.13 logs a TypeError from its own
    // MessageDialog.qml (self-qualified enum) when the About page is created.
    function test_settings_open_standard_about_page() {
        let definition = Qt.createComponent("qrc:/qml/Main.qml")
        compare(definition.status, Component.Ready, definition.errorString())
        let root = createTemporaryObject(definition, null)
        root.switchToPage("qrc:/qml/pages/SettingsPage.qml")
        let page = root.pageStack.currentItem
        findChild(page, "aboutKiosqueButton").clicked()
        tryVerify(() => root.pageStack.currentItem.objectName === "aboutPage", 5000)
        let about = root.pageStack.currentItem
        compare(about.aboutData.displayName, "Kiosque")
        compare(about.aboutData.version, Qt.application.version)
        compare(about.aboutData.licenses[0].spdx, "GPL-3.0-or-later")
    }
}
