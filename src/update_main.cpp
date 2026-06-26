// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

#include <QApplication>
#include <QCoreApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QAction>
#include <QTimer>
#include <QProcess>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusMessage>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QDir>
#include <QFile>
#include <QTextStream>
#include <QFileInfo>
#include <QCommandLineParser>
#include <KLocalizedString>
#include <QDebug>

class UpdateDaemon : public QObject {
    Q_OBJECT
public:
    UpdateDaemon(QObject* parent = nullptr) : QObject(parent) {
        m_trayIcon = new QSystemTrayIcon(QIcon::fromTheme("system-software-update"), this);
        
        m_menu = new QMenu();
        m_actionOpen = m_menu->addAction(QIcon::fromTheme("kiosque"), i18n("Open Store"));
        m_actionCheck = m_menu->addAction(QIcon::fromTheme("view-refresh"), i18n("Check for Updates"));
        m_actionUpdateAll = m_menu->addAction(QIcon::fromTheme("system-software-update"), i18n("Update All"));
        m_menu->addSeparator();
        m_actionQuit = m_menu->addAction(QIcon::fromTheme("application-exit"), i18n("Quit"));
        
        m_trayIcon->setContextMenu(m_menu);
        
        connect(m_actionOpen, &QAction::triggered, this, &UpdateDaemon::openStore);
        connect(m_actionCheck, &QAction::triggered, this, &UpdateDaemon::checkUpdatesAsync);
        connect(m_actionUpdateAll, &QAction::triggered, this, &UpdateDaemon::updateAllAsync);
        connect(m_actionQuit, &QAction::triggered, qApp, &QCoreApplication::quit);
        
        connect(m_trayIcon, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
            if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
                openStore();
            }
        });
        
        // Connect DBus Notification signals
        QDBusConnection::sessionBus().connect(
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "ActionInvoked",
            this,
            SLOT(onNotificationActionInvoked(uint,QString))
        );
        
        // Timer for periodic checking (every 4 hours)
        m_checkTimer = new QTimer(this);
        connect(m_checkTimer, &QTimer::timeout, this, &UpdateDaemon::checkUpdatesAsync);
    }
    
    void start() {
        if (!QFile::exists(QStringLiteral("/.flatpak-info"))) {
            createAutostartFile();
        }
        requestBackgroundPortal();
        
        // Check immediately
        checkUpdatesAsync();
        
        // Start periodic check
        m_checkTimer->start(4 * 60 * 60 * 1000); // 4 hours in ms
    }

public Q_SLOTS:
    void openStore() {
        QProcess::startDetached("kiosque", QStringList());
    }
    
    void checkUpdatesAsync() {
        if (m_checking) return;
        m_checking = true;
        m_actionCheck->setEnabled(false);
        m_actionUpdateAll->setEnabled(false);
        
        QProcess* proc = new QProcess(this);
        connect(proc, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), this, [this, proc](int exitCode, QProcess::ExitStatus exitStatus) {
            m_checking = false;
            m_actionCheck->setEnabled(true);
            
            if (exitStatus == QProcess::NormalExit && exitCode == 0) {
                QByteArray out = proc->readAllStandardOutput().trimmed();
                parseCheckOutput(out);
            } else {
                qDebug() << "Update check process failed:" << proc->readAllStandardError();
            }
            proc->deleteLater();
        });
        
        proc->start("flatpak", QStringList() << "remote-ls" << "--updates" << "-j");
    }
    
    void updateAllAsync() {
        if (m_updating || m_updatesList.isEmpty()) return;
        m_updating = true;
        m_actionCheck->setEnabled(false);
        m_actionUpdateAll->setEnabled(false);
        
        sendNotification(i18n("Updating Applications"), i18n("Kiosque is installing flatpak updates in the background..."), false);
        
        QProcess* proc = new QProcess(this);
        connect(proc, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), this, [this, proc](int exitCode, QProcess::ExitStatus exitStatus) {
            m_updating = false;
            m_actionCheck->setEnabled(true);
            
            if (exitStatus == QProcess::NormalExit && exitCode == 0) {
                sendNotification(i18n("Updates Completed"), i18n("All applications have been successfully updated."), false);
                m_updatesList.clear();
                m_trayIcon->hide();
            } else {
                sendNotification(i18n("Updates Failed"), i18n("An error occurred while installing updates."), false);
                m_actionUpdateAll->setEnabled(true);
            }
            proc->deleteLater();
        });
        
        proc->start("flatpak", QStringList() << "update" << "-y");
    }
    
    void onNotificationActionInvoked(uint notificationId, const QString& actionKey) {
        if (notificationId == m_lastNotificationId && actionKey == "open") {
            openStore();
        }
    }

private:
    void parseCheckOutput(const QByteArray& data) {
        m_updatesList.clear();
        if (data.isEmpty() || data == "[]") {
            m_trayIcon->hide();
            return;
        }
        
        QJsonDocument doc = QJsonDocument::fromJson(data);
        if (doc.isArray()) {
            QJsonArray arr = doc.array();
            for (const auto& val : arr) {
                QJsonObject obj = val.toObject();
                QString appId = obj.value("application").toString();
                if (!appId.isEmpty()) {
                    m_updatesList.append(appId);
                }
            }
        }
        
        int count = m_updatesList.size();
        if (count > 0) {
            m_actionUpdateAll->setEnabled(true);
            m_trayIcon->setToolTip(i18n("Kiosque: %1 updates available", count));
            m_trayIcon->show();
            
            sendNotification(
                i18n("Software Updates Available"),
                i18n("%1 application updates are available. Click to open Kiosque.", count),
                true
            );
        } else {
            m_trayIcon->hide();
        }
    }
    
    void sendNotification(const QString& title, const QString& body, bool hasAction) {
        QDBusInterface notify(
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            QDBusConnection::sessionBus()
        );
        if (!notify.isValid()) {
            qDebug() << "Notifications service not available.";
            return;
        }
        
        QStringList actions;
        if (hasAction) {
            actions << "open" << i18n("Open Kiosque");
        }
        
        QVariantMap hints;
        QDBusMessage reply = notify.call("Notify", "Kiosque", (quint32)m_lastNotificationId, "system-software-update", title, body, actions, hints, (qint32)10000);
        if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
            m_lastNotificationId = reply.arguments().at(0).toUInt();
        }
    }
    
    void createAutostartFile() {
        QString autostartDir = QDir::homePath() + "/.config/autostart";
        QDir().mkpath(autostartDir);
        
        QFile file(autostartDir + "/com.kiosque.app.update.desktop");
        if (file.open(QIODevice::WriteOnly | QIODevice::Text)) {
            QTextStream out(&file);
            out << "[Desktop Entry]\n"
                << "Type=Application\n"
                << "Name=Kiosque Update Notifier\n"
                << "Comment=Checks for Flatpak updates in the background\n"
                << "Exec=kiosque-update --daemon\n"
                << "Icon=system-software-update\n"
                << "Terminal=false\n"
                << "X-GNOME-Autostart-enabled=true\n";
            file.close();
        }
    }
    
    void requestBackgroundPortal() {
        QDBusInterface portal(
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Background",
            QDBusConnection::sessionBus()
        );
        if (!portal.isValid()) {
            qDebug() << "Background portal not available.";
            return;
        }
        
        QVariantMap options;
        options.insert("handle_token", "kiosque_update_bg");
        options.insert("reason", "Check and notify for application updates in the background");
        options.insert("autostart", true);
        options.insert("commandline", QStringList() << "kiosque-update" << "--daemon");
        
        portal.call("RequestBackground", "", options);
    }
    
    QSystemTrayIcon* m_trayIcon;
    QMenu* m_menu;
    QAction* m_actionOpen;
    QAction* m_actionCheck;
    QAction* m_actionUpdateAll;
    QAction* m_actionQuit;
    QTimer* m_checkTimer;
    QStringList m_updatesList;
    uint m_lastNotificationId = 0;
    bool m_checking = false;
    bool m_updating = false;
};

void runOneShotCheck() {
    qInfo() << "kiosque-update: Running one-shot update check...";
    QProcess checkProc;
    checkProc.start("flatpak", QStringList() << "remote-ls" << "--updates" << "-j");
    if (!checkProc.waitForFinished(30000)) {
        qCritical() << "Failed to check for updates: flatpak process timed out.";
        QCoreApplication::exit(-1);
        return;
    }
    
    QByteArray out = checkProc.readAllStandardOutput().trimmed();
    if (out.isEmpty() || out == "[]") {
        qInfo() << "No updates available.";
        QCoreApplication::exit(0);
        return;
    }
    
    qInfo() << "Updates are available. Running flatpak update...";
    QProcess updateProc;
    updateProc.setProcessChannelMode(QProcess::ForwardedChannels);
    updateProc.start("flatpak", QStringList() << "update" << "-y");
    if (!updateProc.waitForFinished(600000)) { // 10 minutes
        qCritical() << "Flatpak update timed out or failed.";
        QCoreApplication::exit(-1);
        return;
    }
    
    if (updateProc.exitCode() == 0) {
        qInfo() << "Updates completed successfully.";
        QCoreApplication::exit(0);
    } else {
        qCritical() << "Flatpak update failed with exit code" << updateProc.exitCode();
        QCoreApplication::exit(updateProc.exitCode());
    }
}

int main(int argc, char* argv[]) {
    bool daemonMode = true;
    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--check") == 0 || strcmp(argv[i], "-c") == 0) {
            daemonMode = false;
            break;
        }
    }
    
    if (!daemonMode) {
        QCoreApplication app(argc, argv);
        QCoreApplication::setApplicationName(QStringLiteral("KiosqueUpdate"));
        QCoreApplication::setOrganizationName(QStringLiteral("Kiosque"));
        QCoreApplication::setOrganizationDomain(QStringLiteral("kiosque.app"));
        
        QDir localeDir(QCoreApplication::applicationDirPath() + QStringLiteral("/../po"));
        if (localeDir.exists()) {
            KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), localeDir.absolutePath());
        } else {
#ifdef LOCALE_INSTALL_DIR
            KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), QStringLiteral(LOCALE_INSTALL_DIR));
#endif
        }
        KLocalizedString::setApplicationDomain("kiosque");
        
        QTimer::singleShot(0, &runOneShotCheck);
        return app.exec();
    }
    
    QApplication app(argc, argv);
    QApplication::setDesktopFileName(QStringLiteral("org.kiosque.Kiosque"));
    QApplication::setApplicationName(QStringLiteral("KiosqueUpdate"));
    QApplication::setOrganizationName(QStringLiteral("Kiosque"));
    QApplication::setOrganizationDomain(QStringLiteral("kiosque.app"));
    QApplication::setApplicationDisplayName(QStringLiteral("Kiosque Update Notifier"));
    
    QDir localeDir(QCoreApplication::applicationDirPath() + QStringLiteral("/../po"));
    if (localeDir.exists()) {
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), localeDir.absolutePath());
    } else {
#ifdef LOCALE_INSTALL_DIR
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), QStringLiteral(LOCALE_INSTALL_DIR));
#endif
    }
    KLocalizedString::setApplicationDomain("kiosque");
    
    UpdateDaemon daemon;
    daemon.start();
    
    return app.exec();
}

#include "update_main.moc"
