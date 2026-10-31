#include "browser.h"
#include <QApplication>
#include <QDir>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QPainter>
#include <QStyleHints>
#include <QWebEngineDownloadRequest>
#include <QWebEngineFileSystemAccessRequest>
#include <QWebEngineHistory>
#include <QWebEngineLoadingInfo>
#include <QWebEnginePage>
#include <QWebEnginePermission>
#include <QWebEngineProfile>
#include <QWebEngineSettings>
#include <QWebEngineView>
#include <cmath>

class Page : public QWebEnginePage {
public:
    Page(QWebEngineProfile *profile, QObject *parent) : QWebEnginePage(profile, parent) {
        connect(this, &QWebEnginePage::permissionRequested, this, [](QWebEnginePermission permission) { permission.deny(); });
        connect(this, &QWebEnginePage::fileSystemAccessRequested, this, [](QWebEngineFileSystemAccessRequest request) { request.reject(); });
        settings()->setUnknownUrlSchemePolicy(QWebEngineSettings::DisallowUnknownUrlSchemes);
        settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, false);
        settings()->setAttribute(QWebEngineSettings::JavascriptCanAccessClipboard, false);
        settings()->setAttribute(QWebEngineSettings::JavascriptCanPaste, false);
        settings()->setAttribute(QWebEngineSettings::LocalContentCanAccessRemoteUrls, false);
        settings()->setAttribute(QWebEngineSettings::LocalContentCanAccessFileUrls, false);
    }
protected:
    bool acceptNavigationRequest(const QUrl &url, NavigationType, bool mainFrame) override {
        return !mainFrame || httpUrl(url);
    }
    QWebEnginePage *createWindow(WebWindowType) override { return nullptr; }
    void javaScriptConsoleMessage(JavaScriptConsoleMessageLevel, const QString &, int, const QString &) override {
        // Websites can print URLs or private data; do not forward their console to logs.
    }
};
QWebEngineProfile *makeProfile(QObject *parent) {
    auto *profile = new QWebEngineProfile("omawser-qt", parent);
    auto root = [](const char *key, const QString &fallback) {
        auto path = qEnvironmentVariable(key);
        return path.isEmpty() ? QDir::homePath()+fallback : path;
    };
    profile->setPersistentStoragePath(root("XDG_DATA_HOME", "/.local/share")+"/omawser-qt");
    profile->setCachePath(root("XDG_CACHE_HOME", "/.cache")+"/omawser-qt");
    profile->setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    profile->setPersistentPermissionsPolicy(QWebEngineProfile::PersistentPermissionsPolicy::AskEveryTime);
    QObject::connect(profile, &QWebEngineProfile::downloadRequested, profile, [](QWebEngineDownloadRequest *download) { download->cancel(); });
    // Keep Qt's actual Chromium version and default user agent for honest measurement.
    return profile;
}
Browser::Browser(QWebEngineProfile *p, const QUrl &url) : profile(p) {
    setAttribute(Qt::WA_DeleteOnClose);
    setWindowFlags(Qt::Window|Qt::FramelessWindowHint);
    setWindowTitle("Omawser Qt");
    resize(1200, 850);
    setMinimumSize(480, 320);
    view = new QWebEngineView(this);
    view->setObjectName("web");
    view->setPage(new Page(profile, view));
    view->setContextMenuPolicy(Qt::NoContextMenu);
    prompt = new QLineEdit(this);
    prompt->setObjectName("address");
    prompt->setFrame(false);
    prompt->setClearButtonEnabled(false);
    prompt->hide();
    error = new QLabel(this);
    error->setTextFormat(Qt::PlainText);
    error->setAttribute(Qt::WA_TransparentForMouseEvents);
    error->hide();
    connect(prompt, &QLineEdit::returnPressed, this, [this] { if (!composing) navigate(prompt->text()); });
    connect(view->page(), &QWebEnginePage::loadingChanged, this, [this](const QWebEngineLoadingInfo &info) {
        if (info.status() == QWebEngineLoadingInfo::LoadFailedStatus) {
            if (!httpUrl(info.url())) return; // Blocked schemes are not load-error targets.
            failedTarget = info.url();
            openPrompt("Load failed. Ctrl+R retry or enter another address.", true);
        } else if (info.status() == QWebEngineLoadingInfo::LoadSucceededStatus) {
            failedTarget = QUrl();
            if (errorPrompt) closePrompt();
            else if (!prompt->isVisible() && isActiveWindow()) focusContent();
        }
    });
    connect(view, &QWebEngineView::titleChanged, this, [this](const QString &title) { setWindowTitle(title+" — Omawser Qt"); });
    new ThemeWatcher(themePaths(), [this](const Theme &t) { applyTheme(t); }, this);
    qApp->installEventFilter(this);
    layout();
    if (httpUrl(url)) view->load(url);
}
void Browser::layout() {
    const int top = prompt->isHidden() ? 1 : 48;
    view->setGeometry(1, top, width()-2, height()-top-1);
    prompt->setGeometry(8, error->isHidden() ? 8 : 3, width()-16, error->isHidden() ? 30 : 26);
    error->setGeometry(12, 30, width()-24, 17);
}
void Browser::resizeEvent(QResizeEvent *event) { QWidget::resizeEvent(event); layout(); }
void Browser::paintEvent(QPaintEvent *) {
    QPainter painter(this);
    painter.fillRect(rect(), QColor(palette.value("background")));
    painter.setPen(QColor(palette.value("accent")));
    painter.drawRect(0, 0, width()-1, height()-1);
}
void Browser::applyTheme(const Theme &t) {
    palette = t;
    auto appPalette = QApplication::palette();
    appPalette.setColor(QPalette::Window, QColor(t["background"]));
    appPalette.setColor(QPalette::Base, QColor(t["background"]));
    appPalette.setColor(QPalette::Text, QColor(t["foreground"]));
    appPalette.setColor(QPalette::WindowText, QColor(t["foreground"]));
    appPalette.setColor(QPalette::Highlight, QColor(t["accent"]));
    appPalette.setColor(QPalette::HighlightedText, QColor(t["background"]));
    QApplication::setPalette(appPalette);
    QGuiApplication::styleHints()->setColorScheme(t["mode"] == "light" ? Qt::ColorScheme::Light : Qt::ColorScheme::Dark);
    prompt->setStyleSheet(QString("QLineEdit { border: 1px solid %1; border-radius: 0; background: %2; color: %3; padding: 2px 4px; }").arg(t["accent"], t["background"], t["foreground"]));
    error->setStyleSheet(QString("QLabel { background: %1; color: %2; font-size: 11px; }").arg(t["background"], t["foreground"]));
    view->page()->setBackgroundColor(QColor(t["background"]));
    update();
}
void Browser::focusContent() {
    if (prompt->isHidden()) view->setFocus(Qt::OtherFocusReason);
    else prompt->setFocus(Qt::OtherFocusReason); // Refocus preserves draft and selection.
}
void Browser::openPrompt(const QString &message, bool loadError) {
    prompt->setText((loadError ? failedTarget : view->url()).toString());
    errorPrompt = loadError;
    error->setText(message);
    prompt->show();
    error->setVisible(!message.isEmpty());
    layout();
    prompt->setFocus(); prompt->selectAll();
}
void Browser::closePrompt() {
    prompt->hide(); error->hide(); errorPrompt = false; composing = false;
    layout(); focusContent();
}
void Browser::navigate(const QString &input) {
    auto target = address(input);
    if (target.isEmpty()) {
        error->setText("Enter a valid http:// or https:// address.");
        error->show(); prompt->setGeometry(8, 3, width()-16, 26); prompt->setFocus();
        return;
    }
    failedTarget = QUrl();
    closePrompt(); view->load(target);
}
bool Browser::handles(const QKeyEvent *event) const {
    if (composing) return false;
    const auto mods = event->modifiers();
    const int key = event->key();
    if (mods == Qt::ControlModifier) {
        if (key == Qt::Key_L || key == Qt::Key_R || key == Qt::Key_N || key == Qt::Key_W || key == Qt::Key_0) return true;
    }
    if (mods == Qt::ControlModifier || mods == (Qt::ControlModifier|Qt::ShiftModifier)) {
        if (key == Qt::Key_Plus || key == Qt::Key_Equal || key == Qt::Key_Minus) return true;
    }
    if (mods == Qt::AltModifier && (key == Qt::Key_Left || key == Qt::Key_Right || key == Qt::Key_Home)) return true;
    if (mods == Qt::NoModifier) {
        if (key == Qt::Key_F5 || key == Qt::Key_F11) return true;
        if (key == Qt::Key_Escape && (!prompt->isHidden() || view->page()->isLoading())) return true;
    }
    return false;
}
void Browser::command(const QKeyEvent *event) {
    const int key = event->key();
    if (key == Qt::Key_L) openPrompt();
    else if (key == Qt::Key_R || key == Qt::Key_F5) { if (failedTarget.isEmpty()) view->reload(); else view->load(failedTarget); }
    else if (key == Qt::Key_N) { auto *window = new Browser(profile); window->show(); window->activateWindow(); }
    else if (key == Qt::Key_W) close();
    else if (key == Qt::Key_0) view->setZoomFactor(1.0);
    else if (key == Qt::Key_Left) view->back();
    else if (key == Qt::Key_Right) view->forward();
    else if (key == Qt::Key_Home) navigate("https://x.com/home");
    else if (key == Qt::Key_F11) { if (isFullScreen()) showNormal(); else showFullScreen(); }
    else if (key == Qt::Key_Escape) { if (!prompt->isHidden()) closePrompt(); else view->stop(); }
    else {
        double zoom = std::log(view->zoomFactor()) / std::log(1.2);
        view->setZoomFactor(qBound(0.25, std::pow(1.2, zoom+(key == Qt::Key_Minus ? -0.5 : 0.5)), 5.0));
    }
}
bool Browser::eventFilter(QObject *object, QEvent *event) {
    auto *widget = qobject_cast<QWidget *>(object);
    if (!widget || widget->window() != this) return false;
    if (event->type() == QEvent::WindowActivate && object == this) {
        QTimer::singleShot(0, this, [this] { if (isActiveWindow()) focusContent(); });
    }
    if (event->type() == QEvent::InputMethod) composing = !static_cast<QInputMethodEvent *>(event)->preeditString().isEmpty();
    if (event->type() == QEvent::ShortcutOverride || event->type() == QEvent::KeyPress) {
        auto *key = static_cast<QKeyEvent *>(event);
        if (handles(key)) {
            key->accept();
            if (event->type() == QEvent::KeyPress) command(key);
            return true;
        }
    }
    return false;
}
