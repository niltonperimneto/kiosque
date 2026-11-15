import QtQuick
import QtTest

TestCase {
    name: "ApplicationNavigation"
    when: windowShown
    visible: true
    function cleanup() {
        if (qtest_results.failed && testArtifactDir !== "")
            grabImage(this).save(testArtifactDir + "/" + name + "-" + Date.now() + ".png")
    }
    function test_startup_and_navigation() {
        let component = Qt.createComponent("qrc:/qml/Main.qml")
        compare(component.status, Component.Ready, component.errorString())
        let window = createTemporaryObject(component, null)
        verify(window !== null)
        window.pushAppDetail("org.kiosque.Fixture")
        tryCompare(window.pageStack.currentItem, "appId", "org.kiosque.Fixture")
        window.switchToPage("qrc:/qml/pages/InstalledPage.qml")
        verify(window.pageStack.depth > 1)
        window.switchToPage("qrc:/qml/pages/ShopfrontPage.qml")
        compare(window.pageStack.depth, 1)
    }
}
