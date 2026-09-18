#pragma once
#include <QtCore/QCoreApplication>
#include <QtCore/QTimer>
#include <QtQml/QQmlApplicationEngine>
#if QT_VERSION < QT_VERSION_CHECK(6, 8, 0)
#error "osu-radio-qt requires Qt 6.8 or newer for native QML window interactions"
#endif

inline void configureEngine(QQmlApplicationEngine &engine, bool smoke) {
    QObject::connect(&engine, &QQmlEngine::quit, QCoreApplication::instance(), &QCoreApplication::quit);
    if (smoke) {
        QTimer::singleShot(300, &engine, [&engine]() {
            QCoreApplication::exit(engine.rootObjects().isEmpty() ? 1 : 0);
        });
    }
}
