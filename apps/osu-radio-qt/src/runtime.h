#pragma once
#include <QtCore/QCoreApplication>
#include <QtCore/QTimer>
#include <QtCore/QSettings>
#include <QtCore/QEventLoop>
#include <QtCore/QPointer>
#include <QtGui/QCloseEvent>
#include <QtGui/QImage>
#include <QtGui/QGuiApplication>
#include <QtGui/QScreen>
#include <QtGui/QStyleHints>
#include <QtQuick/QQuickWindow>
#include <QtQuick/QSGRendererInterface>
#include <QtQuickControls2/QQuickStyle>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include "artwork.h"
#include <algorithm>
#include <cstdint>
#include <memory>
#ifdef __GLIBC__
#include <malloc.h>
#endif
#ifdef Q_OS_WIN
#include <winrt/Windows.Media.h>
#include <systemmediatransportcontrolsinterop.h>
#endif

// Rust completion is independent of QObject delivery and survives event-loop exit.
void attach_window_media(std::intptr_t hwnd) noexcept;
void begin_window_media_detach() noexcept;
bool window_media_detached() noexcept;
void stop_tray() noexcept;

class LiveWindowMedia final : public QObject {
    QPointer<QQuickWindow> window;
    QTimer poll;
    bool closing = false;
    bool probe = false;
    bool observedDetach = false;
    int waitingTicks = 0;
#ifdef Q_OS_WIN
    HWND hwnd = nullptr;
    bool smtcEnabled() const {
        using Controls = winrt::Windows::Media::SystemMediaTransportControls;
        auto factory = winrt::get_activation_factory<Controls, ISystemMediaTransportControlsInterop>();
        Controls controls{nullptr};
        winrt::check_hresult(factory->GetForWindow(hwnd, winrt::guid_of<Controls>(), winrt::put_abi(controls)));
        return controls.IsEnabled();
    }
#endif
    void checkAlive() const {
        if (!window || !window->handle()) qFatal("System media Qt window destroyed before detach");
#ifdef Q_OS_WIN
        if (hwnd && !IsWindow(hwnd)) qFatal("System media HWND destroyed before detach");
#endif
    }
    void acknowledged() {
        if (!probe || observedDetach) return;
        checkAlive();
        if (waitingTicks == 0) qFatal("Detach probe did not process Qt events while waiting");
#ifdef Q_OS_WIN
        if (hwnd && smtcEnabled()) qFatal("SMTC still enabled after detach acknowledgement");
#endif
        qInfo() << "System media Qt detach acknowledged; window alive; waiting ticks:" << waitingTicks;
        observedDetach = true;
    }
    void waitForDetach() {
        begin_window_media_detach();
        QEventLoop cleanup;
        QTimer progress;
        progress.setInterval(5);
        QObject::connect(&progress, &QTimer::timeout, &cleanup, [&]() {
            if (probe) { checkAlive(); ++waitingTicks; }
            if (window_media_detached()) cleanup.quit();
        });
        progress.start();
        // Continue processing Qt work while Rust awaits native cleanup.
        while (!window_media_detached()) cleanup.exec(QEventLoop::ExcludeUserInputEvents);
        acknowledged();
    }
protected:
    bool eventFilter(QObject *object, QEvent *event) override {
        if (object != window || event->type() != QEvent::Close || window_media_detached())
            return false;
        // QML sets this only while the tray icon exists and the user wants close to hide.
        if (window->property("closeToTray").toBool()) {
            event->ignore();
            window->hide();
            return true;
        }
        event->ignore();
        closing = true;
        begin_window_media_detach();
        poll.start();
        return true;
    }
public:
    explicit LiveWindowMedia(QQuickWindow *root) : window(root) {
        const auto testCase = qEnvironmentVariable("OSU_RADIO_QT_PROBE_CASE");
        const bool smoke = qEnvironmentVariableIsSet("OSU_RADIO_QT_SMOKE_TEST");
        probe = smoke && testCase.startsWith(QStringLiteral("system-media"));
        // winId creates the platform window even when a smoke root starts hidden.
        root->create();
        std::intptr_t native = 0;
#ifdef Q_OS_WIN
        if (QGuiApplication::platformName() == QStringLiteral("windows")) {
            hwnd = reinterpret_cast<HWND>(root->winId());
            native = reinterpret_cast<std::intptr_t>(hwnd);
        }
        if (probe && !hwnd) qFatal("Native SMTC probe requires the windows platform plugin");
        if (hwnd) attach_window_media(native);
#else
        attach_window_media(native);
#endif
        if (smoke) qInfo() << "System media Qt attach:" << QGuiApplication::platformName() << native;
        root->installEventFilter(this);
        poll.setInterval(5);
        QObject::connect(&poll, &QTimer::timeout, this, [this]() {
            if (probe) { checkAlive(); ++waitingTicks; }
            if (closing && window_media_detached()) {
                poll.stop();
                acknowledged();
                // close() does nothing on a window hidden to the tray; the second quit passes the filter.
                if (window && window->isVisible()) window->close();
                else QCoreApplication::quit();
            }
        });
        if (probe) {
#ifdef Q_OS_WIN
            QObject::connect(root, &QObject::destroyed, QCoreApplication::instance(), [handle = hwnd]() {
                if (!window_media_detached() || IsWindow(handle)) qFatal("Incorrect media/window destruction order");
                qInfo("System media Qt window destroyed after detach");
            });
#endif
            auto *ready = new QTimer(this);
            ready->setInterval(10);
            QObject::connect(ready, &QTimer::timeout, this, [this, ready, testCase]() {
#ifdef Q_OS_WIN
                if (!smtcEnabled()) return;
#endif
                auto *bridge = window->findChild<QObject *>(QStringLiteral("appBridge"));
                if (!bridge || !bridge->property("connected").toBool()) return;
                ready->stop();
                qInfo("System media Qt native publication observed");
                if (testCase == QStringLiteral("system-media-close")) {
                    window->close();
                    if (window_media_detached() || !window->isVisible()) qFatal("Close did not wait for delayed detach");
                } else {
                    QCoreApplication::quit();
                }
            });
            ready->start();
        }
    }
    ~LiveWindowMedia() override {
        stop_tray();
        poll.stop();
        for (auto *timer : findChildren<QTimer *>()) timer->stop();
        waitForDetach();
        if (window) window->removeEventFilter(this);
    }
};

inline QPointer<QQuickWindow> trayWindow;
inline void releaseWindowResources() {
    if (trayWindow && !trayWindow->isVisible()) {
        trayWindow->setPersistentSceneGraph(false);
        trayWindow->setPersistentGraphics(false);
        trayWindow->releaseResources();
        // The software loop only deletes obsolete scene nodes when it synchronizes a frame.
        if (trayWindow->rendererInterface()->graphicsApi() == QSGRendererInterface::Software)
            trayWindow->grabWindow();
        if (auto *engine = qjsEngine(trayWindow)) engine->collectGarbage();
#ifdef __GLIBC__
        // Decoder arenas otherwise retain freed pages across repeated tray cycles.
        malloc_trim(0);
#endif
    }
}
// Tray callbacks arrive on the Rust runtime; the window's own QML functions run on the Qt thread.
inline void trayCall(const QString &method) {
    if (auto *window = trayWindow.data())
        QMetaObject::invokeMethod(window, method.toUtf8().constData(), Qt::QueuedConnection);
}
inline QByteArray trayIconRgba(int size) {
    QImage image(QStringLiteral(":/assets/logos/lazer.png"));
    if (image.isNull()) return {};
    image = image.scaled(size, size, Qt::KeepAspectRatio, Qt::SmoothTransformation)
        .convertToFormat(QImage::Format_RGBA8888);
    return QByteArray(reinterpret_cast<const char *>(image.constBits()), static_cast<qsizetype>(image.sizeInBytes()));
}
inline std::unique_ptr<LiveWindowMedia> connectLiveWindow(QQmlApplicationEngine &engine) {
    auto *window = engine.rootObjects().isEmpty() ? nullptr : qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (!window) return {};
    trayWindow = window;
    {
        auto cache = artworkCache();
        QMutexLocker lock(&cache->mutex);
        qreal dpr = 1;
        for (auto *screen : QGuiApplication::screens()) dpr = (std::max)(dpr, screen->devicePixelRatio());
        cache->thumbnailEdge = dpr <= 1 ? 512 : dpr <= 2 ? 1024 : 1280;
    }
    return std::make_unique<LiveWindowMedia>(window);
}
#if QT_VERSION < QT_VERSION_CHECK(6, 8, 0)
#error "osu-radio-qt requires Qt 6.8 or newer for native QML window interactions"
#endif
inline bool loadLocalPreferences(bool &titles, bool &artists, bool &discord, bool &closeToTray) {
    QSettings settings(QSettings::NativeFormat, QSettings::UserScope,
        QStringLiteral("osu-radio"), QStringLiteral("osu-radio-qt"));
    settings.setFallbacksEnabled(false);
    titles = settings.value(QStringLiteral("display/use_unicode_titles"), false).toBool();
    artists = settings.value(QStringLiteral("display/use_unicode_artists"), false).toBool();
    discord = settings.value(QStringLiteral("integrations/discord_rich_presence"), false).toBool();
    closeToTray = settings.value(QStringLiteral("window/close_to_tray"), true).toBool();
    settings.sync();
    if (settings.status() == QSettings::NoError) return true;
    titles = artists = discord = false;
    closeToTray = true;
    return false;
}
inline QString saveTrackNamePreference(bool title, bool enabled) {
    QSettings settings(QSettings::NativeFormat, QSettings::UserScope,
        QStringLiteral("osu-radio"), QStringLiteral("osu-radio-qt"));
    settings.setFallbacksEnabled(false);
    settings.setValue(title ? QStringLiteral("display/use_unicode_titles")
                           : QStringLiteral("display/use_unicode_artists"), enabled);
    settings.sync();
    return settings.status() == QSettings::NoError ? QString()
        : QStringLiteral("Could not save track name preferences. Your choice applies for this session.");
}
inline QString saveDiscordRichPresence(bool enabled) {
    QSettings settings(QSettings::NativeFormat, QSettings::UserScope,
        QStringLiteral("osu-radio"), QStringLiteral("osu-radio-qt"));
    settings.setFallbacksEnabled(false);
    settings.setValue(QStringLiteral("integrations/discord_rich_presence"), enabled);
    settings.sync();
    return settings.status() == QSettings::NoError ? QString()
        : QStringLiteral("Could not save Discord preference. Your choice applies for this session.");
}
inline QString saveCloseToTray(bool enabled) {
    QSettings settings(QSettings::NativeFormat, QSettings::UserScope,
        QStringLiteral("osu-radio"), QStringLiteral("osu-radio-qt"));
    settings.setFallbacksEnabled(false);
    settings.setValue(QStringLiteral("window/close_to_tray"), enabled);
    settings.sync();
    return settings.status() == QSettings::NoError ? QString()
        : QStringLiteral("Could not save the tray preference. Your choice applies for this session.");
}
inline void configureMemory() {
#ifdef __GLIBC__
    // Keep large image/JSON buffers out of arenas after glibc's adaptive threshold rises.
    mallopt(M_MMAP_THRESHOLD, 128 * 1024);
#endif
}
inline void configureEngine(QQmlApplicationEngine &engine, bool smoke) {
    // Dialog internals import unqualified Controls; match the app's Basic controls.
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    auto *styleHints = QGuiApplication::styleHints();
    styleHints->setWheelScrollLines(styleHints->wheelScrollLines() * 2);
    engine.addImageProvider(QStringLiteral("artwork"), new ArtworkProvider());
    QObject::connect(&engine, &QQmlEngine::quit, QCoreApplication::instance(), &QCoreApplication::quit);
    // Keep test windows hidden until the requested screen and geometry are set.
    if (smoke) engine.setInitialProperties({{QStringLiteral("visible"), false}});
    if (smoke) QTimer::singleShot(12000, &engine, []() {
        qCritical("Qt smoke probe watchdog expired");
        QCoreApplication::exit(1);
    });
}
inline void configureProbe(QQmlApplicationEngine &engine, bool gallery) {
    auto *window = engine.rootObjects().isEmpty() ? nullptr : qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (!window) qFatal("Qt probe has no window");
    const auto screenName = qEnvironmentVariable("OSU_RADIO_QT_PROBE_SCREEN");
    if (!screenName.isEmpty()) {
        // Wayland leaves initial placement to the compositor; use xcb for desktop probes.
        if (QGuiApplication::platformName().startsWith(QStringLiteral("wayland")))
            qFatal("Screen-targeted Qt probes require QT_QPA_PLATFORM=xcb");
        QScreen *selected = nullptr;
        for (auto *screen : QGuiApplication::screens()) {
            if (screen->name() == screenName) selected = screen;
        }
        if (!selected) qFatal("Requested Qt probe screen is unavailable");
        window->setScreen(selected);
    }
    const int width = qEnvironmentVariableIntValue("OSU_RADIO_QT_PROBE_WIDTH");
    const int height = qEnvironmentVariableIntValue("OSU_RADIO_QT_PROBE_HEIGHT");
    if (width > 0 && height > 0) window->resize(width, height);
    if (!screenName.isEmpty()) {
        const auto bounds = window->screen()->availableGeometry();
        window->setPosition(bounds.topLeft() + QPoint(qMax(0, (bounds.width() - window->width()) / 2),
            qMax(0, (bounds.height() - window->height()) / 2)));
    }
    window->show();
    qInfo() << "Qt probe window:" << window->screen()->name() << window->geometry() << window->devicePixelRatio();
    if (!screenName.isEmpty()) QTimer::singleShot(200, window, [window, screenName]() {
        if (window->screen()->name() != screenName || !window->screen()->geometry().contains(window->frameGeometry().center()))
            qFatal("Qt probe was placed on the wrong screen");
        qInfo() << "Qt probe placed:" << window->screen()->name() << window->frameGeometry();
    });
    const auto screenshot = qEnvironmentVariable("OSU_RADIO_QT_PROBE_SCREENSHOT");
    if (qEnvironmentVariable("OSU_RADIO_QT_PROBE_CASE").startsWith(QStringLiteral("native")) && !screenshot.isEmpty()) {
        auto *capture = new QTimer(&engine);
        capture->setInterval(50);
        QObject::connect(capture, &QTimer::timeout, &engine, [capture, window, screenName, screenshot]() {
            for (auto *candidate : QGuiApplication::allWindows()) {
                auto *dialog = qobject_cast<QQuickWindow *>(candidate);
                if (!dialog || dialog == window || !dialog->isExposed() || dialog->title() != QStringLiteral("Choose playlist cover")) continue;
                qInfo() << "Qt probe dialog:" << dialog->screen()->name() << dialog->frameGeometry();
                if (!screenName.isEmpty() && dialog->screen()->name() != screenName)
                    qFatal("Qt probe dialog was placed on the wrong screen");
                const auto frame = dialog->grabWindow();
                if (!frame.isNull()) {
                    if (!frame.save(screenshot + QStringLiteral("-file-dialog.png"))) qFatal("Could not save Qt dialog screenshot");
                    capture->stop();
                }
            }
        });
        capture->start();
    }
    engine.setInitialProperties({});
    engine.rootContext()->setContextProperty(QStringLiteral("probeCoverPath"), qEnvironmentVariable("OSU_RADIO_QT_PROBE_COVER"));
    engine.rootContext()->setContextProperty(QStringLiteral("probeScreenshotPath"), qEnvironmentVariable("OSU_RADIO_QT_PROBE_SCREENSHOT"));
    engine.rootContext()->setContextProperty(QStringLiteral("probeWindow"),
        engine.rootObjects().isEmpty() ? nullptr : engine.rootObjects().first());
    engine.rootContext()->setContextProperty(QStringLiteral("probeGallery"), gallery);
    engine.rootContext()->setContextProperty(QStringLiteral("probeCase"), qEnvironmentVariable("OSU_RADIO_QT_PROBE_CASE", "fixture"));
}
