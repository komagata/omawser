#include "browser.h"
#include <QApplication>
#include <QCoreApplication>
#include <QWebEngineProfile>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>
#include <cstdio>
#include <cstring>
int main(int argc, char **argv) {
    if (argc == 2 && std::strcmp(argv[1], "--version") == 0) {
        // No QApplication, profile, window or navigation on the version path.
        std::printf("omawser-qt experimental\nQt %s\nQt WebEngine %s\nChromium %s\nChromium security patch %s\n", qVersion(), qWebEngineVersion(), qWebEngineChromiumVersion(), qWebEngineChromiumSecurityPatchVersion());
        return 0;
    }
    QUrl url("https://x.com/home");
    if (argc > 2 || (argc == 2 && (url = address(QString::fromUtf8(argv[1]))).isEmpty())) {
        std::fputs("Usage: omawser-qt [HTTP(S)-URL] | --version\n", stderr); return 2;
    }
    QApplication app(argc, argv);
    app.setApplicationName("omawser-qt");
    QGuiApplication::setDesktopFileName("omawser-qt");
    auto *profile = makeProfile(&app);
    auto *window = new Browser(profile, url);
    window->show();
    const int result = app.exec();
    // Destroy every page before its shared profile (including secondary windows).
    for (auto *widget : QApplication::topLevelWidgets()) if (dynamic_cast<Browser *>(widget)) delete widget;
    delete profile;
    return result;
}
