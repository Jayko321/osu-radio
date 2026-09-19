#pragma once
#include "src/app_bridge.cxxqt.h"
#include "artwork.h"
#include <memory>

inline std::unique_ptr<AppBridge> newAppBridge() { return std::make_unique<AppBridge>(); }
inline QString checkArtworkCache() {
    const auto generation = resetArtwork();
    if (!installArtwork(generation, 1, QByteArray("invalid")).isEmpty()) return "invalid image accepted";
    QImage original(2560, 1280, QImage::Format_ARGB32);
    original.fill(Qt::red);
    QByteArray bytes;
    QBuffer buffer(&bytes);
    buffer.open(QIODevice::WriteOnly);
    if (!original.save(&buffer, "PNG")) return "fixture encoding failed";
    if (installArtwork(generation, 1, bytes).isEmpty()) return "valid image rejected";
    ArtworkProvider provider;
    QSize size;
    auto image = provider.requestImage(QStringLiteral("%1/1").arg(generation), &size, {});
    if (size != QSize(1280, 640) || image.size() != size) return "proportional scaling failed";
    if (image.pixelColor(10, 10) != QColor(Qt::red)) return "decoded pixels differ";
    for (int id = 2; id <= 24; ++id) installArtwork(generation, id, bytes);
    if (!artworkUrl(generation, 1).isEmpty() || artworkUrl(generation, 24).isEmpty()) return "eviction failed";
    {
        auto cache = artworkCache();
        QMutexLocker lock(&cache->mutex);
        if (cache->bytes > 64 * 1024 * 1024) return "decoded budget exceeded";
    }
    resetArtwork();
    if (!installArtwork(generation, 99, bytes).isEmpty()) return "stale decode installed";
    if (!provider.requestImage(QStringLiteral("%1/24").arg(generation), &size, {}).isNull()) return "stale provider image exposed";
    resetArtwork();
    return {};
}
