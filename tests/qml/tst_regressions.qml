import QtQuick
import QtTest
import com.kiosque
import org.kde.kirigami as Kirigami

TestCase {
    name: "PageRegressions"
    when: windowShown
    visible: true

    Component { id: detailFactory; AppDetails {} }
    SignalSpy { id: settingsSpy; target: SettingsController; signalName: "settingsSaved" }
    SignalSpy { id: reviewSpy; signalName: "reviewActionFinished" }
    SignalSpy { id: installSpy; target: StoreController; signalName: "installFinished" }
    SignalSpy { id: remoteAddedSpy; target: RepoModel; signalName: "remoteAdded" }
    SignalSpy { id: remoteRemovedSpy; target: RepoModel; signalName: "remoteRemoved" }

    function init() {
        tryCompare(SettingsController, "saving_settings", false, 10000)
        BackgroundPortalFixture.denyNext = false
        settingsSpy.clear()
        failOnWarning(/TypeError|ReferenceError/)
    }
    function cleanup() {
        reviewSpy.target = null
        BackendFixture.holdOperations(false)
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
        let before = BackgroundPortalFixture.requestCount
        saveSettings(true, "Weekly", "03:15")
        compare(BackgroundPortalFixture.requestCount, before + 1)
        compare(BackgroundPortalFixture.lastAutostart, true)
        compare(BackgroundPortalFixture.lastCommand.join(" "), "kiosque-update --background")
        compare(SettingsController.auto_update, true)
        // Editing the schedule does not prompt again for background permission.
        saveSettings(true, "Daily", "04:00")
        compare(BackgroundPortalFixture.requestCount, before + 1)

        BackgroundPortalFixture.denyNext = true
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
        compare(BackgroundPortalFixture.lastAutostart, false)
        compare(SettingsController.auto_update, false)
    }
    function test_update_status_notices() {
        let root = window()
        root.navigate("settings")
        let page = root.pageStack.currentItem
        compare(page.objectName, "settingsPage")
        // Which notice applies, by situation.
        compare(page.updatesNotice(true, "missing", "", false), "missing")
        compare(page.updatesNotice(true, "outdated", "", false), "outdated")
        compare(page.updatesNotice(true, "unreachable", "", false), "unreachable")
        compare(page.updatesNotice(true, "ready", "", false), "")
        // A host timer that already updates Flatpaks makes the companion moot...
        compare(page.updatesNotice(true, "missing", "uupd.timer", false), "system")
        compare(page.updatesNotice(false, "", "flatpak-system-update.timer", false), "system")
        // ...unless Kiosque's own automatic updates are already on.
        compare(page.updatesNotice(false, "ready", "uupd.timer", true), "")
        // Native builds never warn about the companion.
        compare(page.updatesNotice(false, "missing", "", false), "")

        // This harness is native: the check settles without a notice, the
        // switch stays usable and there is no self-update note.
        tryVerify(() => SettingsController.companion_state !== "", 10000)
        compare(SettingsController.sandboxed, false)
        verify(["ready", "outdated", "missing", "unreachable"].indexOf(SettingsController.companion_state) !== -1)
        verify(page.canEnableAutoUpdates)
        verify(!findChild(page, "updatesNoticeMessage").visible)
        verify(!findChild(page, "selfUpdateNote").visible)
    }
    function test_settings_page_does_not_write_during_initialization() {
        let root = window()
        saveSettings(true, "Weekly", "03:15")
        settingsSpy.clear()
        let requests = BackgroundPortalFixture.requestCount
        root.switchToPage("qrc:/qml/pages/SettingsPage.qml")
        let page = root.pageStack.currentItem
        tryCompare(page, "readyForSaving", true)
        SettingsController.loadSettings()
        compare(settingsSpy.count, 0)
        compare(BackgroundPortalFixture.requestCount, requests)
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
    function test_detail_actions_follow_installed_state() {
        let root = window()
        root.pushAppDetail("org.kiosque.Fixture")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        tryCompare(page.details, "installed", true, 10000)
        let primary = findChild(page, "installRemoveButton")
        let remove = findChild(page, "removeButton")
        compare(primary.text, "Open")
        tryCompare(primary, "enabled", true, 10000)
        tryCompare(remove, "visible", true, 10000)
        // Removing asks for confirmation; dismissing it changes nothing.
        let before = BackendFixture.operationCount()
        remove.clicked()
        let confirm = findChild(page, "removeConfirmDialog")
        tryCompare(confirm, "opened", true, 5000)
        confirm.close()
        tryCompare(confirm, "opened", false)
        wait(200)
        compare(BackendFixture.operationCount(), before)
        compare(page.details.installed, true)

        root.pushAppDetail("org.kiosque.Reviews")
        page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.installed, false)
        compare(findChild(page, "installRemoveButton").text, "Install")
        verify(!findChild(page, "removeButton").visible)
    }
    function test_permission_summary_levels() {
        let root = window()
        root.pushAppDetail("org.kiosque.Fixture")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.computePermissionLevel({}, true), "sandboxed")
        compare(page.computePermissionLevel({ filesystems: ["host"] }, true), "full")
        compare(page.computePermissionLevel({ sockets: ["wayland", "network", "pulseaudio"], devices: ["dri"] }, true), "sandboxed")
        compare(page.computePermissionLevel({ filesystems: ["xdg-download:ro"] }, true), "limited")
        compare(page.computePermissionLevel({ sockets: ["x11"] }, true), "limited")
        compare(page.computePermissionLevel({ devices: ["all"] }, true), "full")
        compare(page.computePermissionLevel({ "session-bus": { talk: ["org.freedesktop.Flatpak"] } }, true), "full")
        compare(page.computePermissionLevel({ "session-bus": { talk: ["org.freedesktop.Notifications"] } }, true), "sandboxed")
        // Unverifiable permissions are never summarised as safe.
        compare(page.computePermissionLevel({}, false), "unknown")
        root.pushAppDetail("org.kiosque.UnknownPermissions")
        page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.permissionLevel, "unknown")
        compare(findChild(page, "permissionChip").text, "Permissions unknown")
    }
    function test_release_notes_and_links() {
        let root = window()
        root.pushAppDetail("org.kiosque.Screenshots")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.latestRelease.version, "2.0")
        let section = findChild(page, "whatsNewSection")
        verify(section.visible)
        let notes = findChild(page, "releaseNotesText")
        compare(notes.textFormat, Text.PlainText)
        verify(notes.text.indexOf("Faster startup.") !== -1, notes.text)
        verify(notes.text.indexOf("New gallery") !== -1, notes.text)
        verify(notes.text.indexOf("<") === -1, notes.text)
        // Only present, secure links are listed.
        compare(page.linkEntries.map(l => l.key).join(","), "homepage,bugtracker")
        verify(findChild(page, "linksCard").visible)

        root.pushAppDetail("org.kiosque.Fixture")
        page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        verify(!findChild(page, "whatsNewSection").visible)
        compare(page.linkEntries.length, 0)
    }
    function test_type_to_search() {
        const root = window()
        root.requestActivate()
        tryCompare(root, "active", true)
        const home = root.pageStack.currentItem
        const popularModel = home.popularModel
        const homeScroll = home.flickable.contentY
        home.forceActiveFocus()
        keyClick(Qt.Key_E)
        tryCompare(root, "currentSection", "search")

        const page = root.pageStack.currentItem
        tryVerify(() => page.searchField && page.searchField.activeFocus)
        compare(page.searchField.text.toLowerCase(), "e")
        for (const key of [Qt.Key_D, Qt.Key_I, Qt.Key_T, Qt.Key_O, Qt.Key_R]) {
            keyClick(key)
        }
        tryCompare(page, "activeQuery", "editor")
        tryVerify(() => !page.resultsModel.loading && rows(page.resultsModel) === 1, 10000)

        keyClick(Qt.Key_A, Qt.ControlModifier)
        keyClick(Qt.Key_Backspace)
        tryCompare(root.pageStack, "currentItem", home)
        compare(root.currentSection, "shopfront")
        compare(home.popularModel, popularModel)
        compare(home.flickable.contentY, homeScroll)
    }
    function test_clearing_search_returns_to_category() {
        const root = window()
        root.currentSection = "categories"
        root.currentCategory = "Game"
        const category = root.pageStack.push("qrc:/qml/pages/CategoryAppListPage.qml",
                                            {categoryId: "Game", categoryName: "Games"})
        const categoryModel = category.categoryModel
        const scrollPosition = category.flickable.contentY
        const scrollDuration = root.pageStack.columnView.scrollDuration
        root.openSearch("slow")
        const search = root.pageStack.currentItem
        compare(root.pageStack.get(root.pageStack.depth - 2), category)
        tryVerify(() => search.searchField !== null)
        search.searchField.text = ""
        tryCompare(root.pageStack, "currentItem", category)
        compare(root.currentSection, "categories")
        compare(root.currentCategory, "Game")
        compare(category.categoryId, "Game")
        compare(category.categoryModel, categoryModel)
        compare(category.flickable.contentY, scrollPosition)
        compare(root.pageStack.columnView.scrollDuration, scrollDuration)
    }

    function test_search_page_states() {
        let root = window()
        // The storefront field hands its query to the Search page.
        let field = findChild(root.pageStack.currentItem, "storefrontSearchField")
        verify(field !== null)
        field.text = "Editor"
        field.accepted()
        tryVerify(() => root.pageStack.currentItem.objectName === "searchPage", 5000)
        let page = root.pageStack.currentItem
        compare(root.currentSection, "search")
        compare(field.text, "")
        tryCompare(page.resultsModel, "loading", false, 10000)
        tryVerify(() => rows(page.resultsModel) === 1, 10000)
        compare(page.resultsModel.error_message, "")

        root.openSearch("empty")
        page = root.pageStack.currentItem
        tryCompare(page.resultsModel, "loading", false, 10000)
        tryVerify(() => findChild(page, "searchEmptyMessage").visible, 10000)
        verify(!findChild(page, "searchErrorMessage").visible)

        // A failed search is reported as such, not as "no results".
        root.openSearch("error")
        page = root.pageStack.currentItem
        tryCompare(page.resultsModel, "loading", false, 10000)
        tryVerify(() => findChild(page, "searchErrorMessage").visible, 10000)
        verify(!findChild(page, "searchEmptyMessage").visible)
        verify(page.resultsModel.error_message.indexOf("503") !== -1)

        // Without a query the page invites a search instead.
        root.navigate("shopfront")
        root.navigate("search")
        page = root.pageStack.currentItem
        verify(findChild(page, "searchPromptMessage").visible)
    }
    function test_catalog_apps_come_from_their_repository() {
        let root = window()
        // The local catalog answers first; Flathub results only supplement it.
        root.openSearch("Atlas")
        let page = root.pageStack.currentItem
        tryCompare(page.resultsModel, "loading", false, 10000)
        tryVerify(() => rows(page.resultsModel) === 2, 10000)
        compare(page.resultsModel.data(page.resultsModel.index(0, 0), 259), "org.example.Atlas")

        // Details, permissions and related apps come from the origin repository.
        root.pushAppDetail("org.example.Atlas")
        page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.errorMessage, "")
        compare(page.details.name, "Atlas")
        compare(page.details.description, "Browse offline maps.")
        compare(page.details.origin.name, "fixture-repo")
        let chip = findChild(page, "originChip")
        verify(chip.visible)
        compare(chip.text, "From Fixture Repository (user)")
        verify(page.details.permissionsKnown)
        compare(page.permissionLevel, "sandboxed")
        compare(page.details.latestRelease.version, "3.1")
        verify(findChild(page, "releaseNotesText").text.indexOf("Sharper coastlines.") !== -1)
        compare(page.details.developerApps.map(a => a.appId).join(","), "org.example.Globe")
        // An app listed under "More by" is not repeated under "Similar".
        compare(page.details.similarApps.length, 0)

        // Installing uses the repository the page showed.
        let install = findChild(page, "installRemoveButton")
        tryVerify(() => install.enabled, 5000, JSON.stringify({ loading: page.details.loading, busy: page.details.busy,
            installed: page.details.installed, known: page.details.permissionsKnown, error: page.details.errorMessage,
            visible: install.visible, text: install.text }))
        install.clicked()
        tryCompare(page.details, "installed", true, 10000)
        compare(BackendFixture.lastInstallRemote(), "fixture-repo")
        page.details.uninstall()
        tryCompare(page.details, "installed", false, 10000)

        // Without repository metadata, permissions stay unknown and Install
        // is not offered, even though the app itself is known.
        root.pushAppDetail("org.example.Globe")
        page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.name, "Globe")
        compare(page.permissionLevel, "unknown")
        verify(!findChild(page, "installRemoveButton").enabled)
        let banner = findChild(page, "permissionsUnavailableMessage")
        verify(banner.visible)
        verify(banner.text.indexOf("Fixture Repository") !== -1, banner.text)

        // Apps known only from the Flathub web API show no origin.
        root.pushAppDetail("org.kiosque.Fixture")
        page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        verify(!findChild(page, "originChip").visible)
    }
    function test_navigation_surfaces_follow_window_width() {
        let root = window()
        let tabs = findChild(root, "navigationTabBar")
        verify(tabs !== null)
        root.width = 60 * Kirigami.Units.gridUnit
        tryCompare(root, "compact", false)
        verify(!tabs.visible)
        let sidebar = root.globalDrawer
        tryCompare(sidebar, "visible", true)
        sidebar.collapsed = true
        root.width = 25 * Kirigami.Units.gridUnit
        tryCompare(root, "compact", true)
        verify(tabs.visible)
        tryCompare(sidebar, "visible", false)
        tryCompare(root.contentItem, "width", root.width)
        root.width = 60 * Kirigami.Units.gridUnit
        tryCompare(root, "compact", false)
        verify(!tabs.visible)
        tryCompare(sidebar, "visible", true)
        tryCompare(sidebar, "position", 1)
        verify(sidebar.collapsed)
        tryCompare(root.contentItem, "width", root.width - sidebar.width)
        // Sections replace each other in a single column with a back button.
        root.navigate("installed")
        compare(root.currentSection, "installed")
        root.navigate("updates")
        compare(root.pageStack.depth, 2)
        compare(root.pageStack.currentItem.objectName, "updatesPage")
        compare(root.title, "Updates — Kiosque")
        root.navigate("shopfront")
        compare(root.pageStack.depth, 1)
        compare(root.title, "Kiosque")
    }
    function test_tasks_dialog_lists_and_cancels_background_work() {
        let root = window()
        let jobs = root.jobsModel
        jobs.clearFinished()
        let tasksAction = root.navigationActions[6]
        compare(tasksAction.objectName, "navTasks")
        verify(!tasksAction.visible)
        BackendFixture.holdOperations(true)
        let detail = createTemporaryObject(detailFactory, this, {appId: "org.kiosque.TasksFixture"})
        detail.install()
        tryCompare(jobs, "active_count", 1, 10000)
        verify(tasksAction.visible)
        compare(tasksAction.text, "Tasks (1)")
        root.openTasks()
        let dialog = findChild(root, "tasksDialog")
        tryCompare(dialog, "opened", true, 5000)
        let list = findItem(dialog.contentItem, "tasksList")
        verify(list !== null)
        tryCompare(list, "count", 1, 5000)
        let id = jobs.data(jobs.index(0, 0), 256)
        let cancel = findItem(list, "cancelTask_" + id)
        verify(cancel !== null && cancel.visible)
        cancel.clicked()
        tryCompare(jobs, "active_count", 0, 10000)
        compare(jobs.finished_count, 1)
        BackendFixture.holdOperations(false)
        dialog.close()
        jobs.clearFinished()
        compare(rows(jobs), 0)
        verify(!tasksAction.visible)
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

    function test_screenshot_viewer_retains_source_resolution() {
        let root = window()
        root.pushAppDetail("org.kiosque.Screenshots")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        let popup = findChild(page, "largeScreenshotPopup")
        popup.openWithIndex(0)
        tryCompare(popup, "visible", true)
        tryVerify(() => findItem(popup.contentItem, "largeScreenshotImage") !== null, 10000)
        let image = findItem(popup.contentItem, "largeScreenshotImage")
        tryCompare(image, "status", Image.Ready, 10000)
        compare(image.implicitWidth, 1600)
        compare(image.implicitHeight, 900)
        image.parent.zoomScale = 2
        compare(image.implicitWidth, 1600, "zoom must keep the full screenshot pixels")
        popup.close()
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
        // Reload clears reviews before ratings arrive. Wait for completed data.
        tryVerify(() => !details.reviewsLoading
                  && details.reviews.length === 0 && details.ratings.total === 0, 10000)
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
        let before = BackendFixture.operationCount()

        // Declining installs nothing, and a later confirm has nothing to install.
        root.openRefFile(TestSupport.endpoint + "/files/fixture.flatpakref")
        tryCompare(dialog, "phase", "confirm", 10000)
        tryCompare(dialog, "opened", true, 5000)
        compare(dialog.appTitle, "Ref Fixture")
        compare(dialog.appName, "org.kiosque.RefFixture")
        compare(dialog.originUrl, "https://example.org/kiosque-fixture/")
        compare(BackendFixture.operationCount(), before)
        press(dialog, "refCancelButton")
        tryCompare(dialog, "opened", false)
        StoreController.confirmRefFileInstall()
        wait(200)
        compare(BackendFixture.operationCount(), before)

        // Accepting starts exactly one install with visible progress; it can be cancelled.
        BackendFixture.holdOperations(true)
        installSpy.clear()
        root.openRefFile(TestSupport.endpoint + "/files/fixture.flatpakref")
        tryCompare(dialog, "phase", "confirm", 10000)
        press(dialog, "refInstallButton")
        compare(dialog.phase, "installing")
        StoreController.confirmRefFileInstall()
        tryVerify(() => BackendFixture.operationCount() === before + 1, 10000)
        let progress = findItem(dialog.contentItem, "refInstallProgress")
        verify(progress !== null && progress.visible)
        press(dialog, "refCancelInstallButton")
        tryCompare(installSpy, "count", 1, 10000)
        compare(installSpy.signalArguments[0][0], false)
        tryCompare(dialog, "opened", false)
        BackendFixture.holdOperations(false)

        // A completed install is reported in the dialog.
        installSpy.clear()
        root.openRefFile(TestSupport.endpoint + "/files/fixture.flatpakref")
        tryCompare(dialog, "phase", "confirm", 10000)
        press(dialog, "refInstallButton")
        tryCompare(dialog, "phase", "done", 10000)
        compare(installSpy.signalArguments[0][0], true)
        press(dialog, "refCloseButton")
        let detail = createTemporaryObject(detailFactory, this, {appId: "org.kiosque.RefFixture"})
        tryCompare(detail, "installed", true, 10000)
        detail.uninstall()
        tryCompare(detail, "installed", false, 10000)

        // Unreadable files are reported, never installed.
        let count = BackendFixture.operationCount()
        root.openRefFile(TestSupport.endpoint + "/files/missing.flatpakref")
        tryCompare(dialog, "phase", "failed", 10000)
        verify(dialog.errorText.length > 0)
        compare(BackendFixture.operationCount(), count)
        press(dialog, "refCloseButton")
    }
    function test_repo_file_requires_confirmation_and_removal_reports() {
        let root = window()
        let dialog = findChild(root, "repoFileDialog")
        verify(dialog !== null)
        remoteAddedSpy.clear()
        root.openRepoFile(TestSupport.endpoint + "/files/fixture.flatpakrepo")
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
    function test_manual_repository_is_inspected_before_adding() {
        let root = window()
        root.switchToPage("qrc:/qml/pages/SettingsPage.qml")
        let page = root.pageStack.currentItem
        let dialog = findChild(root, "repoFileDialog")
        verify(dialog !== null)
        remoteAddedSpy.clear()
        let add = findChild(page, "addRepositoryButton")
        verify(add !== null && add.visible)
        add.clicked()
        tryCompare(dialog, "opened", true, 5000)
        compare(dialog.phase, "enter")
        // Nothing can be inspected until an address is entered.
        verify(!findItem(dialog.contentItem, "repoContinueButton").enabled)
        findItem(dialog.contentItem, "repoSourceField").text = TestSupport.endpoint + "/files/fixture.flatpakrepo"
        findItem(dialog.contentItem, "repoManualNameField").text = "manual-fixture"
        press(dialog, "repoContinueButton")
        tryCompare(dialog, "phase", "confirm", 10000)
        // The descriptor's origin is shown and the requested name is kept.
        compare(dialog.repoUrl, "https://example.org/kiosque-fixture/")
        compare(findItem(dialog.contentItem, "repoFileNameField").text, "manual-fixture")
        compare(remoteAddedSpy.count, 0)
        press(dialog, "repoCancelButton")
        tryCompare(dialog, "opened", false)
        wait(200)
        compare(remoteAddedSpy.count, 0)

        // An unreadable address is reported instead of being added.
        add.clicked()
        tryCompare(dialog, "opened", true, 5000)
        findItem(dialog.contentItem, "repoSourceField").text = TestSupport.endpoint + "/files/missing.flatpakrepo"
        press(dialog, "repoContinueButton")
        tryCompare(dialog, "phase", "failed", 10000)
        verify(dialog.errorText.length > 0)
        press(dialog, "repoCancelButton")
        compare(remoteAddedSpy.count, 0)
    }
    function test_installed_page_uninstall_and_update_progress_and_cancel() {
        let root = window()
        root.navigate("installed")
        let page = root.pageStack.currentItem
        let model = root.installedModel
        model.refresh()
        tryCompare(model, "loading", false, 10000)
        compare(rows(model), 1)

        BackendFixture.holdOperations(true)
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

        verify(BackendFixture.setUpdates(["org.kiosque.Fixture"]))
        model.checkForUpdates()
        tryCompare(model, "updates_available_count", 1, 10000)
        tryCompare(model, "checking_updates", false, 10000)
        // The Installed page links to the Updates page, which owns updating.
        let banner = findChild(page, "updatesAvailableMessage")
        verify(banner !== null)
        tryCompare(banner, "visible", true)
        root.navigate("updates")
        let updates = root.pageStack.currentItem
        compare(updates.objectName, "updatesPage")
        compare(rows(model), 1)
        model.updateSelectedApps()
        compare(model.updating, true)
        let label = findChild(updates, "updateStatusLabel")
        verify(label !== null)
        compare(label.text, "Initializing update…")
        let cancelUpdate = findChild(updates, "cancelUpdateButton")
        verify(cancelUpdate !== null && cancelUpdate.visible)
        cancelUpdate.clicked()
        tryCompare(model, "updating", false, 10000)
        BackendFixture.holdOperations(false)
        verify(BackendFixture.setUpdates([]))
        model.checkForUpdates()
        tryCompare(model, "updates_available_count", 0, 10000)
        tryCompare(model, "checking_updates", false, 10000)
        let upToDate = findChild(updates, "upToDateMessage")
        tryCompare(upToDate, "visible", true)
        compare(upToDate.text, "Up to Date")
        model.updates_error = "Synthetic backend failure"
        let failure = findChild(updates, "updateCheckError")
        verify(failure !== null && failure.visible)
        compare(failure.text, model.updates_error)
        verify(upToDate.text !== "Up to Date")
        verify(upToDate.helpfulAction !== null)
        model.updates_error = ""
        // Leaving the Updates page restores the full installed list.
        root.navigate("installed")
        page = root.pageStack.currentItem
        tryVerify(() => rows(model) === 1, 10000)

        // An uninstall that is not cancelled completes and removes the row.
        model.uninstallApp(0)
        tryCompare(model, "uninstalling_app_id", "", 10000)
        tryVerify(() => rows(model) === 0, 10000)
        let detail = createTemporaryObject(detailFactory, this, {appId: "org.kiosque.Fixture"})
        detail.install()
        tryCompare(detail, "installed", true, 10000)
        tryVerify(() => rows(model) === 1, 10000)
    }
    function test_detail_page_shows_what_an_install_downloads() {
        verify(BackendFixture.seedRuntimes())
        let root = window()
        root.pushAppDetail("org.example.Atlas")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        compare(page.details.installed, false)
        compare(page.details.runtime, "runtime/org.gnome.Platform/x86_64/48")
        let chip = findChild(page, "runtimeChip")
        verify(chip !== null && chip.visible)
        compare(chip.text, "Runtime: GNOME Platform 48")
        let label = findChild(page, "installPlanLabel")
        tryVerify(() => label.text.indexOf("Downloads 412.0 MB, uses 1.2 GB on disk.") !== -1, 10000, label.text)
        verify(label.text.indexOf("Also installs GNOME Platform 48 (400.0 MB)") !== -1, label.text)
        verify(findChild(page, "installPlanRow").visible)
        verify(!findChild(page, "runtimeEolMessage").visible)
    }
    function test_remove_dialog_offers_runtimes_left_unused() {
        verify(BackendFixture.seedRuntimes())
        let root = window()
        root.pushAppDetail("org.kiosque.Fixture")
        let page = root.pageStack.currentItem
        tryCompare(page.details, "loading", false, 10000)
        tryCompare(page.details, "installed", true, 10000)
        let remove = findChild(page, "removeButton")
        tryCompare(remove, "visible", true, 10000)
        remove.clicked()
        let dialog = findChild(page, "removeConfirmDialog")
        tryCompare(dialog, "opened", true, 5000)
        tryCompare(dialog.planner, "loading", false, 10000)
        compare(dialog.leftovers.length, 1)
        let kde = "runtime/org.kde.Platform/x86_64/6.9"
        let box = findItem(dialog.contentItem, "leftoverRuntime_" + kde)
        verify(box !== null && box.visible)
        // Offered pre-selected, with the space it frees.
        verify(box.checked)
        compare(box.text, "KDE Platform 6.9 (900.0 MB)")
        verify(findItem(dialog.contentItem, "unusedRuntimesLabel").visible)

        // The runtime goes only after the application is removed.
        let before = BackendFixture.operationCount()
        dialog.customFooterActions[0].trigger()
        tryCompare(page.details, "installed", false, 10000)
        tryVerify(() => BackendFixture.operationCount() === before + 2, 10000)
        compare(BackendFixture.lastRemovedRuntimes(), kde)

        installSpy.clear()
        StoreController.installApp("org.kiosque.Fixture")
        tryCompare(installSpy, "count", 1, 10000)
        tryCompare(page.details, "installed", true, 10000)

        // Unticking keeps the runtime.
        verify(BackendFixture.seedRuntimes())
        remove = findChild(page, "removeButton")
        tryCompare(remove, "visible", true, 10000)
        remove.clicked()
        tryCompare(dialog, "opened", true, 5000)
        tryCompare(dialog.planner, "loading", false, 10000)
        tryVerify(() => findItem(dialog.contentItem, "leftoverRuntime_" + kde) !== null, 5000)
        findItem(dialog.contentItem, "leftoverRuntime_" + kde).checked = false
        before = BackendFixture.operationCount()
        dialog.customFooterActions[0].trigger()
        tryCompare(page.details, "installed", false, 10000)
        wait(300)
        compare(BackendFixture.operationCount(), before + 1)

        installSpy.clear()
        StoreController.installApp("org.kiosque.Fixture")
        tryCompare(installSpy, "count", 1, 10000)
    }
    function test_installed_page_lists_runtimes_and_removes_unused() {
        verify(BackendFixture.seedRuntimes())
        let root = window()
        root.navigate("installed")
        let page = root.pageStack.currentItem
        tryCompare(page.runtimes, "loading", false, 10000)
        tryCompare(page.runtimes, "count", 2, 10000)
        tryVerify(() => {
            let section = findChild(page, "runtimesSection")
            return section !== null && section.visible
        }, 10000)
        let kde = findChild(page, "runtimeRow_runtime/org.kde.Platform/x86_64/6.9")
        verify(kde !== null)
        let usage = findChild(kde, "runtimeUsage").text
        verify(usage.indexOf("Used by ") === 0 && usage.indexOf("900.0 MB") !== -1, usage)
        let unused = findChild(page, "runtimeRow_runtime/org.freedesktop.Platform/x86_64/24.08")
        verify(unused !== null)
        compare(findChild(unused, "runtimeUsage").text, "Not used by any application  ·  900.0 MB")
        tryCompare(findChild(page, "unusedRuntimesMessage"), "visible", true)

        // Removing unused runtimes asks first and names them.
        let action = page.actions.find(a => a.objectName === "removeUnusedRuntimesAction")
        verify(action.enabled)
        action.trigger()
        let dialog = findChild(page, "removeUnusedRuntimesDialog")
        tryCompare(dialog, "opened", true, 5000)
        verify(dialog.subtitle.indexOf("Freedesktop Platform 24.08 (900.0 MB)") !== -1, dialog.subtitle)
        dialog.customFooterActions[0].trigger()
        tryVerify(() => BackendFixture.lastRemovedRuntimes() === "runtime/org.freedesktop.Platform/x86_64/24.08", 10000)
        tryCompare(page.runtimes, "count", 1, 10000)
        tryVerify(() => page.runtimes.unused.length === 0, 10000)
        verify(!action.enabled)

        // Uninstalling from this page confirms too and offers the leftovers.
        let remove = findChild(page, "removeConfirmDialog")
        remove.openFor("org.kiosque.Fixture", "Fixture")
        tryCompare(remove, "opened", true, 5000)
        tryCompare(remove.planner, "loading", false, 10000)
        compare(remove.leftovers.length, 1)
        remove.customFooterActions[0].trigger()
        tryVerify(() => BackendFixture.lastRemovedRuntimes() === "runtime/org.kde.Platform/x86_64/6.9", 10000)
        tryCompare(page.runtimes, "count", 0, 10000)

        installSpy.clear()
        StoreController.installApp("org.kiosque.Fixture")
        tryCompare(installSpy, "count", 1, 10000)
    }
}
