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

#include "image_provider.h"
#include <QCommandLineParser>
#include <QFileInfo>
#include <QQmlContext>
#include <QTextStream>
#include <QTimer>

namespace {

struct LaunchTarget {
    QString appId;
    QString refFile;
};

bool isFlatpakRef(const QString &path)
{
    return path.endsWith(QLatin1String(".flatpakref"), Qt::CaseInsensitive);
}

// Interpret one positional argument (desktop file `Exec=kiosque %u`):
// appstream://<id>, appstream:<id>, a .flatpakref path, or a file:// or
// http(s):// URL to a .flatpakref.
void parseLaunchArgument(const QString &arg, LaunchTarget &target)
{
    if (arg.startsWith(QLatin1String("appstream:"), Qt::CaseInsensitive)) {
        QString id = arg.mid(int(qstrlen("appstream:")));
        while (id.startsWith(QLatin1Char('/'))) {
            id.remove(0, 1);
        }
        while (id.endsWith(QLatin1Char('/'))) {
            id.chop(1);
        }
        id = QUrl::fromPercentEncoding(id.toUtf8()).trimmed();
        if (!id.isEmpty()) {
            target.appId = id;
        }
        return;
    }
    const QUrl url(arg);
    const QString scheme = url.scheme().toLower();
    if (scheme == QLatin1String("file")) {
        if (isFlatpakRef(url.path())) {
            target.refFile = url.toLocalFile();
        }
    } else if (scheme == QLatin1String("http") || scheme == QLatin1String("https")) {
        if (isFlatpakRef(url.path())) {
            target.refFile = url.toString();
        }
    } else if (isFlatpakRef(arg)) {
        target.refFile = QFileInfo(arg).absoluteFilePath();
    }
}

} // namespace

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    QCommandLineParser parser;
    parser.addHelpOption();
    parser.addOption({QStringLiteral("check-ui"), QStringLiteral("Validate startup resources and exit.")});
    parser.addOption({QStringLiteral("app"), QStringLiteral("Open the details page of an application."), QStringLiteral("id")});
    parser.addPositionalArgument(QStringLiteral("url"),
                                 QStringLiteral("appstream:// link, or .flatpakref file or URL to install."),
                                 QStringLiteral("[url]"));
    parser.process(app);

    LaunchTarget launch;
    for (const QString &arg : parser.positionalArguments()) {
        parseLaunchArgument(arg, launch);
    }
    if (parser.isSet(QStringLiteral("app"))) {
        launch.appId = parser.value(QStringLiteral("app")).trimmed();
    }
    QGuiApplication::setApplicationName(QStringLiteral("Kiosque"));
    QGuiApplication::setApplicationVersion(QStringLiteral(PROJECT_VERSION));
    QGuiApplication::setOrganizationName(QStringLiteral("Kiosque"));
    QGuiApplication::setOrganizationDomain(QStringLiteral("kiosque.app"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Kiosque"));
    QGuiApplication::setDesktopFileName(QStringLiteral("org.kiosque.Kiosque"));
    QGuiApplication::setWindowIcon(QIcon(":/qml/images/logo.svg"));

    // Allow loading translations from the build directory for testing/development
    QDir localeDir(QCoreApplication::applicationDirPath() + QStringLiteral("/../po"));
    if (localeDir.exists()) {
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), localeDir.absolutePath());
    } else {
#ifdef LOCALE_INSTALL_DIR
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), QStringLiteral(LOCALE_INSTALL_DIR));
#endif
    }
    KLocalizedString::setApplicationDomain("kiosque");

    // Use the native KDE/Plasma desktop style
    QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));

    // Manually register Rust backend QML types to ensure static linkage is not discarded
    qml_register_types_com_kiosque();

    QQmlApplicationEngine engine;
    engine.addImageProvider(QStringLiteral("kiosque"), new KiosqueImageProvider());
    KLocalization::setupLocalizedContext(&engine);
    // Always defined (empty by default) so QML never hits a ReferenceError.
    engine.rootContext()->setContextProperty(QStringLiteral("initialAppId"), launch.appId);
    engine.rootContext()->setContextProperty(QStringLiteral("initialRefFile"), launch.refFile);


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

    if (parser.isSet(QStringLiteral("check-ui"))) {
        if (QImage(QStringLiteral(":/qml/images/logo.svg")).isNull()) {
            qCritical("Cannot decode packaged logo");
            return 2;
        }
        QTextStream(stdout) << "KIOSQUE_UI_READY\n";
        QTimer::singleShot(0, &app, &QCoreApplication::quit);
    }
    return app.exec();
}
