use omawser::{address, navigation_allowed, theme, Prompt};

#[test]
fn normalizes_web_addresses() {
    assert_eq!(address(" x.com/home "), Some("https://x.com/home".into()));
    assert_eq!(
        address("http://localhost:8000"),
        Some("http://localhost:8000/".into())
    );
    assert_eq!(
        address("HTTPS://EXAMPLE.COM"),
        Some("https://example.com/".into())
    );
}
#[test]
fn rejects_executable_local_and_obfuscated_schemes() {
    for input in [
        "",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,hi",
        "tauri://localhost",
        "asset://localhost/a",
        "about:blank",
        "blob:https://example.com/id",
        "http://",
        "java\nscript:alert(1)",
    ] {
        assert_eq!(address(input), None, "{input}");
        assert!(!navigation_allowed(input), "{input}");
    }
    assert!(!navigation_allowed("example.com"));
    assert!(navigation_allowed("https://example.com/"));
}
#[test]
fn parses_only_safe_semantic_theme_values() {
    let p = theme("background = '#000000'\nforeground = \"#e0ffd7\"\naccent = \"#91ff70\"\nmuted = \"bad; color:red\"\nmode = \"light\"");
    assert_eq!(p.background, "#000000");
    assert_eq!(p.foreground, "#e0ffd7");
    assert_eq!(p.accent, "#91ff70");
    assert_eq!(p.muted, "#565f89");
    assert_eq!(p.mode, "light");
    assert_eq!(theme("mode = 'bad'").mode, "dark");
}
#[test]
fn validation_keeps_draft_and_retry_uses_failed_target() {
    let mut p = Prompt::default();
    p.failed("http://localhost:9/retry", "connection refused");
    assert!(p.open);
    assert_eq!(p.draft, "http://localhost:9/retry");
    p.draft = "javascript:bad".into();
    assert_eq!(p.submit(), None);
    assert!(p.open);
    assert_eq!(p.draft, "javascript:bad");
    assert_eq!(
        p.retry_target().as_deref(),
        Some("http://localhost:9/retry")
    );
    p.failed("http://localhost:9/retry", "still refused");
    assert_eq!(p.draft, "javascript:bad");
    p.succeeded();
    assert!(!p.open);
    assert!(p.error.is_empty());
    assert_eq!(p.failed_target, None);
}
#[test]
fn focus_show_preserves_draft_and_success_does_not_dismiss_manual_prompt() {
    let mut p = Prompt::default();
    p.show("https://example.com/");
    p.draft = "my draft".into();
    p.show("https://other.example/");
    assert_eq!(p.draft, "my draft");
    p.succeeded();
    assert!(p.open);
    p.hide();
    assert!(!p.open);
    p.show("https://example.com/");
    p.draft = "http://localhost:8000".into();
    assert_eq!(p.submit().as_deref(), Some("http://localhost:8000/"));
    assert!(!p.open);
}

#[test]
fn escape_keeps_retry_target_but_success_clears_it() {
    let mut p = Prompt::default();
    p.failed("https://example.com/", "offline");
    p.hide();
    assert_eq!(p.retry_target().as_deref(), Some("https://example.com/"));
    p.succeeded();
    assert_eq!(p.retry_target(), None);
}
