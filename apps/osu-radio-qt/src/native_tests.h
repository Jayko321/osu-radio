#pragma once
#include "src/app_bridge.cxxqt.h"
#include "artwork.h"
#include <QtCore/QTemporaryDir>
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
    auto image = provider.requestImage(artworkUrl(generation, 1).mid(QStringLiteral("image://artwork/").size()), &size, {});
    if (size != QSize(512, 256) || image.size() != size) return "proportional scaling failed";
    if (image.pixelColor(10, 10) != QColor(Qt::red)) return "decoded pixels differ";
    protectArtwork(generation, 1);
    installArtwork(generation, 1, bytes);
    const auto selected = provider.requestImage(selectedArtworkUrl(generation, 1).mid(QStringLiteral("image://artwork/").size()), &size, {});
    if (selected.size() != QSize(1280, 640) || image.size() != QSize(512, 256)) return "thumbnail replaced selected variant";
    const auto requested = provider.requestImage(artworkUrl(generation, 1).mid(QStringLiteral("image://artwork/").size()), nullptr, QSize(128, 128));
    if (requested.size() != QSize(128, 64)) return "provider ignored requested size";
    protectArtwork(generation, -1);
    const auto originalUrl = artworkUrl(generation, 1);
    installArtwork(generation, 1, bytes);
    if (artworkUrl(generation, 1) != originalUrl) return "duplicate delivery replaced immutable thumbnail";
    installCachedArtwork(generation, QString::number(1), bytes);
    const auto replacementUrl = artworkUrl(generation, 1);
    if (replacementUrl == originalUrl) return "same-key installation reused provider URL";
    const auto resident = provider.requestImage(originalUrl.mid(QStringLiteral("image://artwork/").size()), &size, {});
    if (resident.isNull() || resident.size() != image.size() || resident.pixelColor(10, 10) != QColor(Qt::red)) return "resident same-key image missing through earlier installation URL";
    if (provider.requestImage(replacementUrl.mid(QStringLiteral("image://artwork/").size()), &size, {}).isNull()) return "new image installation version missing";
    for (int id = 2; id <= 160; ++id) installArtwork(generation, id, bytes);
    if (!artworkUrl(generation, 1).isEmpty() || artworkUrl(generation, 160).isEmpty()) return "eviction failed";
    if (!provider.requestImage(replacementUrl.mid(QStringLiteral("image://artwork/").size()), &size, {}).isNull()) return "evicted provider key exposed";
    if (image.pixelColor(10, 10) != QColor(Qt::red)) return "eviction invalidated an already displayed image";
    {
        auto cache = artworkCache();
        QMutexLocker lock(&cache->mutex);
        if (cache->bytes > 64 * 1024 * 1024) return "decoded budget exceeded";
    }
    const auto lastUrl = artworkUrl(generation, 160);
    resetArtwork();
    if (!installArtwork(generation, 99, bytes).isEmpty()) return "stale decode installed";
    if (!provider.requestImage(lastUrl.mid(QStringLiteral("image://artwork/").size()), &size, {}).isNull()) return "stale provider image exposed";
    resetArtwork();
    const auto protectedGeneration = resetArtwork();
    installArtwork(protectedGeneration, 1, bytes);
    protectArtwork(protectedGeneration, 1);
    const auto protectedUrl = artworkUrl(protectedGeneration, 1);
    for (int id = 2; id <= 160; ++id) {
        const auto affected = installArtwork(protectedGeneration, id, bytes);
        if (affected.size() != 1 || affected.first() != id) return "eviction advertised a displayed victim";
    }
    if (artworkUrl(protectedGeneration, 1) != protectedUrl) return "selected player cover evicted";
    if (!artworkUrl(protectedGeneration, 2).isEmpty()) return "unpinned LRU victim retained";
    const auto reentered = installArtwork(protectedGeneration, 2, bytes);
    if (reentered.size() != 1 || artworkUrl(protectedGeneration, 2).isEmpty()) return "evicted cover cannot reload";
    {
        auto cache = artworkCache();
        QMutexLocker lock(&cache->mutex);
        if (cache->bytes > 64 * 1024 * 1024) return "protected cache exceeds budget";
    }
    resetArtwork();
    const auto playlistGeneration = resetArtwork();
    installArtwork(playlistGeneration, 1, bytes);
    installPlaylistArtwork(playlistGeneration, 1, 1, -1, bytes);
    installPlaylistArtwork(playlistGeneration, 1, 2, -1, bytes);
    if (artworkUrl(playlistGeneration, 1).isEmpty() || playlistArtworkUrl(playlistGeneration, 1, 1, -1).isEmpty()
        || playlistArtworkUrl(playlistGeneration, 1, 2, -1).isEmpty()) return "playlist cache keys collide";
    if (playlistArtworkUrl(playlistGeneration, 1, 1, -1) == playlistArtworkUrl(playlistGeneration, 1, 2, -1)) return "cover version URL reused";
    if (!installPlaylistArtwork(playlistGeneration, 1, -1, 9, bytes).isEmpty()) return "playlist key advertised a track change";
    if (!installDraftArtwork(playlistGeneration, 9, bytes).isEmpty()) return "draft key advertised a track change";
    const auto automatic = playlistArtworkUrl(playlistGeneration, 1, -1, 9);
    const auto draft = draftArtworkUrl(playlistGeneration, 9);
    if (automatic.isEmpty() || draft.isEmpty() || automatic == draft
        || automatic == playlistArtworkUrl(playlistGeneration, 1, 1, -1)) return "automatic/draft keys collide";
    if (!playlistArtworkUrl(playlistGeneration, 1, -1, 10).isEmpty()
        || !draftArtworkUrl(playlistGeneration, 10).isEmpty()) return "missing artwork key exposed";
    const auto prefix = QStringLiteral("image://artwork/").size();
    if (provider.requestImage(automatic.mid(prefix), nullptr, {}).isNull()
        || provider.requestImage(draft.mid(prefix), nullptr, {}).isNull()) return "nested provider keys missing";
    if (!provider.requestImage(QStringLiteral("1/invalid"), &size, {}).isNull()) return "malformed provider path exposed";
    const auto replacementGeneration = resetArtwork();
    if (replacementGeneration <= playlistGeneration || !artworkUrl(playlistGeneration, 1).isEmpty()
        || !playlistArtworkUrl(playlistGeneration, 1, -1, 9).isEmpty()
        || !draftArtworkUrl(playlistGeneration, 9).isEmpty()) return "reset retained old URLs";
    if (!installDraftArtwork(playlistGeneration, 9, bytes).isEmpty()
        || !draftArtworkUrl(replacementGeneration, 9).isEmpty()) return "stale draft installation accepted";
    {
        auto cache = artworkCache();
        QMutexLocker lock(&cache->mutex);
        if (cache->bytes != 0 || !cache->entries.empty() || !cache->protectedKey.isEmpty()) return "reset retained cache state";
    }
    return {};
}
inline QString checkPlaylistCoverPreparation() {
    QTemporaryDir directory;
    if (!directory.isValid()) return "temporary image directory failed";
    const auto path = directory.filePath("source.png");
    QImage original(800, 400, QImage::Format_RGB32);
    original.fill(Qt::green);
    for (int y = 0; y < 400; ++y) for (int x = 0; x < 200; ++x) {
        original.setPixelColor(x, y, Qt::red);
        original.setPixelColor(799 - x, y, Qt::blue);
    }
    if (!original.save(path)) return "source PNG encoding failed";
    QString error;
    const auto png = preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error);
    if (!error.isEmpty() || png.isEmpty()) return "valid PNG failed preparation";
    QFile::remove(path);
    const auto decoded = QImage::fromData(png, "PNG");
    if (decoded.size() != QSize(512, 512) || decoded.pixelColor(0, 0) != QColor(Qt::green)
        || decoded.pixelColor(511, 511) != QColor(Qt::green)) return "center crop or retained PNG failed";
    if (!preparePlaylistCover("https://example.invalid/image.png", error).isEmpty() || error.isEmpty()) return "remote URL accepted";
    if (!preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty()
        || error != "Cannot open the selected image.") return "missing local file error lost";
    if (!original.save(path, "BMP")) return "BMP fixture encoding failed";
    if (!preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty()
        || error != "Choose a PNG or JPEG image.") return "unsupported image format accepted";
    QFile invalid(path);
    if (!invalid.open(QIODevice::WriteOnly)) return "invalid fixture creation failed";
    invalid.write("broken image"); invalid.close();
    if (!preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty() || error.isEmpty()) return "corrupt image accepted";
    if (!invalid.open(QIODevice::WriteOnly) || !invalid.resize(16 * 1024 * 1024 + 1)) return "oversize fixture creation failed";
    invalid.close();
    if (!preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty() || !error.contains("16 MiB")) return "size limit ignored";
    QImage tooManyPixels(4001, 4000, QImage::Format_RGB32);
    tooManyPixels.fill(Qt::black);
    if (!tooManyPixels.save(path)) return "large PNG creation failed";
    if (!preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty() || !error.contains("16 megapixels")) return "pixel limit ignored";
    // A JPEG with EXIF orientation 6 must rotate the red/blue halves into top/bottom.
    QImage landscape(80, 40, QImage::Format_RGB32);
    landscape.fill(Qt::red);
    for (int y = 0; y < 40; ++y) for (int x = 40; x < 80; ++x) landscape.setPixelColor(x, y, Qt::blue);
    QByteArray jpeg; QBuffer jpegBuffer(&jpeg); jpegBuffer.open(QIODevice::WriteOnly);
    if (!landscape.save(&jpegBuffer, "JPEG")) return "JPEG plugin unavailable";
    const QByteArray exif = QByteArray::fromHex("ffe1002245786966000049492a0008000000010012010300010000000600000000000000");
    jpeg.insert(2, exif);
    if (!invalid.open(QIODevice::WriteOnly)) return "JPEG fixture creation failed";
    invalid.write(jpeg); invalid.close();
    const auto oriented = QImage::fromData(preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error), "PNG");
    if (!error.isEmpty() || oriented.isNull()) return "JPEG preparation failed";
    if (oriented.pixelColor(256, 40).red() < 200 || oriented.pixelColor(256, 470).blue() < 200) return "EXIF orientation ignored";
    // Keep valid PNG metadata but corrupt compressed pixels to reach the decode error.
    QByteArray encoded; QBuffer encodedBuffer(&encoded); encodedBuffer.open(QIODevice::WriteOnly);
    if (!original.save(&encodedBuffer, "PNG")) return "damaged fixture encoding failed";
    const auto idat = encoded.indexOf("IDAT");
    if (idat < 0 || idat + 6 >= encoded.size()) return "fixture pixel chunk missing";
    encoded[idat + 4] = 0; encoded[idat + 5] = 0;
    if (!invalid.open(QIODevice::WriteOnly)) return "damaged fixture creation failed";
    invalid.write(encoded); invalid.close();
    if (!preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty()
        || error != "Cannot decode the selected image.") return "damaged PNG decode error lost: " + error;
    if (!original.save(path, "PNG") || preparePlaylistCover(QUrl::fromLocalFile(path).toString(), error).isEmpty()
        || !error.isEmpty()) return "successful preparation retained a previous error";
    return {};
}
