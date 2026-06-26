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

#include <QQuickImageProvider>
#include <cstdint>
#include <cstring>

extern "C" {
    struct RustImageResult {
        uint8_t* data;
        int len;
        int width;
        int height;
    };
    RustImageResult fetch_image_from_rust(
        const char* app_id,
        const char* img_type,
        int index,
        int req_width,
        int req_height
    );
    void free_rust_image(RustImageResult res);
}

class KiosqueImageProvider : public QQuickImageProvider
{
public:
    KiosqueImageProvider()
        : QQuickImageProvider(QQuickImageProvider::Image)
    {
    }

    QImage requestImage(const QString &id, QSize *size, const QSize &requestedSize) override
    {
        QStringList parts = id.split(QLatin1Char('/'));
        if (parts.size() < 2) {
            return QImage();
        }
        
        QString appId = parts.at(0);
        QString imgType = parts.at(1);
        int index = (parts.size() > 2) ? parts.at(2).toInt() : 0;

        int reqWidth = requestedSize.width();
        int reqHeight = requestedSize.height();

        RustImageResult res = fetch_image_from_rust(
            appId.toUtf8().constData(),
            imgType.toUtf8().constData(),
            index,
            reqWidth,
            reqHeight
        );

        if (res.data == nullptr || res.len <= 0 || res.width <= 0 || res.height <= 0) {
            return QImage();
        }

        QImage img(res.width, res.height, QImage::Format_RGBA8888);
        if (img.sizeInBytes() == res.len) {
            std::memcpy(img.bits(), res.data, res.len);
        } else {
            img = QImage();
        }

        if (size) {
            *size = QSize(res.width, res.height);
        }

        free_rust_image(res);
        return img;
    }
};

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
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
