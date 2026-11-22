import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "ApplicationNavigation"
    when: windowShown
    visible: true

    function cleanup() {
        if (qtest_results.failed && TestSupport.artifactDir !== "")
            grabImage(this).save(TestSupport.artifactDir + "/" + name + "-" + Date.now() + ".png")
    }

    function createWindow() {
        let component = Qt.createComponent("qrc:/qml/Main.qml")
        compare(component.status, Component.Ready, component.errorString())
        let window = createTemporaryObject(component, null)
        verify(window !== null)
        return window
    }

    function test_startup_and_navigation() {
        let window = createWindow()
        compare(Qt.application.domain, "kiosque.app")
        window.pushAppDetail("org.kiosque.Fixture")
        tryCompare(window.pageStack.currentItem, "appId", "org.kiosque.Fixture")
        window.switchToPage("qrc:/qml/pages/InstalledPage.qml")
        verify(window.pageStack.depth > 1)
        window.switchToPage("qrc:/qml/pages/ShopfrontPage.qml")
        compare(window.pageStack.depth, 1)
    }

    function test_stacked_detail_pages_keep_their_own_state() {
        let window = createWindow()
        window.pushAppDetail("org.kiosque.Fixture")
        let first = window.pageStack.currentItem
        tryCompare(first, "title", "Fixture", 10000)
        window.pushAppDetail("org.kiosque.Other")
        let second = window.pageStack.currentItem
        verify(second !== first)
        compare(second.appId, "org.kiosque.Other")
        window.pushAppDetail("org.kiosque.Other")
        compare(window.pageStack.currentItem, second, "re-opening the visible app is a no-op")
        window.pageStack.pop()
        compare(window.pageStack.currentItem, first)
        compare(first.appId, "org.kiosque.Fixture")
        compare(first.title, "Fixture")
    }

    function test_popping_a_page_mid_load() {
        let window = createWindow()
        window.pushAppDetail("org.kiosque.Fixture")
        window.pageStack.pop()
        wait(500) // completions for the destroyed page must be dropped
        window.pushAppDetail("org.kiosque.Fixture")
        tryCompare(window.pageStack.currentItem, "title", "Fixture", 10000)
    }
}
