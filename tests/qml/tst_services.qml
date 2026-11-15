import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "ServiceInteractions"
    when: windowShown
    visible: true
    function cleanup() {
        if (qtest_results.failed && testArtifactDir !== "")
            grabImage(this).save(testArtifactDir + "/" + name + "-" + Date.now() + ".png")
    }
    width: 640
    height: 480
    Component { id: appFactory; AppListModel {} }
    Component { id: installedFactory; InstalledModel {} }
    Component {
        id: viewFactory
        ListView {
            width: 320; height: 200
            delegate: Item { required property string appId; width: 320; height: 24 }
        }
    }

    function test_search_states_and_latest_response() {
        let model = createTemporaryObject(appFactory, this)
        verify(model !== null)
        let view = createTemporaryObject(viewFactory, this, { model: model })
        model.search("slow")
        model.search("fast")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 1)
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        compare(view.itemAtIndex(0).appId, "org.kiosque.fast")
        wait(500) // Let the deliberately delayed response arrive; it must be ignored.
        tryCompare(view, "count", 1)
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        compare(view.itemAtIndex(0).appId, "org.kiosque.fast")
        model.search("empty")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 0)
        model.search("error")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 0)
    }

    function test_storefront_collections_and_newest_order() {
        let model = createTemporaryObject(appFactory, this)
        let view = createTemporaryObject(viewFactory, this, { model: model })
        model.loadCategorySorted("Game", "newest")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 3)
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        compare(view.itemAtIndex(0).appId, "org.kiosque.New")
        model.loadCategorySorted("Slow", "trending")
        model.loadMobile()
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 1)
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        compare(view.itemAtIndex(0).appId, "org.kiosque.Mobile")
        wait(500) // Deliver the deliberately delayed fixture response.
        compare(view.count, 1)
        compare(view.itemAtIndex(0).appId, "org.kiosque.Mobile")
        model.loadCategorySorted("Game", "invalid")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 0)
    }

    function test_details_and_object_destruction() {
        let detail = StoreController
        detail.loadAppDetails("org.kiosque.Fixture")
        tryCompare(detail, "loading", false, 10000)
        compare(detail.detail_name, "Fixture")
        let pending = appFactory.createObject(this)
        pending.search("slow")
        pending.destroy()
        wait(500) // The queued Rust completion must tolerate a destroyed QObject.
    }

    function test_transaction_state_is_shared() {
        let installed = createTemporaryObject(installedFactory, this)
        let detail = StoreController
        let view = createTemporaryObject(viewFactory, this, { model: installed })
        installed.refresh()
        detail.loadAppDetails("org.kiosque.Fixture")
        tryCompare(installed, "loading", false, 10000)
        tryCompare(detail, "loading", false, 10000)
        tryCompare(view, "count", 1)
        detail.uninstallApp("org.kiosque.Fixture")
        tryCompare(detail, "detail_is_installed", false, 10000)
        tryCompare(view, "count", 0, 10000)
        detail.installApp("org.kiosque.Fixture")
        tryCompare(detail, "detail_is_installed", true, 10000)
        tryCompare(view, "count", 1, 10000)
    }

    function test_settings_roundtrip() {
        SettingsController.loadSettings()
        SettingsController.saveSettings(true, "Weekly", "03:15")
        tryCompare(SettingsController, "saving_settings", false, 10000)
        SettingsController.loadSettings()
        compare(SettingsController.auto_update, true)
        compare(SettingsController.update_frequency, "Weekly")
        compare(SettingsController.update_time, "03:15")
    }
}
