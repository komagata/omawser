#include "browser.h"
#include <QApplication>
#include <QDir>
#include <QFile>
#include <QLineEdit>
#include <QTcpServer>
#include <QTcpSocket>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QWebEngineView>
#include <QtTest>
#include <cstdio>

class GuiTest : public QObject {
    Q_OBJECT
    QTcpServer server;
    QWebEngineProfile *profile = nullptr;
    Browser *window = nullptr;
    QWebEngineView *view = nullptr;
    QLineEdit *prompt = nullptr;
    QUrl first, second;
    int requests = 0;
    void key(int key, Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
        auto *focus = QApplication::focusWidget();
        QVERIFY(focus);
        QTest::keyClick(focus, Qt::Key(key), modifiers);
    }
private slots:
    void initTestCase() {
        QVERIFY(server.listen(QHostAddress::LocalHost));
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto *socket = server.nextPendingConnection()) {
                auto buffer = std::make_shared<QByteArray>();
                connect(socket, &QTcpSocket::readyRead, socket, [this, socket, buffer] {
                    *buffer += socket->readAll();
                    if (!buffer->contains("\r\n\r\n")) return;
                    ++requests;
                    const QByteArray body = "<!doctype html><title>fixture</title><input id='field'><p>Local keyboard test</p>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: "+QByteArray::number(body.size())+"\r\nConnection: close\r\n\r\n"+body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        first = QUrl(QString("http://127.0.0.1:%1/one").arg(server.serverPort()));
        second = first.resolved(QUrl("/two"));
        profile = makeProfile(this);
        window = new Browser(profile, first);
        view = window->findChild<QWebEngineView *>("web");
        prompt = window->findChild<QLineEdit *>("address");
        QVERIFY(view); QVERIFY(prompt);
        window->show(); window->activateWindow();
        QVERIFY(QTest::qWaitForWindowActive(window));
        QTRY_COMPARE(view->url(), first);
        QTRY_VERIFY(!view->page()->isLoading());
        QTRY_VERIFY(view->hasFocus());
    }
    void profileAndBounds() {
        QVERIFY(!profile->isOffTheRecord());
        QCOMPARE(profile->persistentStoragePath(), qEnvironmentVariable("XDG_DATA_HOME")+"/omawser-qt");
        QCOMPARE(profile->cachePath(), qEnvironmentVariable("XDG_CACHE_HOME")+"/omawser-qt");
        QVERIFY(!prompt->isVisible());
        QCOMPARE(view->geometry(), QRect(1, 1, window->width()-2, window->height()-2));
        QCOMPARE(view->contextMenuPolicy(), Qt::NoContextMenu);
        QVERIFY(window->windowFlags().testFlag(Qt::FramelessWindowHint));
    }
    void promptKeyboard() {
        key(Qt::Key_L, Qt::ControlModifier);
        QTRY_VERIFY(prompt->hasFocus());
        QCOMPARE(prompt->selectedText(), first.toString());
        QCOMPARE(view->geometry().y(), 48);
        prompt->setText("javascript:alert(1)");
        key(Qt::Key_Return);
        QVERIFY(prompt->hasFocus()); QVERIFY(prompt->isVisible()); QCOMPARE(view->url(), first);
        prompt->setText("draft.example/path");
        QWidget other; other.show(); other.activateWindow();
        QVERIFY(QTest::qWaitForWindowActive(&other));
        window->activateWindow(); QVERIFY(QTest::qWaitForWindowActive(window));
        QTRY_VERIFY(prompt->hasFocus()); QCOMPARE(prompt->text(), QString("draft.example/path"));
        key(Qt::Key_Escape);
        QTRY_VERIFY(view->hasFocus()); QVERIFY(!prompt->isVisible());
        key(Qt::Key_L, Qt::ControlModifier);
        QTRY_VERIFY(prompt->hasFocus());
        prompt->setText(second.toString()); key(Qt::Key_Return);
        QTRY_COMPARE(view->url(), second); QTRY_VERIFY(!view->page()->isLoading());
        QTRY_VERIFY(view->hasFocus());
    }
    void webShortcuts() {
        key(Qt::Key_Plus, Qt::ControlModifier|Qt::ShiftModifier);
        QVERIFY(view->zoomFactor() > 1.0);
        key(Qt::Key_Minus, Qt::ControlModifier);
        QVERIFY(qAbs(view->zoomFactor()-1.0) < 0.001);
        key(Qt::Key_Equal, Qt::ControlModifier);
        key(Qt::Key_0, Qt::ControlModifier); QCOMPARE(view->zoomFactor(), 1.0);
        key(Qt::Key_Left, Qt::AltModifier); QTRY_COMPARE(view->url(), first);
        QTRY_VERIFY(!view->page()->isLoading());
        key(Qt::Key_Right, Qt::AltModifier); QTRY_COMPARE(view->url(), second);
        QTRY_VERIFY(!view->page()->isLoading());
        int previous = requests; key(Qt::Key_R, Qt::ControlModifier);
        QTRY_VERIFY(requests > previous); QTRY_VERIFY(!view->page()->isLoading());
        previous = requests; key(Qt::Key_F5); QTRY_VERIFY(requests > previous);
        QTRY_VERIFY(!view->page()->isLoading());
        key(Qt::Key_F11); QTRY_VERIFY(window->isFullScreen());
        key(Qt::Key_F11); QTRY_VERIFY(!window->isFullScreen());
        QTRY_VERIFY(view->hasFocus());
    }
    void loadFailureRetry() {
        const auto port = server.serverPort();
        server.close();
        key(Qt::Key_L, Qt::ControlModifier); QTRY_VERIFY(prompt->hasFocus());
        prompt->setText(second.toString()); key(Qt::Key_Return);
        QTRY_VERIFY(prompt->isVisible()); QTRY_VERIFY(prompt->hasFocus());
        QCOMPARE(prompt->text(), second.toString());
        QVERIFY(server.listen(QHostAddress::LocalHost, port));
        int previous = requests; key(Qt::Key_R, Qt::ControlModifier);
        QTRY_VERIFY(requests > previous); QTRY_VERIFY(!view->page()->isLoading());
        QTRY_VERIFY(!prompt->isVisible()); QTRY_VERIFY(view->hasFocus());
        QCOMPARE(view->url(), second);
    }
    void themeAndNavigationPolicy() {
        QString path = themePaths().first();
        QFile replacement(path+".next"); QVERIFY(replacement.open(QIODevice::WriteOnly));
        replacement.write("accent='#123456'\nbackground='#eeeeee'\nforeground='#111111'\nmode='light'\n"); replacement.close();
        QVERIFY(::rename(qPrintable(path+".next"), qPrintable(path)) == 0);
        QTRY_VERIFY(prompt->styleSheet().contains("#123456"));
        view->load(QUrl("file:///etc/hostname"));
        QTest::qWait(250); QCOMPARE(view->url(), second);
        view->page()->runJavaScript("window.open('http://127.0.0.1/denied')");
        QTest::qWait(250);
        int browsers = 0;
        for (auto *w : QApplication::topLevelWidgets()) if (dynamic_cast<Browser *>(w)) ++browsers;
        QCOMPARE(browsers, 1);
    }
    void cleanupTestCase() { delete window; window = nullptr; delete profile; profile = nullptr; }
};

static bool isolated() {
    for (const auto *name : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"}) {
        QString path = qEnvironmentVariable(name);
        QFileInfo info(path);
        if (!path.startsWith("/tmp/omawser-qt-test-") || !info.isDir()
            || info.canonicalFilePath() != path) return false;
        if (QByteArray(name) != "XDG_STATE_HOME" && !QDir(path).entryList(QDir::AllEntries|QDir::NoDotAndDotDot).isEmpty()) return false;
    }
    auto path = themePaths().first();
    return QFileInfo(path).isFile() && QFileInfo(path).canonicalFilePath() == path;
}
int main(int argc, char **argv) {
    if (!isolated()) { std::fputs("gui-test requires fresh isolated XDG directories and a regular theme fixture; see native/README.md\n", stderr); return 2; }
    QApplication app(argc, argv);
    app.setApplicationName("omawser-qt");
    QGuiApplication::setDesktopFileName("omawser-qt");
    GuiTest test;
    return QTest::qExec(&test, argc, argv);
}
#include "gui_test.moc"
