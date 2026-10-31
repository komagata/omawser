#include "core.h"
#include <QDir>
#include <QFile>
#include <QTemporaryDir>
#include <QtTest>
#include <unistd.h>
class CoreTest : public QObject {
    Q_OBJECT
    static void write(const QString &path, const QByteArray &data) {
        QDir().mkpath(QFileInfo(path).absolutePath());
        QFile f(path); QVERIFY(f.open(QIODevice::WriteOnly)); QCOMPARE(f.write(data), data.size());
    }
private slots:
    void urls() {
        QCOMPARE(address(" example.com ").toString(), QString("https://example.com"));
        QCOMPARE(address("http://127.0.0.1:123/a?q=b").scheme(), QString("http"));
        QCOMPARE(address("https://例.jp/日本").host(), QString::fromUtf8("例.jp"));
        for (const auto &s : {"", " ", "javascript:alert(1)", "file:///tmp/a", "data:text/html,x", "mailto:a@b", "https://", "https://bad host/", "http://x:99999", "https://x\n.com"})
            QVERIFY2(address(s).isEmpty(), s);
        QVERIFY(!httpUrl(QUrl("about:blank")));
    }
    void theme() {
        auto t = parseTheme("background = '#123456'\nforeground=\"#ABCDEF\"\nmode='light'\naccent='#xyzxyz'\nmuted='red'\nunknown='#123456'\n");
        QCOMPARE(t["background"], QString("#123456"));
        QCOMPARE(t["foreground"], QString("#ABCDEF"));
        QCOMPARE(t["accent"], QString("#7aa2f7"));
        QCOMPARE(t["muted"], QString("#565f89"));
        QCOMPARE(t["mode"], QString("light"));
        QCOMPARE(t.size(), 6);
        QCOMPARE(parseTheme("mode='invalid'")["mode"], QString("dark"));
    }
    void themeReplacements() {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        QString primary = dir.path()+"/state/omarchy/current/theme/colors.toml";
        QString fallback = dir.path()+"/config/colors.toml";
        write(fallback, "accent='#111111'\n");
        Theme seen;
        ThemeWatcher watcher({primary, fallback}, [&](const Theme &t) { seen = t; });
        QCOMPARE(seen["accent"], QString("#111111"));
        write(primary, "accent='#222222'\n");
        QTRY_COMPARE(seen["accent"], QString("#222222"));
        write(primary+".next", "accent='#333333'\n");
        QVERIFY(::rename(qPrintable(primary+".next"), qPrintable(primary)) == 0);
        QTRY_COMPARE(seen["accent"], QString("#333333"));
        QString parent = QFileInfo(primary).absolutePath();
        QVERIFY(QDir().rename(parent, parent+"-old"));
        write(primary, "accent='#444444'\n");
        QTRY_COMPARE(seen["accent"], QString("#444444"));
        QVERIFY(QDir(parent).removeRecursively());
        QTRY_COMPARE(seen["accent"], QString("#111111"));
        QString target = dir.path()+"/themes/other";
        write(target+"/colors.toml", "accent='#555555'\n");
        QVERIFY(::symlink(qPrintable(target), qPrintable(parent)) == 0);
        QTRY_COMPARE(seen["accent"], QString("#555555"));
        QString target2 = dir.path()+"/themes/next";
        write(target2+"/colors.toml", "accent='#666666'\n");
        QVERIFY(::symlink(qPrintable(target2), qPrintable(parent+".next")) == 0);
        QVERIFY(::rename(qPrintable(parent+".next"), qPrintable(parent)) == 0);
        QTRY_COMPARE(seen["accent"], QString("#666666"));
        write(target2+"/colors.toml.next", "accent='#777777'\n");
        QVERIFY(::rename(qPrintable(target2+"/colors.toml.next"), qPrintable(target2+"/colors.toml")) == 0);
        QTRY_COMPARE(seen["accent"], QString("#777777"));
    }
};
QTEST_GUILESS_MAIN(CoreTest)
#include "core_test.moc"
