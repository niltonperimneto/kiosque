#pragma once
#include <QImage>
#include <QString>
#include <QSize>
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
        const char* id,
        int req_width,
        int req_height
    );
    void free_rust_image(RustImageResult res);
}

class KiosqueImageProvider : public QQuickImageProvider
{
public:
    KiosqueImageProvider()
        : QQuickImageProvider(QQuickImageProvider::Image, QQmlImageProviderBase::ForceAsynchronousImageLoading)
    {
    }

    QImage requestImage(const QString &id, QSize *size, const QSize &requestedSize) override
    {
        int reqWidth = requestedSize.width();
        int reqHeight = requestedSize.height();

        RustImageResult res = fetch_image_from_rust(
            id.toUtf8().constData(),
            reqWidth,
            reqHeight
        );

        if (res.data == nullptr || res.len <= 0 || res.width <= 0 || res.height <= 0) {
            return QImage();
        }

        const qint64 pixels = qint64(res.width) * res.height;
        if (res.len % 4 != 0 || pixels != res.len / 4) {
            free_rust_image(res);
            return QImage();
        }
        auto *owned = new RustImageResult(res);
        QImage wrapped(res.data, res.width, res.height, QImage::Format_RGBA8888,
            [](void *context) {
                auto *pixels = static_cast<RustImageResult *>(context);
                free_rust_image(*pixels);
                delete pixels;
            }, owned);
        QImage img = wrapped.convertToFormat(QImage::Format_ARGB32_Premultiplied);
        if (size) { *size = QSize(res.width, res.height); }

        return img;
    }
};

