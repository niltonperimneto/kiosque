import QtQuick
import QtTest
import com.kiosque

TestCase {
    name: "PageRegressions"
    when: windowShown
    visible: true

    SignalSpy { id: settingsSpy; target: SettingsController; signalName: "settingsSaved" }
    SignalSpy { id: reviewSpy; signalName: "reviewActionFinished" }

    function init() {
        tryCompare(SettingsController, "saving_settings", false, 10000)
        backgroundPortalFixture.denyNext = false
        settingsSpy.clear()
        failOnWarning(/TypeError|ReferenceError/)
    }
    function cleanup() { reviewSpy.target = null }
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
        verify(banner.visible && banner.text.indexOf("fixture rejected review") !== -1)
        compare(details.reviews.length, 1)
        details.dismissReview(second)
        tryVerify(() => details.reviews.length === 0, 10000)
        compare(details.ratings.total, 0)
        compare(banner.visible, false)
    }
}
