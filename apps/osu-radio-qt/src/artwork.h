#pragma once
#include <QtQuick/QQuickImageProvider>
#include <QtCore/QBuffer>
#include <QtGui/QImageReader>
#include <QtCore/QMutex>
#include <QtCore/QMutexLocker>
#include <QtCore/QFile>
#include <QtCore/QUrl>
#include <algorithm>
#include <cstdint>
#include "cxx-qt-lib/qvector.h"
#include <map>
#include <memory>

// The provider is the only decoded-image cache. QML disables its extra image cache.
class ArtworkCache {
public:
    struct Entry { QImage image; std::uint64_t touched, version; };
    QMutex mutex;
    std::uint64_t generation = 0, clock = 0;
    qsizetype bytes = 0;
    std::map<QString, Entry> entries;
    QString protectedKey;
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
    cache->protectedKey.clear();
    return cache->generation;
}
inline QString cachedArtworkUrl(std::uint64_t generation, const QString &key) {
    auto cache = artworkCache();
    QMutexLocker lock(&cache->mutex);
    auto found = cache->entries.find(key);
    if (cache->generation != generation || found == cache->entries.end()) return {};
    found->second.touched = ++cache->clock;
    return QStringLiteral("image://artwork/%1/%2/%3").arg(generation).arg(found->second.version).arg(key);
}
inline void protectArtwork(std::uint64_t generation, int id) {
    auto cache = artworkCache();
    QMutexLocker lock(&cache->mutex);
    if (cache->generation == generation)
        cache->protectedKey = id >= 0 ? QString::number(id) : QString{};
}
inline QString artworkUrl(std::uint64_t generation, int id) {
    return cachedArtworkUrl(generation, QString::number(id));
}
inline QString playlistArtworkKey(int id, std::int64_t revision, int beatmap) {
    return QStringLiteral("playlist/%1/%2").arg(id).arg(
        revision >= 0 ? QString::number(revision) : QStringLiteral("auto-%1").arg(beatmap));
}
inline QString playlistArtworkUrl(std::uint64_t generation, int id, std::int64_t revision, int beatmap) {
    return cachedArtworkUrl(generation, playlistArtworkKey(id, revision, beatmap));
}
inline QVector<int> installCachedArtwork(std::uint64_t generation, const QString &key, const QByteArray &bytes) {
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
    if (auto old = cache->entries.find(key); old != cache->entries.end()) {
        cache->bytes -= old->second.image.sizeInBytes();
        cache->entries.erase(old);
    }
    QVector<int> affected;
    bool track = false;
    const auto trackId = key.toInt(&track);
    if (track) affected.append(trackId);
    constexpr qsizetype budget = 64 * 1024 * 1024;
    if (image.sizeInBytes() > budget) return {};
    while (!cache->entries.empty() && cache->bytes + image.sizeInBytes() > budget) {
        auto victim = cache->entries.end();
        for (auto entry = cache->entries.begin(); entry != cache->entries.end(); ++entry)
            if (entry->first != cache->protectedKey &&
                (victim == cache->entries.end() || entry->second.touched < victim->second.touched))
                victim = entry;
        if (victim == cache->entries.end()) return {};
        cache->bytes -= victim->second.image.sizeInBytes();
        cache->entries.erase(victim);
    }
    cache->bytes += image.sizeInBytes();
    const auto version = ++cache->clock;
    cache->entries.emplace(key, ArtworkCache::Entry{std::move(image), version, version});
    return affected;
}
inline QVector<int> installArtwork(std::uint64_t generation, int id, const QByteArray &bytes) {
    return installCachedArtwork(generation, QString::number(id), bytes);
}
inline QVector<int> installPlaylistArtwork(std::uint64_t generation, int id, std::int64_t revision, int beatmap, const QByteArray &bytes) {
    return installCachedArtwork(generation, playlistArtworkKey(id, revision, beatmap), bytes);
}
inline QString draftArtworkUrl(std::uint64_t generation, std::uint64_t serial) {
    return cachedArtworkUrl(generation, QStringLiteral("draft/%1").arg(serial));
}
inline QVector<int> installDraftArtwork(std::uint64_t generation, std::uint64_t serial, const QByteArray &bytes) {
    return installCachedArtwork(generation, QStringLiteral("draft/%1").arg(serial), bytes);
}
inline QByteArray preparePlaylistCover(const QString &url, QString &error) {
    error.clear();
    const QUrl source(url);
    if (!source.isLocalFile()) { error = "Choose a local PNG or JPEG image."; return {}; }
    QFile file(source.toLocalFile());
    constexpr qint64 limit = 16 * 1024 * 1024;
    if (!file.open(QIODevice::ReadOnly)) { error = "Cannot open the selected image."; return {}; }
    if (file.size() > limit) { error = "Images must be at most 16 MiB."; return {}; }
    const auto data = file.read(limit + 1);
    if (data.size() > limit) { error = "Images must be at most 16 MiB."; return {}; }
    QBuffer input;
    input.setData(data);
    input.open(QIODevice::ReadOnly);
    QImageReader reader(&input);
    const auto format = reader.format().toLower();
    if (format != "png" && format != "jpeg" && format != "jpg") {
        error = "Choose a PNG or JPEG image."; return {};
    }
    const auto size = reader.size();
    if (!size.isValid() || qint64(size.width()) * size.height() > 16000000) {
        error = "Images must be at most 16 megapixels."; return {};
    }
    reader.setAutoTransform(true);
    const auto image = reader.read();
    if (image.isNull()) { error = "Cannot decode the selected image."; return {}; }
    const auto side = std::min(image.width(), image.height());
    const auto square = image.copy((image.width() - side) / 2, (image.height() - side) / 2, side, side)
        .scaled(512, 512, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
    QByteArray png;
    QBuffer output(&png);
    output.open(QIODevice::WriteOnly);
    if (!square.save(&output, "PNG") || png.size() > 2 * 1024 * 1024) {
        error = "Cannot prepare the cover PNG."; return {};
    }
    return png;
}
class ArtworkProvider final : public QQuickImageProvider {
public:
    ArtworkProvider() : QQuickImageProvider(QQuickImageProvider::Image), cache(artworkCache()) {}
    QImage requestImage(const QString &id, QSize *size, const QSize &) override {
        const auto parts = id.split('/');
        QMutexLocker lock(&cache->mutex);
        if (parts.size() < 3 || parts[0].toULongLong() != cache->generation) return {};
        auto found = cache->entries.find(parts.mid(2).join('/'));
        // Versions refresh QML; earlier URLs still address this immutable epoch/key.
        if (found == cache->entries.end()) return {};
        found->second.touched = ++cache->clock;
        if (size) *size = found->second.image.size();
        return found->second.image;
    }
private:
    std::shared_ptr<ArtworkCache> cache;
};
