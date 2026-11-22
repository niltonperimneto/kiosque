// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors
//
// kiosque-update: the Kiosque daemon shell. The transaction queue, update
// checks, automatic updates and the D-Bus services (org.kiosque.Kiosque.Daemon
// and the KRunner runner) live in Rust (rust/core/src/daemon). This file only
// provides the tray icon and desktop notifications.

#include <cstring>
#include <mutex>

#include <QAction>
#include <QApplication>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusMessage>
#include <QFileInfo>
#include <QIcon>
#include <QPalette>
#include <QMenu>
#include <QMetaObject>
#include <QProcess>
#include <QSystemTrayIcon>
#include <QTextStream>
#include <KLocalizedString>

#include "localization.h"

// Exported by kiosque-core (rust/core/src/daemon/mod.rs), linked via libkiosque.a.
extern "C" {
typedef void (*KiosqueDaemonCallback)(int event, int count, const char *text, void *ctx);
bool kiosque_daemon_start(bool background, KiosqueDaemonCallback cb, void *ctx);
void kiosque_daemon_check_updates();
void kiosque_daemon_update_all();
bool kiosque_daemon_busy();
int kiosque_run_update_once();
}

namespace {

// Keep in sync with EVENT_* in rust/core/src/daemon/mod.rs.
constexpr int EventUpdates = 1;
constexpr int EventJobFinished = 2;
constexpr int EventIdleExit = 3;
constexpr int EventAutoUpdate = 4;

class UpdateDaemon;

// The Rust side keeps the callback for the lifetime of the process and has
// no unregister entry point, so the callback never dereferences its context
// pointer. Instead it posts to the currently registered daemon object, which
// detaches itself under this lock before it is destroyed. The mutex is
// intentionally leaked so worker threads can still lock it during static
// destruction at process exit.
std::mutex &daemonLock()
{
    static auto *lock = new std::mutex;
    return *lock;
}
UpdateDaemon *g_daemon = nullptr;

void daemonEventCallback(int event, int count, const char *text, void *ctx);

// Monochrome Kiosque logo for the tray, as KDE uses symbolic icons there. The
// installed theme icon lets Plasma recolour it for the panel; without it, pick
// the black or white logo for the current palette (grid-aligned at 16 px).
QIcon trayIcon()
{
    const QString themed = QStringLiteral("org.kiosque.Kiosque-symbolic");
    if (QIcon::hasThemeIcon(themed)) {
        return QIcon::fromTheme(themed);
    }
    const bool dark = QApplication::palette().color(QPalette::Window).lightness() < 128;
    const QString colour = dark ? QStringLiteral("white") : QStringLiteral("black");
    QIcon icon;
    icon.addFile(QStringLiteral(":/qml/images/logo-symbolic-16-%1.svg").arg(colour), QSize(16, 16));
    icon.addFile(QStringLiteral(":/qml/images/logo-symbolic-%1.svg").arg(colour), QSize(32, 32));
    return icon;
}

class UpdateDaemon : public QObject
{
    Q_OBJECT
public:
    explicit UpdateDaemon(bool background, QObject *parent = nullptr)
        : QObject(parent)
        , m_background(background)
    {
        m_trayIcon = new QSystemTrayIcon(trayIcon(), this);

        m_menu = new QMenu();
        m_actionOpen = m_menu->addAction(QIcon::fromTheme(QStringLiteral("org.kiosque.Kiosque")), i18n("Open Store"));
        m_actionCheck = m_menu->addAction(QIcon::fromTheme(QStringLiteral("view-refresh")), i18n("Check for Updates"));
        m_actionUpdateAll = m_menu->addAction(QIcon::fromTheme(QStringLiteral("system-software-update")), i18n("Update All"));
        m_menu->addSeparator();
        m_actionQuit = m_menu->addAction(QIcon::fromTheme(QStringLiteral("application-exit")), i18n("Quit"));
        m_actionUpdateAll->setEnabled(false);
        m_trayIcon->setContextMenu(m_menu);

        connect(m_actionOpen, &QAction::triggered, this, &UpdateDaemon::openStore);
        connect(m_actionCheck, &QAction::triggered, this, &UpdateDaemon::checkUpdates);
        connect(m_actionUpdateAll, &QAction::triggered, this, &UpdateDaemon::updateAll);
        connect(m_actionQuit, &QAction::triggered, qApp, &QCoreApplication::quit);
        connect(m_trayIcon, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
            if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
                openStore();
            }
        });

        QDBusConnection::sessionBus().connect(QStringLiteral("org.freedesktop.Notifications"),
                                              QStringLiteral("/org/freedesktop/Notifications"),
                                              QStringLiteral("org.freedesktop.Notifications"),
                                              QStringLiteral("ActionInvoked"),
                                              this,
                                              SLOT(onNotificationActionInvoked(uint, QString)));

        std::lock_guard<std::mutex> guard(daemonLock());
        g_daemon = this;
    }

    ~UpdateDaemon() override
    {
        {
            std::lock_guard<std::mutex> guard(daemonLock());
            if (g_daemon == this) {
                g_daemon = nullptr;
            }
        }
        // Queued events still pending for this object are discarded by ~QObject.
        delete m_menu;
    }

    // Claims the bus name and starts the Rust services. False if another
    // instance already owns org.kiosque.Kiosque.Daemon or the bus is unreachable.
    bool start()
    {
        // The update check runs inside the daemon scheduler immediately.
        return kiosque_daemon_start(m_background, &daemonEventCallback, nullptr);
    }

    // Called on the GUI thread (queued from the Rust worker threads).
    void handleEvent(int event, int count, const QString &text)
    {
        switch (event) {
        case EventUpdates:
            onUpdates(count);
            break;
        case EventJobFinished:
            onJobFinished(count != 0, text);
            break;
        case EventAutoUpdate:
            sendNotification(i18n("Updating Applications"),
                             i18n("Kiosque is installing updates automatically in the background."),
                             false);
            refreshActions();
            break;
        case EventIdleExit:
            if (!m_background && !kiosque_daemon_busy()) {
                QCoreApplication::quit();
            }
            break;
        default:
            break;
        }
    }

public Q_SLOTS:
    void openStore()
    {
        // Launch the sibling GUI binary (same prefix / sandbox), falling back to PATH.
        const QString sibling = QCoreApplication::applicationDirPath() + QStringLiteral("/kiosque");
        const QString program = QFileInfo(sibling).isExecutable() ? sibling : QStringLiteral("kiosque");
        if (!QProcess::startDetached(program, QStringList())) {
            qWarning("kiosque-update: cannot launch the store");
        }
    }

    void checkUpdates()
    {
        kiosque_daemon_check_updates();
    }

    void updateAll()
    {
        if (m_updateCount <= 0 || kiosque_daemon_busy()) {
            return;
        }
        kiosque_daemon_update_all();
        sendNotification(i18n("Updating Applications"),
                         i18n("Kiosque is installing updates in the background..."),
                         false);
        refreshActions();
    }

    void onNotificationActionInvoked(uint notificationId, const QString &actionKey)
    {
        if (notificationId == m_lastNotificationId && actionKey == QLatin1String("open")) {
            openStore();
        }
    }

private:
    void onUpdates(int count)
    {
        const int previous = m_updateCount;
        m_updateCount = count;
        refreshActions();
        if (count <= 0) {
            m_trayIcon->hide();
            return;
        }
        m_trayIcon->setToolTip(i18np("Kiosque: %1 update available", "Kiosque: %1 updates available", count));
        m_trayIcon->show();
        // Only notify when the number of pending updates grows, not on every re-check.
        if (count > previous) {
            sendNotification(i18n("Software Updates Available"),
                             i18np("%1 application update is available. Click to open Kiosque.",
                                   "%1 application updates are available. Click to open Kiosque.",
                                   count),
                             true);
        }
    }

    void onJobFinished(bool success, const QString &title)
    {
        refreshActions();
        if (success) {
            sendNotification(i18n("Operation Completed"), title, false);
        } else {
            sendNotification(i18n("Operation Failed"), title, true);
        }
    }

    void refreshActions()
    {
        const bool busy = kiosque_daemon_busy();
        m_actionUpdateAll->setEnabled(!busy && m_updateCount > 0);
    }

    void sendNotification(const QString &title, const QString &body, bool hasAction)
    {
        QDBusInterface notify(QStringLiteral("org.freedesktop.Notifications"),
                              QStringLiteral("/org/freedesktop/Notifications"),
                              QStringLiteral("org.freedesktop.Notifications"),
                              QDBusConnection::sessionBus());
        if (!notify.isValid()) {
            qDebug("kiosque-update: notification service not available");
            return;
        }
        QStringList actions;
        if (hasAction) {
            actions << QStringLiteral("open") << i18n("Open Kiosque");
        }
        QVariantMap hints;
        hints.insert(QStringLiteral("desktop-entry"), QStringLiteral("org.kiosque.Kiosque"));
        const QDBusMessage reply = notify.call(QStringLiteral("Notify"),
                                               QStringLiteral("Kiosque"),
                                               static_cast<quint32>(m_lastNotificationId),
                                               QStringLiteral("system-software-update"),
                                               title,
                                               body,
                                               actions,
                                               hints,
                                               static_cast<qint32>(10000));
        if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
            m_lastNotificationId = reply.arguments().at(0).toUInt();
        }
    }

    bool m_background;
    QSystemTrayIcon *m_trayIcon;
    QMenu *m_menu;
    QAction *m_actionOpen;
    QAction *m_actionCheck;
    QAction *m_actionUpdateAll;
    QAction *m_actionQuit;
    int m_updateCount = 0;
    uint m_lastNotificationId = 0;
};

// Invoked on Rust worker threads; `text` is only valid during the call.
void daemonEventCallback(int event, int count, const char *text, void * /*unused*/)
{
    const QString message = text ? QString::fromUtf8(text) : QString();
    std::lock_guard<std::mutex> guard(daemonLock());
    if (UpdateDaemon *daemon = g_daemon) {
        QMetaObject::invokeMethod(
            daemon,
            [daemon, event, count, message]() { daemon->handleEvent(event, count, message); },
            Qt::QueuedConnection);
    }
}

void printUsage()
{
    QTextStream(stdout) << "Usage: kiosque-update [--background | --check | --help]\n"
                           "\n"
                           "Without options the daemon is started on demand (D-Bus activation)\n"
                           "and exits after it has been idle for a while.\n"
                           "\n"
                           "  --background  Keep running while idle (autostart; alias: --daemon)\n"
                           "  --check       Check for and install all updates once, then exit\n"
                           "  --help        Show this help and exit\n";
}

} // namespace

int main(int argc, char *argv[])
{
    bool background = false;
    bool oneShot = false;
    for (int i = 1; i < argc; ++i) {
        const char *arg = argv[i];
        if (strcmp(arg, "--help") == 0 || strcmp(arg, "-h") == 0) {
            printUsage();
            return 0;
        } else if (strcmp(arg, "--check") == 0 || strcmp(arg, "-c") == 0) {
            oneShot = true;
        } else if (strcmp(arg, "--background") == 0 || strcmp(arg, "--daemon") == 0) {
            background = true;
        }
    }

    if (oneShot) {
        return kiosque_run_update_once();
    }

    QApplication app(argc, argv);
    QApplication::setDesktopFileName(QStringLiteral("org.kiosque.Kiosque"));
    QApplication::setApplicationName(QStringLiteral("KiosqueUpdate"));
    QApplication::setOrganizationName(QStringLiteral("Kiosque"));
    QApplication::setOrganizationDomain(QStringLiteral("kiosque.app"));
    QApplication::setApplicationDisplayName(QStringLiteral("Kiosque Update Notifier"));
    QApplication::setQuitOnLastWindowClosed(false);
    setupLocalization();

    UpdateDaemon daemon(background);
    if (!daemon.start()) {
        // Another instance owns the daemon name (or no session bus): nothing to do.
        return 0;
    }
    return app.exec();
}

#include "update_main.moc"
