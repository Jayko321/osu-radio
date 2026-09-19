#pragma once
#include <QtCore/QCoreApplication>
#include <QtCore/QTimer>
#include <QtGui/QGuiApplication>
#include <QtGui/QStyleHints>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include "artwork.h"
#if QT_VERSION < QT_VERSION_CHECK(6, 8, 0)
#error "osu-radio-qt requires Qt 6.8 or newer for native QML window interactions"
#endif
inline void configureEngine(QQmlApplicationEngine &engine, bool smoke) {
    auto *styleHints = QGuiApplication::styleHints();
    styleHints->setWheelScrollLines(styleHints->wheelScrollLines() * 2);
    engine.addImageProvider(QStringLiteral("artwork"), new ArtworkProvider());
    QObject::connect(&engine, &QQmlEngine::quit, QCoreApplication::instance(), &QCoreApplication::quit);
    if (smoke) QTimer::singleShot(12000, &engine, []() {
        qCritical("Qt smoke probe watchdog expired");
        QCoreApplication::exit(1);
    });
}
inline void configureProbe(QQmlApplicationEngine &engine, bool gallery) {
    engine.rootContext()->setContextProperty(QStringLiteral("probeWindow"),
        engine.rootObjects().isEmpty() ? nullptr : engine.rootObjects().first());
    engine.rootContext()->setContextProperty(QStringLiteral("probeGallery"), gallery);
    engine.rootContext()->setContextProperty(QStringLiteral("probeCase"), qEnvironmentVariable("OSU_RADIO_QT_PROBE_CASE", "fixture"));
}
