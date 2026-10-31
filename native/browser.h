#pragma once
#include "core.h"
#include <QWidget>
class QLineEdit;
class QLabel;
class QWebEngineProfile;
class QWebEngineView;
class QKeyEvent;
QWebEngineProfile *makeProfile(QObject *parent);
class Browser : public QWidget {
public:
    Browser(QWebEngineProfile *profile, const QUrl &url = QUrl("https://x.com/home"));
protected:
    bool eventFilter(QObject *object, QEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;
    void paintEvent(QPaintEvent *event) override;
private:
    bool handles(const QKeyEvent *event) const;
    void command(const QKeyEvent *event);
    void layout();
    void focusContent();
    void openPrompt(const QString &message = {}, bool loadError = false);
    void closePrompt();
    void navigate(const QString &input);
    void applyTheme(const Theme &theme);
    QWebEngineProfile *profile;
    QWebEngineView *view;
    QLineEdit *prompt;
    QLabel *error;
    Theme palette;
    QUrl failedTarget;
    bool errorPrompt = false;
    bool composing = false;
};
