#pragma once
#include <QtCore/QCoreApplication>
#include <QtCore/QTimer>
#include <QtCore/QSettings>
#include <QtGui/QGuiApplication>
#include <QtGui/QScreen>
#include <QtGui/QStyleHints>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include "artwork.h"
#if QT_VERSION < QT_VERSION_CHECK(6, 8, 0)
#error "osu-radio-qt requires Qt 6.8 or newer for native QML window interactions"
#endif
inline QString loadTrackNamePreferences(bool &titles, bool &artists) {
    QSettings settings(QSettings::NativeFormat, QSettings::UserScope,
        QStringLiteral("osu-radio"), QStringLiteral("osu-radio-qt"));
    settings.setFallbacksEnabled(false);
    titles = settings.value(QStringLiteral("display/use_unicode_titles"), false).toBool();
    artists = settings.value(QStringLiteral("display/use_unicode_artists"), false).toBool();
    settings.sync();
    if (settings.status() == QSettings::NoError) return {};
    titles = artists = false;
    return QStringLiteral("Could not read track name preferences. Using default names.");
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
