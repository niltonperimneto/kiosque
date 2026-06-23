// Kiosque — Flatpak Storefront for KDE Plasma
// Minimal C++ entry point. All logic lives in Rust via cxx-qt.

#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QUrl>
#include <QDir>
#include <QCoreApplication>
#include <KLocalizedQmlContext>
#include <KLocalizedString>

// Forward declaration of the generated QML type registration function
void qml_register_types_com_kiosque();

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("Kiosque"));
    QGuiApplication::setOrganizationName(QStringLiteral("Kiosque"));
    QGuiApplication::setOrganizationDomain(QStringLiteral("kiosque.app"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Kiosque"));
    QGuiApplication::setDesktopFileName(QStringLiteral("org.kiosque.Kiosque"));
    QGuiApplication::setWindowIcon(QIcon(":/qml/images/logo.svg"));

    // Allow loading translations from the build directory for testing/development
    QDir localeDir(QCoreApplication::applicationDirPath() + QStringLiteral("/../po"));
    if (localeDir.exists()) {
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), localeDir.absolutePath());
    }
    KLocalizedString::setApplicationDomain("kiosque");

    // Use the native KDE/Plasma desktop style
    QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));

    // Manually register Rust backend QML types to ensure static linkage is not discarded
    qml_register_types_com_kiosque();

    QQmlApplicationEngine engine;
    KLocalization::setupLocalizedContext(&engine);

    // Load the root QML from embedded resources
    const QUrl url(QStringLiteral("qrc:/qml/Main.qml"));
    QObject::connect(
        &engine, &QQmlApplicationEngine::objectCreationFailed,
        &app, []() { QCoreApplication::exit(-1); },
        Qt::QueuedConnection);

    engine.load(url);

    if (engine.rootObjects().isEmpty()) {
        return -1;
    }

    return app.exec();
}
