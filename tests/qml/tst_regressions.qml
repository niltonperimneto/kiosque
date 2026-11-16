import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "PageRegressions"
    when: windowShown
    visible: true

    SignalSpy { id: settingsSpy; target: SettingsController; signalName: "settingsSaved" }
    SignalSpy { id: reviewSpy; signalName: "reviewActionFinished" }
    SignalSpy { id: installSpy; target: StoreController; signalName: "installFinished" }
    SignalSpy { id: remoteAddedSpy; target: RepoModel; signalName: "remoteAdded" }
    SignalSpy { id: remoteRemovedSpy; target: RepoModel; signalName: "remoteRemoved" }

    function init() {
        tryCompare(SettingsController, "saving_settings", false, 10000)
        backgroundPortalFixture.denyNext = false
        settingsSpy.clear()
        failOnWarning(/TypeError|ReferenceError/)
    }
    function cleanup() {
        reviewSpy.target = null
        backendFixture.holdOperations(false)
    }
    function window() {
        let component = Qt.createComponent("qrc:/qml/Main.qml")
        compare(component.status, Component.Ready, component.errorString())
        let root = createTemporaryObject(component, null)
        verify(root !== null)
        return root
    }
    function saveSettings(enabled, frequency, time) {
        SettingsController.saveSettings(enabled, frequency, time)
        tryCompare(SettingsController, "saving_settings", false, 10000)
    }
    function test_autostart_permission_and_denial() {
        let root = window()
        root.switchToPage("qrc:/qml/pages/SettingsPage.qml")
        let page = root.pageStack.currentItem
        saveSettings(false, "Daily", "02:00")
        let before = backgroundPortalFixture.requestCount
        saveSettings(true, "Weekly", "03:15")
        compare(backgroundPortalFixture.requestCount, before + 1)
        compare(backgroundPortalFixture.lastAutostart, true)
        compare(backgroundPortalFixture.lastCommand.join(" "), "kiosque-update --background")
        compare(SettingsController.auto_update, true)
        // Editing the schedule does not prompt again for background permission.
        saveSettings(true, "Daily", "04:00")
        compare(backgroundPortalFixture.requestCount, before + 1)

        backgroundPortalFixture.denyNext = true
        settingsSpy.clear()
        saveSettings(false, "Daily", "04:00")
        compare(settingsSpy.count, 1)
        compare(settingsSpy.signalArguments[0][0], false)
        verify(settingsSpy.signalArguments[0][1].length > 0)
        SettingsController.loadSettings()
        compare(SettingsController.auto_update, true)
        let error = findChild(page, "settingsErrorMessage")
        verify(error !== null && error.visible && error.text.length > 0)
        compare(findChild(page, "autoUpdateSwitch").checked, true)

        saveSettings(false, "Daily", "02:00")
        compare(backgroundPortalFixture.lastAutostart, false)
        compare(SettingsController.auto_update, false)
    }
    function test_settings_page_does_not_write_during_initialization() {
        let root = window()
        saveSettings(true, "Weekly", "03:15")
        settingsSpy.clear()
        let requests = backgroundPortalFixture.requestCount
        root.switchToPage("qrc:/qml/pages/SettingsPage.qml")
        let page = root.pageStack.currentItem
        tryCompare(page, "readyForSaving", true)
        SettingsController.loadSettings()
        compare(settingsSpy.count, 0)
        compare(backgroundPortalFixture.requestCount, requests)
        compare(SettingsController.auto_update, true)
        compare(SettingsController.update_frequency, "Weekly")
        compare(SettingsController.update_time, "03:15")

        let frequency = findChild(page, "updateFrequencyCombo")
        let time = findChild(page, "updateTimeField")
        // Labels may be translated, but the persisted schedule is canonical.
        frequency.model = ["Diário", "Semanal"]
        frequency.currentIndex = 0
        compare(SettingsController.update_frequency, "Weekly")
        frequency.activated(0)
        compare(SettingsController.update_frequency, "Daily")
        time.text = "05:45"
        compare(SettingsController.update_time, "03:15")
        time.editingFinished()
        compare(SettingsController.update_time, "05:45")
        time.text = "25:99"
        time.editingFinished()
        compare(SettingsController.update_time, "05:45")
        saveSettings(false, "Daily", "02:00")
    }
    function test_remote_description_markup_is_displayed_without_resource_loading() {
        let root = window()
        root.pushAppDetail("org.kiosque.EncodedMarkup")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        let label = findChild(page, "appDescriptionText")
        compare(label.textFormat, Text.PlainText)
        compare(label.text, "<img src='file:///private'>")
    }
    function test_unknown_permissions_never_claim_isolation_or_allow_install() {
        let root = window()
        root.pushAppDetail("org.kiosque.UnknownPermissions")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.permissionsKnown, false)
        verify(findChild(page, "permissionsUnavailableMessage").visible)
        verify(!findChild(page, "installRemoveButton").enabled)
        verify(page.permissionItems[0].title.indexOf("unavailable") !== -1)
    }
    function test_detail_failure_retry_and_action_feedback() {
        let root = window()
        root.pushAppDetail("org.kiosque.Broken")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        verify(page.details.errorMessage.length > 0)
        let banner = findChild(page, "detailErrorMessage")
        let button = findChild(page, "installRemoveButton")
        verify(banner !== null && banner.visible && banner.text.length > 0)
        verify(button !== null && !button.enabled)
        findChild(page, "retryDetailsAction").trigger()
        tryCompare(page.details, "name", "Broken", 10000)
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.errorMessage, "")
        compare(banner.visible, false)
        verify(button.enabled)
        // Exercise the public error signal consumed by this real page.
        page.details.operationFinished(false, "Synthetic transaction failure")
        verify(banner.visible && banner.text.indexOf("Synthetic transaction failure") !== -1)
        page.details.operationFinished(true, "")
        compare(banner.visible, false)
    }
    function test_screenshot_popup_releases_scene_on_close() {
        let root = window()
        root.pushAppDetail("org.kiosque.Screenshots")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.screenshotUrls.length, 1)
        let popup = findChild(page, "largeScreenshotPopup")
        verify(popup !== null)
        popup.openWithIndex(0)
        tryCompare(popup, "visible", true)
        popup.close()
        tryCompare(popup, "visible", false)
        popup.openWithIndex(0)
        tryCompare(popup, "visible", true)
        popup.close()
        tryCompare(popup, "visible", false)
    }

    function test_review_mutations_refresh_the_actual_page() {
        let root = window()
        root.pushAppDetail("org.kiosque.Reviews")
        let page = root.pageStack.currentItem
        let details = page.details
        tryCompare(details, "reviewsLoading", false, 10000)
        compare(details.reviews.length, 1)
        compare(details.ratings.total, 1)
        reviewSpy.target = details
        reviewSpy.clear()
        details.submitReview(5, "New fixture review", "Synthetic review body", true)
        tryVerify(() => details.reviews.length === 2, 10000)
        compare(details.ratings.total, 2)
        compare(reviewSpy.signalArguments[0][0], true)
        let first = details.reviews[0].reviewId
        let second = details.reviews[1].reviewId
        details.upvoteReview(first)
        tryVerify(() => details.reviews.length > 0 && details.reviews[0].karmaUp === 3, 10000)
        details.downvoteReview(first)
        tryVerify(() => details.reviews.length > 0 && details.reviews[0].karmaDown === 1, 10000)
        details.removeReview(first)
        tryVerify(() => details.reviews.length === 1 && details.reviews[0].reviewId === second, 10000)
        compare(details.ratings.total, 1)
        reviewSpy.clear()
        details.submitReview(5, "Reject fixture review", "Synthetic rejection", true)
        tryCompare(reviewSpy, "count", 1, 10000)
        compare(reviewSpy.signalArguments[0][0], false)
        let banner = findChild(page, "detailErrorMessage")
        verify(banner.visible)
        verify(banner.text.indexOf("HTTP Error 503") !== -1)
        verify(banner.text.indexOf("fixture rejected review") === -1)
        compare(details.reviews.length, 1)
        details.dismissReview(second)
        tryVerify(() => details.reviews.length === 0, 10000)
        compare(details.ratings.total, 0)
        compare(banner.visible, false)
    }

    // The bridge models expose rowCount(parent) without a default argument.
    function rows(model) { return model.rowCount(model.index(-1, -1)) }
    // Dialog content is reparented into the popup's visual tree, which
    // findChild() does not reach; walk the item children instead.
    function findItem(item, name) {
        if (!item)
            return null
        if (item.objectName === name)
            return item
        for (let child of item.children) {
            let found = findItem(child, name)
            if (found)
                return found
        }
        return null
    }
    function press(dialog, name) {
        let button = findItem(dialog.contentItem, name)
        verify(button !== null, name)
        verify(button.visible && button.enabled, name)
        button.clicked()
    }
    function test_ref_file_requires_confirmation() {
        let root = window()
        let dialog = findChild(root, "refFileDialog")
        verify(dialog !== null)
        let before = backendFixture.operationCount()

        // Declining installs nothing, and a later confirm has nothing to install.
        root.openRefFile(fixtureEndpoint + "/files/fixture.flatpakref")
        tryCompare(dialog, "phase", "confirm", 10000)
        tryCompare(dialog, "opened", true, 5000)
        compare(dialog.appTitle, "Ref Fixture")
        compare(dialog.appName, "org.kiosque.RefFixture")
        compare(dialog.originUrl, "https://example.org/kiosque-fixture/")
        compare(backendFixture.operationCount(), before)
        press(dialog, "refCancelButton")
        tryCompare(dialog, "opened", false)
        StoreController.confirmRefFileInstall()
        wait(200)
        compare(backendFixture.operationCount(), before)

        // Accepting starts exactly one install with visible progress; it can be cancelled.
        backendFixture.holdOperations(true)
        installSpy.clear()
        root.openRefFile(fixtureEndpoint + "/files/fixture.flatpakref")
        tryCompare(dialog, "phase", "confirm", 10000)
        press(dialog, "refInstallButton")
        compare(dialog.phase, "installing")
        tryVerify(() => backendFixture.operationCount() === before + 1, 10000)
        let progress = findItem(dialog.contentItem, "refInstallProgress")
        verify(progress !== null && progress.visible)
        press(dialog, "refCancelInstallButton")
        tryCompare(installSpy, "count", 1, 10000)
        compare(installSpy.signalArguments[0][0], false)
        tryCompare(dialog, "opened", false)
        backendFixture.holdOperations(false)

        // A completed install is reported in the dialog.
        installSpy.clear()
        root.openRefFile(fixtureEndpoint + "/files/fixture.flatpakref")
        tryCompare(dialog, "phase", "confirm", 10000)
        press(dialog, "refInstallButton")
        tryCompare(dialog, "phase", "done", 10000)
        compare(installSpy.signalArguments[0][0], true)
        press(dialog, "refCloseButton")
        installSpy.clear()
        StoreController.uninstallApp("org.kiosque.RefFixture")
        tryCompare(installSpy, "count", 1, 10000)

        // Unreadable files are reported, never installed.
        let count = backendFixture.operationCount()
        root.openRefFile(fixtureEndpoint + "/files/missing.flatpakref")
        tryCompare(dialog, "phase", "failed", 10000)
        verify(dialog.errorText.length > 0)
        compare(backendFixture.operationCount(), count)
        press(dialog, "refCloseButton")
    }
    function test_repo_file_requires_confirmation_and_removal_reports() {
        let root = window()
        let dialog = findChild(root, "repoFileDialog")
        verify(dialog !== null)
        remoteAddedSpy.clear()
        root.openRepoFile(fixtureEndpoint + "/files/fixture.flatpakrepo")
        tryCompare(dialog, "phase", "confirm", 10000)
        compare(dialog.repoTitle, "Repo Fixture")
        compare(dialog.repoUrl, "https://example.org/kiosque-fixture/")
        compare(findChild(dialog.contentItem, "repoFileNameField").text, "kiosque-fixture")
        compare(remoteAddedSpy.count, 0)
        press(dialog, "repoAddButton")
        tryCompare(remoteAddedSpy, "count", 1, 10000)
        compare(remoteAddedSpy.signalArguments[0][0], true)
        tryVerify(() => rows(RepoModel) === 1, 10000)

        let status = findChild(root, "repositoryStatusDialog")
        verify(status !== null)
        status.close()
        remoteRemovedSpy.clear()
        RepoModel.removeRemote("kiosque-fixture")
        tryCompare(remoteRemovedSpy, "count", 1, 10000)
        compare(remoteRemovedSpy.signalArguments[0][0], true)
        tryVerify(() => status.opened, 10000)
        status.close()
        // A failed removal is shown to the user rather than ignored.
        remoteRemovedSpy.clear()
        RepoModel.removeRemote("kiosque-fixture")
        tryCompare(remoteRemovedSpy, "count", 1, 10000)
        compare(remoteRemovedSpy.signalArguments[0][0], false)
        tryVerify(() => status.opened, 10000)
        verify(status.messageText.indexOf("no such remote") !== -1)
        status.close()
    }
    function test_installed_page_uninstall_and_update_progress_and_cancel() {
        let root = window()
        root.currentSection = "installed"
        root.switchToPage("qrc:/qml/pages/InstalledPage.qml")
        let page = root.pageStack.currentItem
        let model = root.installedModel
        model.refresh()
        tryCompare(model, "loading", false, 10000)
        compare(rows(model), 1)

        backendFixture.holdOperations(true)
        model.uninstallApp(0)
        compare(model.uninstalling_app_id, "org.kiosque.Fixture")
        let bar = findChild(page, "uninstallProgressBar")
        verify(bar !== null)
        tryCompare(bar, "visible", true)
        verify(bar.indeterminate)
        let cancel = findChild(page, "cancelUninstallButton")
        verify(cancel !== null && cancel.visible)
        cancel.clicked()
        tryCompare(model, "uninstalling_app_id", "", 10000)
        // The model resets when the job ends, so look the delegate up again.
        let after = findChild(page, "uninstallProgressBar")
        verify(after === null || !after.visible)
        model.refresh()
        tryCompare(model, "loading", false, 10000)
        compare(rows(model), 1)

        verify(backendFixture.setUpdates(["org.kiosque.Fixture"]))
        model.checkForUpdates()
        tryCompare(model, "updates_available_count", 1, 10000)
        tryCompare(model, "checking_updates", false, 10000)
        model.updateSelectedApps()
        compare(model.updating, true)
        let label = findChild(page, "updateStatusLabel")
        verify(label !== null)
        compare(label.text, "Initializing update…")
        let cancelUpdate = findChild(page, "cancelUpdateButton")
        verify(cancelUpdate !== null && cancelUpdate.visible)
        cancelUpdate.clicked()
        tryCompare(model, "updating", false, 10000)
        backendFixture.holdOperations(false)
        verify(backendFixture.setUpdates([]))
        model.checkForUpdates()
        tryCompare(model, "updates_available_count", 0, 10000)
        tryCompare(model, "checking_updates", false, 10000)

        // An uninstall that is not cancelled completes and removes the row.
        model.uninstallApp(0)
        tryCompare(model, "uninstalling_app_id", "", 10000)
        tryVerify(() => rows(model) === 0, 10000)
        installSpy.clear()
        StoreController.installApp("org.kiosque.Fixture")
        tryCompare(installSpy, "count", 1, 10000)
        tryVerify(() => rows(model) === 1, 10000)
    }
}
