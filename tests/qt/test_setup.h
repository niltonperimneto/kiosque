#pragma once
#include <QObject>
#include <QCoreApplication>
#include <QQmlEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QTemporaryDir>
#include <QUrl>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusVirtualObject>
#include <QDBusArgument>
#include <KLocalizedQmlContext>
#include <KLocalizedString>
#include "../../src/image_provider.h"

extern "C" bool kiosque_test_setup(const char*, const char*);
extern "C" void kiosque_test_shutdown();
extern "C" bool kiosque_test_set_updates(const char*);
extern "C" void kiosque_test_hold_operations(bool);
extern "C" unsigned long long kiosque_test_operation_count();
extern "C" const char* kiosque_test_last_install_remote();
void qml_register_types_com_kiosque();

// Registered only on the harness's private dbus-run-session bus. No desktop
// permission dialogs or real autostart entries are created by settings tests.
class BackgroundPortal : public QDBusVirtualObject {
    Q_OBJECT
    Q_PROPERTY(int requestCount MEMBER requestCount NOTIFY changed)
    Q_PROPERTY(bool lastAutostart MEMBER lastAutostart NOTIFY changed)
    Q_PROPERTY(QStringList lastCommand MEMBER lastCommand NOTIFY changed)
    Q_PROPERTY(bool denyNext MEMBER denyNext)
public:
    int requestCount = 0;
    bool lastAutostart = false;
    bool denyNext = false;
    QStringList lastCommand;
    QString introspect(const QString &) const override {
        return QStringLiteral("<interface name=\"org.freedesktop.portal.Background\"><method name=\"RequestBackground\"><arg type=\"s\" direction=\"in\"/><arg type=\"a{sv}\" direction=\"in\"/><arg type=\"o\" direction=\"out\"/></method></interface>");
    }
    bool handleMessage(const QDBusMessage &message, const QDBusConnection &bus) override {
        if (message.interface() != QStringLiteral("org.freedesktop.portal.Background") ||
            message.member() != QStringLiteral("RequestBackground")) {
            bus.send(message.createErrorReply(QStringLiteral("org.freedesktop.DBus.Error.UnknownMethod"), QStringLiteral("Not implemented by test portal")));
            return true;
        }
        const auto options = qdbus_cast<QVariantMap>(message.arguments().at(1));
        lastAutostart = options.value(QStringLiteral("autostart")).toBool();
        lastCommand = qdbus_cast<QStringList>(options.value(QStringLiteral("commandline")));
        ++requestCount;
        emit changed();
        QString sender = message.service().mid(1);
        sender.replace('.', '_');
        const QString path = QStringLiteral("/org/freedesktop/portal/desktop/request/") + sender + '/' + options.value(QStringLiteral("handle_token")).toString();
        const bool denied = denyNext;
        denyNext = false;
        bus.send(message.createReply(QVariant::fromValue(QDBusObjectPath(path))));
        // Send immediately after the method reply to cover the subscription race.
        QVariantMap results{{QStringLiteral("background"), !denied}, {QStringLiteral("autostart"), lastAutostart && !denied}};
        auto response = QDBusMessage::createSignal(path, QStringLiteral("org.freedesktop.portal.Request"), QStringLiteral("Response"));
        response.setArguments({QVariant::fromValue(denied ? 2u : 0u), results});
        bus.send(response);
        return true;
    }
signals:
    void changed();
};

// Controls the harness's mock Flatpak backend. Only linked into test binaries.
class BackendFixture : public QObject {
    Q_OBJECT
public:
    Q_INVOKABLE bool setUpdates(const QStringList &ids) {
        return kiosque_test_set_updates(ids.join(QLatin1Char('\n')).toUtf8().constData());
    }
    Q_INVOKABLE void holdOperations(bool hold) { kiosque_test_hold_operations(hold); }
    Q_INVOKABLE int operationCount() const { return int(kiosque_test_operation_count()); }
    Q_INVOKABLE QString lastInstallRemote() const { return QString::fromUtf8(kiosque_test_last_install_remote()); }
};

class Setup : public QObject {
    Q_OBJECT
    QTemporaryDir root;
    BackgroundPortal portal;
    BackendFixture backend;
public slots:
    void applicationAvailable() {
        const auto endpoint = qgetenv("KIOSQUE_TEST_HTTP");
        if (!root.isValid() || QUrl(QString::fromUtf8(endpoint)).host() != QStringLiteral("127.0.0.1") ||
            !kiosque_test_setup(endpoint.constData(), root.path().toUtf8().constData()))
            qFatal("An isolated fixture server and writable temporary storage are required");
        auto bus = QDBusConnection::sessionBus();
        if (!bus.registerService(QStringLiteral("org.freedesktop.portal.Desktop")) ||
            !bus.registerVirtualObject(QStringLiteral("/org/freedesktop/portal/desktop"), &portal, QDBusConnection::SubPath))
            qFatal("A private D-Bus session without a real portal is required");
        QCoreApplication::setOrganizationName(QStringLiteral("KiosqueTests"));
        QCoreApplication::setOrganizationDomain(QStringLiteral("test.kiosque.local"));
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
        KLocalizedString::setApplicationDomain("kiosque");
        qml_register_types_com_kiosque();
    }
    void qmlEngineAvailable(QQmlEngine* engine) {
        engine->rootContext()->setContextProperty(QStringLiteral("testArtifactDir"), qEnvironmentVariable("KIOSQUE_TEST_ARTIFACTS"));
        engine->rootContext()->setContextProperty(QStringLiteral("backgroundPortalFixture"), &portal);
        engine->rootContext()->setContextProperty(QStringLiteral("backendFixture"), &backend);
        engine->rootContext()->setContextProperty(QStringLiteral("fixtureEndpoint"), qEnvironmentVariable("KIOSQUE_TEST_HTTP"));
        // Present in production (set by main.cpp); empty means no launch target.
        engine->rootContext()->setContextProperty(QStringLiteral("initialAppId"), QString());
        engine->rootContext()->setContextProperty(QStringLiteral("initialRefFile"), QString());
        engine->rootContext()->setContextProperty(QStringLiteral("initialRepoFile"), QString());
        engine->addImageProvider(QStringLiteral("kiosque"), new KiosqueImageProvider());
        KLocalization::setupLocalizedContext(engine);
    }
    void cleanupTestCase() { kiosque_test_shutdown(); }
};
