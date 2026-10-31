use omawser::tabs::Tabs;

#[test]
fn closing_inactive_preserves_active_identity() {
    let mut tabs = Tabs::default();
    tabs.push("a", true);
    tabs.push("b", true);
    tabs.push("c", false);
    assert_eq!(tabs.active(), Some(&"b"));
    assert_eq!(tabs.close(0, "https://a/"), Some("a"));
    assert_eq!(tabs.active(), Some(&"b"));
    assert_eq!(tabs.active_index(), Some(0));
    tabs.close(1, "https://c/");
    assert_eq!(tabs.active(), Some(&"b"));
}

#[test]
fn closing_active_selects_right_then_left_and_last_is_empty() {
    let mut tabs = Tabs::default();
    for value in ["a", "b", "c"] {
        tabs.push(value, true);
    }
    tabs.select(1);
    tabs.close(1, "https://b/");
    assert_eq!(tabs.active(), Some(&"c"));
    tabs.close(1, "https://c/");
    assert_eq!(tabs.active(), Some(&"a"));
    tabs.close(0, "https://a/");
    assert_eq!(tabs.active_index(), None);
    assert!(tabs.is_empty());
    assert_eq!(tabs.close(0, "https://invalid/"), None);
}

#[test]
fn selection_wraps_and_out_of_range_is_noop() {
    let mut tabs = Tabs::default();
    tabs.cycle(-1);
    for value in 0..3 {
        tabs.push(value, false);
    }
    tabs.cycle(-1);
    assert_eq!(tabs.active(), Some(&2));
    tabs.cycle(1);
    assert_eq!(tabs.active(), Some(&0));
    tabs.select(8);
    assert_eq!(tabs.active(), Some(&0));
    tabs.select(1);
    assert_eq!(tabs.active(), Some(&1));
}

#[test]
fn reopen_is_url_only_lifo_bounded_and_rejects_unsafe_urls() {
    let mut tabs = Tabs::default();
    for i in 0..23 {
        tabs.push(i, true);
        tabs.close(0, &format!("https://example.com/{i}"));
    }
    tabs.push(100, true);
    tabs.close(0, "javascript:alert(1)");
    for i in (3..23).rev() {
        assert_eq!(tabs.reopen(), Some(format!("https://example.com/{i}")));
    }
    assert_eq!(tabs.reopen(), None);
}
