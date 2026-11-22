import QtQuick
import QtTest
import com.kiosque

// Model contracts of the real bridge types (formerly tests/qt/models.cpp).
TestCase {
    name: "ModelContracts"
    when: windowShown
    visible: true
    width: 320
    height: 240

    Component { id: appFactory; AppListModel {} }
    Component { id: installedFactory; InstalledModel {} }
    Component {
        id: appView
        ListView {
            width: 320; height: 200
            delegate: Item {
                required property string name
                required property string summary
                required property string iconUrl
                required property string appId
                width: 320; height: 24
            }
        }
    }
    Component {
        id: installedView
        ListView {
            width: 320; height: 200
            delegate: Item {
                required property string name
                required property string appId
                required property string version
                required property string size
                required property string origin
                required property bool hasUpdate
                required property bool isChecked
                required property bool isRuntime
                required property string sectionGroup
                width: 320; height: 24
            }
        }
    }
    SignalSpy { id: resetSpy; signalName: "modelReset" }
    SignalSpy { id: loadingSpy; signalName: "loadingChanged" }
    SignalSpy { id: countSpy; signalName: "countChanged" }
    SignalSpy { id: changedSpy; signalName: "dataChanged" }

    function spyOn(model) {
        for (let spy of [resetSpy, loadingSpy, countSpy, changedSpy]) {
            spy.clear()
            spy.target = model
        }
    }

    function test_app_list_roles_and_loading_signals() {
        let model = createTemporaryObject(appFactory, this)
        spyOn(model)
        let view = createTemporaryObject(appView, this, { model: model })
        model.load("popular", "")
        compare(model.loading, true)
        tryCompare(model, "loading", false, 10000)
        compare(loadingSpy.count, 2, "loading goes true then false exactly once")
        verify(resetSpy.count >= 1, "results arrive through a model reset")
        compare(model.count, model.rowCount(model.index(-1,-1)))
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        let item = view.itemAtIndex(0)
        verify(item.appId !== "")
        verify(item.name !== "")
        // Unknown sources do nothing.
        loadingSpy.clear()
        model.load("bogus", "")
        compare(loadingSpy.count, 0)
    }

    function test_clear_rejects_in_flight_results() {
        let model = createTemporaryObject(appFactory, this)
        model.load("search", "slow")
        wait(400)
        model.clear()
        compare(model.loading, false)
        wait(1200)
        compare(model.count, 0, "a cancelled request's result is dropped")
    }

    function test_installed_roles_toggle_and_counts() {
        let model = createTemporaryObject(installedFactory, this)
        let view = createTemporaryObject(installedView, this, { model: model })
        model.refresh()
        tryCompare(model, "loading", false, 10000)
        verify(BackendFixture.setUpdates(["org.kiosque.Fixture"]))
        model.checkForUpdates()
        tryCompare(model, "checking_updates", false, 10000)
        tryCompare(model, "updates_available_count", 1, 10000)
        compare(model.updates_checked_count, 1)
        tryVerify(function() { return view.itemAtIndex(0) !== null })
        let row = view.itemAtIndex(0)
        compare(row.appId, "org.kiosque.Fixture")
        compare(row.hasUpdate, true)
        compare(row.sectionGroup, "updates")
        spyOn(model)
        model.toggleUpdateChecked(0)
        compare(changedSpy.count, 1, "toggling emits dataChanged for one row")
        compare(resetSpy.count, 0, "toggling does not reset the model")
        compare(model.updates_checked_count, 0)
        compare(view.itemAtIndex(0).isChecked, false)
        model.setAllUpdatesChecked(true)
        compare(model.updates_checked_count, 1)
        model.applySearchFilter("no such app")
        compare(model.count, 0)
        model.applySearchFilter("")
        compare(model.count, 1)
    }

    function test_repositories_singleton() {
        spyOn(RepoModel)
        RepoModel.refresh()
        tryCompare(RepoModel, "loading", false, 10000)
        compare(RepoModel.count, RepoModel.rowCount(RepoModel.index(-1,-1)))
    }
}
