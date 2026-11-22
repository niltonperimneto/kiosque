import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "QtbridgeContracts"
    when: windowShown
    visible: true
    width: 640
    height: 480
    Component { id: appFactory; AppListModel {} }
    Component { id: detailFactory; AppDetails {} }
    Component { id: imageFactory; RemoteImage {} }

    function test_screenshot_and_related_lists_are_arrays() {
        let detail = createTemporaryObject(detailFactory, this, {appId: "org.kiosque.Screenshots"})
        tryCompare(detail, "loading", false, 10000)
        verify(Array.isArray(detail.screenshots))
        verify(detail.screenshots.length > 0)
        verify(Array.isArray(detail.developerApps))
        verify(Array.isArray(detail.similarApps))
        tryCompare(detail, "reviewsLoading", false, 10000)
        verify(Array.isArray(detail.reviews))
    }
    function test_deferred_signal_handler_can_clear_model() {
        let model = createTemporaryObject(appFactory, this)
        let observed = []
        model.countChanged.connect(function() {
            observed.push(model.count)
            if (model.count > 0) Qt.callLater(model.clear)
        })
        model.loadPopular()
        tryVerify(function() { return observed.length >= 2 }, 10000)
        verify(observed[0] > 0)
        compare(observed[observed.length - 1], 0)
        compare(model.count, 0)
    }
    function test_remote_pixels_are_sanitized_and_reused() {
        let image = createTemporaryObject(imageFactory, this, {url: TestSupport.endpoint + "/fixture-image.png", kind: "icon:64"})
        tryVerify(function() { return image.source !== "" }, 10000)
        verify(image.source.startsWith("file://"))
        verify(image.source.endsWith("-icon64.png"))
        compare(image.failed, false)
        let cached = createTemporaryObject(imageFactory, this, {url: TestSupport.endpoint + "/fixture-image.png", kind: "icon:64"})
        compare(cached.source, image.source)
        let rejected = createTemporaryObject(imageFactory, this, {url: "http://example.invalid/icon.png", kind: "icon:64"})
        tryCompare(rejected, "failed", true, 10000)
        compare(rejected.source, "")
    }
    function test_dispose_ignores_pending_detail_response() {
        let detail = detailFactory.createObject(this, {appId:"org.kiosque.Fixture"})
        detail.dispose()
        detail.destroy()
        let model = appFactory.createObject(this)
        model.search("slow")
        wait(300)
        model.destroy()
        wait(700)
    }
    function test_localization_arguments() {
        compare(L10n.i18n("%1 of %2", ["3", "10"]), "3 of 10")
        compare(L10n.i18np("%1 update", "%1 updates", 4, []), "4 updates")
    }
}
