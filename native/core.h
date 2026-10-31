#pragma once
#include <QFileSystemWatcher>
#include <QMap>
#include <QObject>
#include <QTimer>
#include <QUrl>
#include <functional>
using Theme = QMap<QString, QString>;
QUrl address(QString input);
bool httpUrl(const QUrl &url);
Theme parseTheme(const QString &source);
QStringList themePaths();
class ThemeWatcher : public QObject {
public:
    ThemeWatcher(QStringList paths, std::function<void(const Theme &)> changed, QObject *parent = nullptr);
private:
    void refresh();
    void arm();
    QStringList paths;
    std::function<void(const Theme &)> changed;
    QFileSystemWatcher watcher;
    QTimer debounce;
    Theme current;
};
