#pragma once
#include <QObject>
#include <QQmlEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QTemporaryDir>
#include <QUrl>
#include <KLocalizedQmlContext>
#include <KLocalizedString>
#include "../../src/image_provider.h"

extern "C" bool kiosque_test_setup(const char*, const char*);
extern "C" void kiosque_test_shutdown();
void qml_register_types_com_kiosque();

class Setup : public QObject {
    Q_OBJECT
    QTemporaryDir root;
public slots:
    void applicationAvailable() {
        const auto endpoint = qgetenv("KIOSQUE_TEST_HTTP");
        if (!root.isValid() || QUrl(QString::fromUtf8(endpoint)).host() != QStringLiteral("127.0.0.1") ||
            !kiosque_test_setup(endpoint.constData(), root.path().toUtf8().constData()))
            qFatal("An isolated fixture server and writable temporary storage are required");
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
        KLocalizedString::setApplicationDomain("kiosque");
        qml_register_types_com_kiosque();
    }
    void qmlEngineAvailable(QQmlEngine* engine) {
        engine->rootContext()->setContextProperty(QStringLiteral("testArtifactDir"), qEnvironmentVariable("KIOSQUE_TEST_ARTIFACTS"));
        engine->addImageProvider(QStringLiteral("kiosque"), new KiosqueImageProvider());
        KLocalization::setupLocalizedContext(engine);
    }
    void cleanupTestCase() { kiosque_test_shutdown(); }
};
