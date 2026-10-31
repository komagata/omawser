#include "core.h"
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QRegularExpression>
#include <QSet>

bool httpUrl(const QUrl &url) {
    return url.isValid() && !url.host().isEmpty()
        && (url.scheme() == "https" || url.scheme() == "http");
}
QUrl address(QString input) {
    input = input.trimmed();
    if (input.isEmpty() || input.contains(QRegularExpression("[\\x00-\\x20\\x7f]"))) return {};
    if (!input.contains(QRegularExpression("^[a-zA-Z][a-zA-Z0-9+.-]*:"))) input.prepend("https://");
    QUrl url(input, QUrl::StrictMode);
    return httpUrl(url) ? url : QUrl();
}
Theme parseTheme(const QString &source) {
    Theme t{{"background", "#1a1b26"}, {"foreground", "#c0caf5"}, {"accent", "#7aa2f7"},
            {"muted", "#565f89"}, {"lighter_background", "#24283b"}, {"mode", "dark"}};
    const QRegularExpression entry(R"(^\s*(\w+)\s*=\s*["']([^"']+)["'])", QRegularExpression::MultilineOption);
    const QRegularExpression color("^#[a-fA-F0-9]{6}$");
    auto matches = entry.globalMatch(source);
    while (matches.hasNext()) {
        auto m = matches.next();
        auto key = m.captured(1), value = m.captured(2);
        if (key == "mode") { if (value == "dark" || value == "light") t[key] = value; }
        else if (t.contains(key) && color.match(value).hasMatch()) t[key] = value;
    }
    return t;
}
static QString xdg(const char *variable, const QString &fallback) {
    auto value = qEnvironmentVariable(variable);
    return value.isEmpty() ? QDir::homePath()+fallback : value;
}
QStringList themePaths() {
    return {xdg("XDG_STATE_HOME", "/.local/state")+"/omarchy/current/theme/colors.toml",
            xdg("XDG_CONFIG_HOME", "/.config")+"/omarchy/current/theme/colors.toml"};
}
ThemeWatcher::ThemeWatcher(QStringList p, std::function<void(const Theme &)> c, QObject *parent)
    : QObject(parent), paths(std::move(p)), changed(std::move(c)) {
    debounce.setSingleShot(true);
    debounce.setInterval(50);
    auto schedule = [this](const QString &) { debounce.start(); };
    connect(&watcher, &QFileSystemWatcher::directoryChanged, this, schedule);
    connect(&watcher, &QFileSystemWatcher::fileChanged, this, schedule);
    connect(&debounce, &QTimer::timeout, this, [this] { arm(); refresh(); });
    arm();
    refresh();
}
void ThemeWatcher::refresh() {
    QString source;
    for (const auto &path : paths) {
        QFile f(path);
        if (f.open(QIODevice::ReadOnly)) { source = QString::fromUtf8(f.readAll()); break; }
    }
    auto next = parseTheme(source);
    if (current != next) { current = next; changed(current); }
}
void ThemeWatcher::arm() {
    // Watch existing lexical ancestors and canonical target ancestors. This covers
    // creation of missing descendants and replacement of symlinks or their targets.
    QSet<QString> wanted;
    auto chain = [&wanted](QString path) {
        while (true) {
            QFileInfo info(path);
            if (info.exists()) wanted.insert(info.absoluteFilePath());
            QString parent = info.absolutePath();
            if (parent == path) break;
            path = parent;
        }
    };
    for (const auto &path : paths) {
        QString prefix = QFileInfo(path).absoluteFilePath();
        chain(prefix);
        while (true) {
            const auto resolved = QFileInfo(prefix).canonicalFilePath();
            if (!resolved.isEmpty()) chain(resolved);
            const auto parent = QFileInfo(prefix).absolutePath();
            if (parent == prefix) break;
            prefix = parent;
        }
    }
    const auto existing = watcher.files()+watcher.directories();
    if (!existing.isEmpty()) watcher.removePaths(existing);
    if (!wanted.isEmpty()) watcher.addPaths(wanted.values());
    // No periodic retry on unavailable watches; a surviving ancestor can recover.
}
