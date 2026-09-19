#pragma once
#include <QtQuick/QQuickImageProvider>
#include <QtCore/QBuffer>
#include <QtGui/QImageReader>
#include <QtCore/QMutex>
#include <QtCore/QMutexLocker>
#include <algorithm>
#include <cstdint>
#include "cxx-qt-lib/qvector.h"
#include <map>
#include <memory>

// The provider is the only decoded-image cache. QML disables its extra image cache.
class ArtworkCache {
public:
    struct Entry { QImage image; std::uint64_t touched; };
    QMutex mutex;
    std::uint64_t generation = 0, clock = 0;
    qsizetype bytes = 0;
    std::map<int, Entry> entries;
};
inline std::shared_ptr<ArtworkCache> artworkCache() {
    static auto cache = std::make_shared<ArtworkCache>();
    return cache;
}
inline std::uint64_t resetArtwork() {
    auto cache = artworkCache();
    QMutexLocker lock(&cache->mutex);
    ++cache->generation;
    cache->entries.clear();
    cache->bytes = 0;
    return cache->generation;
}
inline QString artworkUrl(std::uint64_t generation, int id) {
    auto cache = artworkCache();
    QMutexLocker lock(&cache->mutex);
    if (cache->generation != generation || cache->entries.find(id) == cache->entries.end()) return {};
    return QStringLiteral("image://artwork/%1/%2").arg(generation).arg(id);
}
inline QVector<int> installArtwork(std::uint64_t generation, int id, const QByteArray &bytes) {
    QBuffer buffer;
    buffer.setData(bytes);
    buffer.open(QIODevice::ReadOnly);
    QImageReader reader(&buffer);
    const auto size = reader.size();
    if (!size.isValid()) return {};
    const auto scaled = size.scaled(QSize(1280, 1280), Qt::KeepAspectRatio);
    if (size.width() > 1280 || size.height() > 1280) reader.setScaledSize(scaled);
    auto image = reader.read();
    if (image.isNull()) return {};
    if (image.width() > 1280 || image.height() > 1280)
        image = image.scaled(1280, 1280, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    auto cache = artworkCache();
    QMutexLocker lock(&cache->mutex);
    if (cache->generation != generation) return {};
    if (auto old = cache->entries.find(id); old != cache->entries.end()) {
        cache->bytes -= old->second.image.sizeInBytes();
        cache->entries.erase(old);
    }
    QVector<int> affected {id};
    constexpr qsizetype budget = 64 * 1024 * 1024;
    while (!cache->entries.empty() && cache->bytes + image.sizeInBytes() > budget) {
        auto victim = std::min_element(cache->entries.begin(), cache->entries.end(),
            [](const auto &a, const auto &b) { return a.second.touched < b.second.touched; });
        affected.append(victim->first);
        cache->bytes -= victim->second.image.sizeInBytes();
        cache->entries.erase(victim);
    }
    cache->bytes += image.sizeInBytes();
    cache->entries.emplace(id, ArtworkCache::Entry{std::move(image), ++cache->clock});
    return affected;
}
class ArtworkProvider final : public QQuickImageProvider {
public:
    ArtworkProvider() : QQuickImageProvider(QQuickImageProvider::Image), cache(artworkCache()) {}
    QImage requestImage(const QString &id, QSize *size, const QSize &) override {
        const auto parts = id.split('/');
        QMutexLocker lock(&cache->mutex);
        if (parts.size() != 2 || parts[0].toULongLong() != cache->generation) return {};
        auto found = cache->entries.find(parts[1].toInt());
        if (found == cache->entries.end()) return {};
        found->second.touched = ++cache->clock;
        if (size) *size = found->second.image.size();
        return found->second.image;
    }
private:
    std::shared_ptr<ArtworkCache> cache;
};
