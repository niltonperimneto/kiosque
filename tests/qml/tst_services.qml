import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "ServiceInteractions"
    when: windowShown
    visible: true
    width: 640
    height: 480

    function cleanup() {
        if (qtest_results.failed && TestSupport.artifactDir !== "")
            grabImage(this).save(TestSupport.artifactDir + "/" + name + "-" + Date.now() + ".png")
    }

    Component { id: appFactory; AppListModel {} }
    Component { id: installedFactory; InstalledModel {} }
    Component { id: detailsFactory; AppDetails {} }
    Component { id: imageFactory; RemoteImage {} }
    Component {
        id: viewFactory
        ListView {
            width: 320; height: 200
            delegate: Item { required property string appId; width: 320; height: 24 }
        }
    }

    function fixtureQueries() {
        let xhr = new XMLHttpRequest();
        xhr.open("GET", TestSupport.endpoint + "/_queries", false);
        xhr.send();
        return JSON.parse(xhr.responseText);
    }

    function test_search_is_debounced() {
        let model = createTemporaryObject(appFactory, this)
        model.load("search", "deb")
        model.load("search", "debo")
        model.load("search", "debounced")
        tryCompare(model, "loading", false, 10000)
        let queries = fixtureQueries()
        verify(queries.indexOf("debounced") !== -1)
        compare(queries.indexOf("deb"), -1, "superseded keystrokes are never sent")
        compare(queries.indexOf("debo"), -1)
    }

    function test_search_states_and_latest_response() {
        let model = createTemporaryObject(appFactory, this)
        let view = createTemporaryObject(viewFactory, this, { model: model })
        model.load("search", "slow")
        wait(400) // past the debounce: the slow request is in flight
        model.load("search", "fast")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 1)
        compare(model.count, 1)
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        compare(view.itemAtIndex(0).appId, "org.kiosque.fast")
        wait(1200) // the stale "slow" response arrives now and must be ignored
        compare(view.itemAtIndex(0).appId, "org.kiosque.fast")
        model.load("search", "empty")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 0)
        compare(model.errorString, "")
        model.load("search", "error")
        tryCompare(model, "loading", false, 10000)
        tryCompare(view, "count", 0)
        verify(model.errorString !== "")
    }

    function test_details_and_object_destruction() {
        let detail = createTemporaryObject(detailsFactory, this, { appId: "org.kiosque.Fixture" })
        tryCompare(detail, "loading", false, 10000)
        compare(detail.name, "Fixture")
        compare(detail.description, "<p>Fixture details</p>")
        verify(Array.isArray(detail.screenshots))
        compare(typeof detail.links, "object")
        tryCompare(detail, "reviewsLoading", false, 10000)
        compare(detail.ratings.stars.length, 6)
        // Destroyed while their requests are pending: the late completions
        // must be dropped without touching freed objects.
        let pending = appFactory.createObject(this)
        pending.load("search", "slow")
        let pendingDetail = detailsFactory.createObject(this, { appId: "org.kiosque.Fixture" })
        pendingDetail.dispose()
        pendingDetail.destroy()
        wait(400)
        pending.destroy()
        wait(1200)
    }

    function test_detail_pages_are_independent() {
        let a = createTemporaryObject(detailsFactory, this, { appId: "org.kiosque.Fixture" })
        let b = createTemporaryObject(detailsFactory, this, { appId: "org.kiosque.Other" })
        tryCompare(a, "loading", false, 10000)
        tryCompare(b, "loading", false, 10000)
        compare(a.appId, "org.kiosque.Fixture")
        compare(b.appId, "org.kiosque.Other")
        tryCompare(a, "isInstalled", true, 10000)
        compare(b.isInstalled, false)
    }

    function test_transaction_state_is_shared() {
        let installed = createTemporaryObject(installedFactory, this)
        let detail = createTemporaryObject(detailsFactory, this, { appId: "org.kiosque.Fixture" })
        let view = createTemporaryObject(viewFactory, this, { model: installed })
        installed.refresh()
        tryCompare(installed, "loading", false, 10000)
        tryCompare(detail, "loading", false, 10000)
        tryCompare(view, "count", 1)
        detail.uninstall()
        tryCompare(detail, "isInstalled", false, 10000)
        tryCompare(view, "count", 0, 10000)
        tryCompare(detail, "busy", false, 10000)
        compare(detail.installProgress, 0)
        detail.install()
        tryCompare(detail, "isInstalled", true, 10000)
        tryCompare(view, "count", 1, 10000)
    }

    function test_remote_images_are_sanitized_pngs() {
        let ok = createTemporaryObject(imageFactory, this, { url: TestSupport.endpoint + "/images/icon.png", kind: "icon:64" })
        tryVerify(function() { return ok.source !== "" }, 10000)
        verify(ok.source.startsWith("file://"), ok.source)
        verify(ok.source.endsWith("-icon64.png"), ok.source)
        compare(ok.failed, false)
        // A second element for the same URL is a synchronous cache hit.
        let again = createTemporaryObject(imageFactory, this, { url: TestSupport.endpoint + "/images/icon.png", kind: "icon:64" })
        compare(again.source, ok.source)

        let corrupt = createTemporaryObject(imageFactory, this, { url: TestSupport.endpoint + "/images/corrupt.png", kind: "icon:64" })
        tryCompare(corrupt, "failed", true, 10000)
        compare(corrupt.source, "")

        let insecure = createTemporaryObject(imageFactory, this, { url: "http://example.invalid/icon.png", kind: "icon:64" })
        tryCompare(insecure, "failed", true, 10000)
        compare(insecure.source, "")
    }

    // Signals are emitted from Rust after state is updated. Handlers may
    // read the emitter synchronously; calls that mutate it go through
    // Qt.callLater (a re-entrant mutable borrow would abort).
    function test_reentrant_handlers_use_call_later() {
        let model = createTemporaryObject(appFactory, this)
        let seen = []
        model.countChanged.connect(function() {
            seen.push(model.count + ":" + model.loading)
            if (model.count > 0)
                Qt.callLater(model.clear)
        })
        model.load("popular", "")
        tryCompare(model, "count", 0, 10000)
        verify(seen.length >= 2, JSON.stringify(seen))
        compare(seen[0], "1:true", "count changes before loading ends")
    }

    function test_settings_roundtrip() {
        SettingsController.loadSettings()
        SettingsController.saveSettings(true, "Weekly", "03:15")
        SettingsController.loadSettings()
        compare(SettingsController.auto_update, true)
        compare(SettingsController.update_frequency, "Weekly")
        compare(SettingsController.update_time, "03:15")
    }

    function test_translations_substitute_arguments() {
        compare(L10n.i18n("%1 of %2", ["3", "10"]), "3 of 10")
        compare(L10n.i18np("%1 update", "%1 updates", 1, []), "1 update")
        compare(L10n.i18np("%1 update", "%1 updates", 4, []), "4 updates")
        compare(L10n.i18nc("verb", "Open", []), "Open")
    }
}
