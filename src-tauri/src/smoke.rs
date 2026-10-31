//! Local, feature-gated test driver. No test IPC or remote commands exist in release builds.
use super::*;
use gdk::{keys::constants as k, ModifierType as M};
use glib::translate::*;
use javascriptcore::ValueExt;
use std::time::Duration;

pub fn validate_environment(target: &str) {
    // A panic inside a spawned GLib test future must fail the executable, not leave
    // a browser running until the outer harness deadline.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        std::process::exit(101);
    }));
    let root = PathBuf::from(std::env::var_os("OMA_SMOKE_ROOT").expect("smoke requires wrapper"));
    assert!(
        root.starts_with("/tmp")
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("omawser-tauri-smoke-")
    );
    assert_eq!(root.canonicalize().unwrap(), root);
    for kind in ["CONFIG", "DATA", "CACHE", "STATE"] {
        let path = PathBuf::from(
            std::env::var_os(format!("XDG_{kind}_HOME")).expect("isolated XDG required"),
        );
        assert_eq!(path, root.join(kind.to_lowercase()));
        assert_eq!(path.canonicalize().unwrap(), path);
    }
    assert_eq!(
        url::Url::parse(target).unwrap().host_str(),
        Some("127.0.0.1")
    );
}
async fn settle() {
    glib::timeout_future(Duration::from_millis(120)).await;
}
async fn wait(description: &str, predicate: impl Fn() -> bool) -> Result<(), String> {
    for _ in 0..100 {
        if predicate() {
            return Ok(());
        }
        settle().await;
    }
    Err(format!("timeout: {description}"))
}
fn require(value: bool, description: &str) -> Result<(), String> {
    if value {
        println!("PASS {description}");
        Ok(())
    } else {
        Err(description.into())
    }
}
async fn js(ui: &Ui, expression: &str) -> Result<bool, String> {
    ui.active()
        .web
        .evaluate_javascript_future(expression, None, None)
        .await
        .map(|v| v.to_boolean())
        .map_err(|e| e.to_string())
}
fn key(ui: &Ui, key: gdk::keys::Key, modifiers: M) {
    // Feed a native GDK event into GTK's normal window dispatch (including its focused
    // child and default key handler). This does not call Ui::key or DOM dispatchEvent.
    for kind in [gdk::EventType::KeyPress, gdk::EventType::KeyRelease] {
        let mut event = gdk::Event::new(kind);
        let window = ui.native.window().expect("mapped GDK window");
        let display = window.display();
        let hardware = gdk::Keymap::for_display(&display)
            .and_then(|map| map.entries_for_keyval(*key).first().map(|k| k.keycode()))
            .unwrap_or(0) as u16;
        unsafe {
            let raw: *mut gdk::ffi::GdkEvent = event.to_glib_none_mut().0;
            (*raw).key.window = window.to_glib_full();
            (*raw).key.send_event = 1;
            (*raw).key.time = 0;
            (*raw).key.keyval = *key;
            (*raw).key.state = modifiers.bits();
            (*raw).key.hardware_keycode = hardware;
        }
        event.set_device(display.default_seat().and_then(|s| s.keyboard()).as_ref());
        gtk::main_do_event(&mut event);
    }
}
fn link_click(ui: &Ui, button: u32, modifiers: M) {
    for kind in [gdk::EventType::ButtonPress, gdk::EventType::ButtonRelease] {
        let mut event = gdk::Event::new(kind);
        let window = ui.active().web.window().expect("mapped WebView");
        let display = window.display();
        unsafe {
            let raw: *mut gdk::ffi::GdkEvent = event.to_glib_none_mut().0;
            (*raw).button.window = window.to_glib_full();
            (*raw).button.send_event = 1;
            (*raw).button.time = 0;
            (*raw).button.x = 30.;
            (*raw).button.y = 110.;
            (*raw).button.button = button;
            (*raw).button.state = modifiers.bits();
        }
        event.set_device(display.default_seat().and_then(|s| s.pointer()).as_ref());
        gtk::main_do_event(&mut event);
    }
}
async fn background_links(ui: &Rc<Ui>, base: &str) -> Result<(), String> {
    navigate(ui, &format!("{base}/one")).await?;
    wait("link fixture", || {
        ui.active().web.title().as_deref() == Some("/one") && !ui.active().web.is_loading()
    })
    .await?;
    let original = ui.active();
    // Real GTK button dispatch reaches native NavigationAction; no DOM command IPC.
    for (button, modifiers) in [(1, M::CONTROL_MASK), (2, M::empty())] {
        link_click(ui, button, modifiers);
        wait("modified link creates a background tab", || {
            ui.tabs.borrow().len() == 2
        })
        .await?;
        let background = ui.tabs.borrow().items()[1].clone();
        wait("background link load", || {
            background.web.title().as_deref() == Some("/two") && !background.web.is_loading()
        })
        .await?;
        require(
            Rc::ptr_eq(&ui.active(), &original)
                && original.web.title().as_deref() == Some("/one")
                && original.web.has_focus(),
            "modified link preserves foreground page and focus",
        )?;
        require(
            background.web.user_content_manager().is_some(),
            "background tab is a native WebView",
        )?;
        ui.close_tab(1);
    }
    ui.show();
    original.entry.set_text("background creation draft");
    original.entry.select_region(2, 8);
    ui.add_tab(&format!("{base}/two"), false, false)
        .map_err(|e| e.to_string())?;
    wait("background tab with native prompt", || {
        !ui.tabs.borrow().items()[1].web.is_loading()
    })
    .await?;
    require(
        original.entry.has_focus()
            && original.entry.text() == "background creation draft"
            && original.entry.selection_bounds() == Some((2, 8)),
        "background creation preserves address draft selection and focus",
    )?;
    ui.hide();
    let names = ui.notebook.accessible().map(|a| {
        (0..a.n_accessible_children())
            .map(|i| {
                a.ref_accessible_child(i)
                    .and_then(|c| c.name())
                    .map(|s| s.to_string())
            })
            .collect::<Vec<_>>()
    });
    ui.notebook.grab_focus();
    key(ui, k::Right, M::empty());
    settle().await;
    require(
        ui.notebook.has_focus() && ui.tabs.borrow().active_index() == Some(1),
        "keyboard tabstrip arrows retain tabstrip focus",
    )?;
    require(
        names == Some(vec![Some("/one".into()), Some("/two".into())]),
        "native accessibility tree names every tab",
    )?;
    key(ui, k::Left, M::empty());
    settle().await;
    require(
        ui.notebook.has_focus() && ui.tabs.borrow().active_index() == Some(0),
        "consecutive tabstrip arrows retain focus",
    )?;
    ui.active().web.reload();
    settle().await;
    wait("header-focused page reload", || {
        !ui.active().web.is_loading()
    })
    .await?;
    require(
        ui.notebook.has_focus(),
        "page load does not steal tabstrip keyboard focus",
    )?;
    ui.native.emit_by_name::<bool>(
        "focus-in-event",
        &[&gdk::Event::new(gdk::EventType::FocusChange)],
    );
    settle().await;
    require(
        ui.notebook.has_focus(),
        "native refocus preserves tabstrip keyboard focus",
    )?;
    key(ui, k::Tab, M::empty());
    settle().await;
    require(ui.active().web.has_focus(), "Tab leaves tabstrip for page")?;

    ui.close_tab(1);
    ui.hide();
    Ok(())
}
async fn navigate(ui: &Ui, url: &str) -> Result<(), String> {
    key(ui, k::l, M::CONTROL_MASK);
    require(
        ui.active().prompt.borrow().open && ui.active().entry.has_focus(),
        "Ctrl+L opens native entry before page key handlers",
    )?;
    ui.active().entry.set_text(url);
    key(ui, k::Return, M::empty());
    settle().await;
    Ok(())
}
async fn run(mut ui: Rc<Ui>, target: String) -> Result<(), String> {
    wait("first load", || {
        !ui.active().web.is_loading() && ui.active().web.title().as_deref() == Some("/one")
    })
    .await?;
    settle().await;
    if std::env::var("OMA_SMOKE_PHASE").as_deref() == Ok("persistence") {
        require(js(&ui, "document.cookie.includes('oma_smoke=present') && localStorage.getItem('oma_smoke') === 'present'").await?, "cookies and localStorage survive process restart")?;
        println!("OMA_SMOKE_OK");
        return Ok(());
    }
    require(ui.active().web.has_focus(), "initial native WebView focus")?;
    require(
        js(&ui, "document.hasFocus()").await?,
        "initial document focus",
    )?;
    require(js(&ui, "!window.__TAURI__ && !window.__TAURI_INTERNALS__ && !window.ipc && !(window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc)").await?, "remote Tauri bootstrap and message endpoint absent")?;
    let data = ui.active().web.website_data_manager().unwrap();
    use webkit2gtk::WebsiteDataManagerExt;
    require(
        data.base_data_directory().as_deref()
            == Some(
                xdg("XDG_DATA_HOME", ".local/share")
                    .join("omawser/webkit")
                    .to_str()
                    .unwrap(),
            ),
        "independent persistent data directory",
    )?;
    key(&ui, k::l, M::CONTROL_MASK);
    settle().await;
    require(
        ui.active().entry.has_focus() && ui.active().strip.is_visible(),
        "native address focus",
    )?;
    ui.active().entry.set_text("draft to preserve");
    ui.native.emit_by_name::<bool>(
        "focus-in-event",
        &[&gdk::Event::new(gdk::EventType::FocusChange)],
    );
    settle().await;
    require(
        ui.active().entry.text() == "draft to preserve",
        "focus preserves draft",
    )?;
    key(&ui, k::Escape, M::empty());
    settle().await;
    require(
        !ui.active().strip.is_visible() && ui.active().web.has_focus(),
        "Escape closes entry and refocuses page",
    )?;
    key(&ui, k::Escape, M::empty());
    settle().await;
    require(
        js(&ui, "escapeCount === 1").await?,
        "idle Escape reaches website",
    )?;
    navigate(&ui, "javascript:alert(1)").await?;
    require(
        ui.active().prompt.borrow().open && !ui.active().prompt.borrow().error.is_empty(),
        "unsafe address stays in native prompt",
    )?;
    let base = target.trim_end_matches("/one");
    navigate(&ui, &format!("{base}/two")).await?;
    wait("address Enter", || {
        ui.active().web.title().as_deref() == Some("/two") && !ui.active().web.is_loading()
    })
    .await?;
    require(!ui.active().strip.is_visible(), "valid Enter hides entry")?;
    key(&ui, k::Left, M::MOD1_MASK);
    wait("back", || {
        ui.active().web.title().as_deref() == Some("/one") && !ui.active().web.is_loading()
    })
    .await?;
    key(&ui, k::Right, M::MOD1_MASK);
    wait("forward", || {
        ui.active().web.title().as_deref() == Some("/two") && !ui.active().web.is_loading()
    })
    .await?;
    key(&ui, k::equal, M::CONTROL_MASK);
    require(ui.active().web.zoom_level() > 1.0, "zoom in")?;
    key(&ui, k::minus, M::CONTROL_MASK);
    key(&ui, k::_0, M::CONTROL_MASK);
    require(ui.active().web.zoom_level() == 1.0, "zoom reset")?;
    key(&ui, k::r, M::CONTROL_MASK);
    settle().await;
    wait("reload", || !ui.active().web.is_loading()).await?;
    navigate(&ui, &format!("{base}/retry")).await?;
    wait("load failure prompt", || {
        ui.active().prompt.borrow().failed_target.is_some()
    })
    .await?;
    require(
        ui.active().strip.is_visible(),
        "main frame error opens prompt",
    )?;
    ui.active().entry.set_text("keep this draft");
    key(&ui, k::r, M::CONTROL_MASK);
    settle().await;
    wait("failed retry", || !ui.active().web.is_loading()).await?;
    require(
        ui.active().entry.text() == "keep this draft",
        "failed retry preserves draft",
    )?;
    let root = PathBuf::from(std::env::var_os("OMA_SMOKE_ROOT").unwrap());
    std::fs::write(root.join("recover"), "ok").map_err(|e| e.to_string())?;
    key(&ui, k::r, M::CONTROL_MASK);
    wait("successful retry", || {
        ui.active().web.title().as_deref() == Some("/retry") && !ui.active().web.is_loading()
    })
    .await?;
    require(
        !ui.active().strip.is_visible() && ui.active().web.has_focus(),
        "successful retry hides error and focuses page",
    )?;
    let colors = theme_paths()[0].clone();
    let next = colors.with_extension("next");
    std::fs::write(&next, "accent = '#abcdef'\nmode = 'dark'\n").map_err(|e| e.to_string())?;
    std::fs::rename(&next, &colors).map_err(|e| e.to_string())?;
    wait("theme atomic refresh", || {
        let css = ui.css.to_string();
        css.contains("#abcdef") || css.contains("rgb(171,205,239)")
    })
    .await?;
    let frame = ui.window.default_vbox().unwrap();
    let border = frame.style_context().border(gtk::StateFlags::NORMAL);
    require(
        border.left == 0 && border.top == 0 && border.right == 0 && border.bottom == 0,
        "no outer CSS border",
    )?;
    key(&ui, k::l, M::CONTROL_MASK);
    settle().await;
    let address_border = ui
        .active()
        .entry
        .style_context()
        .border(gtk::StateFlags::NORMAL);
    require(
        address_border.left == 1
            && address_border.top == 1
            && address_border.right == 1
            && address_border.bottom == 1,
        "address entry retains one pixel border after Ctrl+L",
    )?;
    key(&ui, k::Escape, M::empty());
    settle().await;
    require(
        js(&ui, "matchMedia('(prefers-color-scheme: dark)').matches").await?,
        "theme dark preference reaches WebKit",
    )?;
    for forbidden in [
        "file:///etc/passwd",
        "tauri://localhost/index.html",
        "asset://localhost/etc/passwd",
        "data:text/html,blocked",
        "ipc://localhost/plugin:window|close",
    ] {
        ui.active().web.load_uri(forbidden);
        settle().await;
        require(
            ui.active()
                .web
                .uri()
                .as_deref()
                .is_none_or(navigation_allowed),
            "native policy rejects local/custom schemes",
        )?;
    }
    navigate(&ui, &format!("{base}/redirect")).await?;
    settle().await;
    require(
        ui.active()
            .web
            .uri()
            .as_deref()
            .is_none_or(navigation_allowed),
        "HTTP redirect cannot reach file scheme",
    )?;
    // New-window callback creates a live foreground tab without an opener relationship.
    ui.active()
        .web
        .evaluate_javascript_future(&format!("window.open('{base}/two'); true"), None, None)
        .await
        .map_err(|e| e.to_string())?;
    wait("safe popup", || ui.tabs.borrow().len() == 2).await?;
    wait("popup loaded", || {
        ui.active().web.title().as_deref() == Some("/two")
    })
    .await?;
    require(
        js(&ui, "window.opener === null").await?,
        "popup tab has no opener",
    )?;
    key(&ui, k::w, M::CONTROL_MASK);
    wait("popup cleanup", || ui.tabs.borrow().len() == 1).await?;
    background_links(&ui, base).await?;
    tab_checks(&ui, base).await?;
    // Native permission policy must deny actual requests from a loopback secure context.
    navigate(&ui, &format!("{base}/one")).await?;
    wait("permission fixture", || {
        ui.active().web.title().as_deref() == Some("/one") && !ui.active().web.is_loading()
    })
    .await?;
    ui.active().web.evaluate_javascript_future("window.geoResult = 'pending'; navigator.geolocation.getCurrentPosition(() => geoResult = 'allowed', e => geoResult = String(e.code)); true", None, None).await.map_err(|e| e.to_string())?;
    let mut denied = false;
    for _ in 0..30 {
        if js(&ui, "geoResult === '1'").await? {
            denied = true;
            break;
        }
        settle().await;
    }
    require(denied, "native geolocation permission denied")?;
    download_checks(&ui, &root, base, false).await?;
    std::fs::remove_file(root.join("saved.txt")).map_err(|e| e.to_string())?;
    key(&ui, k::n, M::CONTROL_MASK);
    wait("surviving window", || {
        ui.window.app_handle().windows().len() == 2
    })
    .await?;
    let survivor = WINDOWS
        .with(|windows| {
            windows
                .borrow()
                .iter()
                .filter_map(std::rc::Weak::upgrade)
                .find(|other| other.window.label() != ui.window.label())
        })
        .ok_or("survivor UI absent")?;
    wait("survivor mapped and loaded", || {
        survivor.native.is_mapped()
            && !survivor.active().web.is_loading()
            && survivor.active().web.title().as_deref() == Some("/one")
    })
    .await?;
    key(&ui, k::W, M::CONTROL_MASK | M::SHIFT_MASK);
    wait("original window destroyed", || ui.destroyed.get()).await?;
    require(
        ui.window.app_handle().windows().len() == 1,
        "first window closes while sibling survives",
    )?;
    ui = survivor;
    download_checks(&ui, &root, base, true).await?;
    require(js(&ui, "document.cookie = 'oma_smoke=present; Max-Age=3600; SameSite=Lax'; localStorage.setItem('oma_smoke', 'present'); true").await?, "persistent state written to isolated profile")?;
    settle().await;
    println!("OMA_SMOKE_OK");
    Ok(())
}
async fn download_checks(
    ui: &Rc<Ui>,
    root: &PathBuf,
    base: &str,
    via_navigation: bool,
) -> Result<(), String> {
    for save in [false, true] {
        let chooser_root = root.clone();
        let choice = glib::MainContext::default().spawn_local(async move {
            wait("native download chooser", || {
                gtk::Window::list_toplevels()
                    .iter()
                    .any(|w| w.is::<gtk::FileChooserDialog>())
            })
            .await?;
            let dialog = gtk::Window::list_toplevels()
                .into_iter()
                .find_map(|w| w.downcast::<gtk::FileChooserDialog>().ok())
                .unwrap();
            require(
                dialog.set_current_folder(&chooser_root),
                "download chooser accepts isolated folder",
            )?;
            dialog.set_current_name("saved.txt");
            let expected = chooser_root.join("saved.txt");
            wait("download chooser selected folder and filename", || {
                dialog.current_folder().as_deref() == Some(chooser_root.as_path())
                    && dialog.filename().as_deref() == Some(expected.as_path())
            })
            .await?;
            if save {
                dialog.response(gtk::ResponseType::Accept);
            } else {
                dialog.response(gtk::ResponseType::Cancel);
            }
            Ok::<(), String>(())
        });
        if via_navigation {
            ui.active().web.load_uri(&format!("{base}/download"));
        } else {
            ui.active().web.download_uri(&format!("{base}/download"));
        }
        choice.await.map_err(|e| e.to_string())??;
        settle().await;
        if save {
            wait("download bytes", || root.join("saved.txt").exists()).await?;
            require(
                std::fs::read(root.join("saved.txt")).map_err(|e| e.to_string())?
                    == b"oma fixture download\n",
                "native download destination and contents",
            )?;
        } else {
            require(
                !root.join("saved.txt").exists()
                    && !root.join("fixture.txt").exists()
                    && !PathBuf::from(std::env::var_os("XDG_DOWNLOAD_DIR").unwrap())
                        .join("fixture.txt")
                        .exists()
                    && !PathBuf::from(std::env::var_os("HOME").unwrap())
                        .join("Downloads/fixture.txt")
                        .exists(),
                "download cancellation writes no file to isolated destinations",
            )?;
        }
    }
    Ok(())
}
async fn tab_checks(ui: &Rc<Ui>, base: &str) -> Result<(), String> {
    let first = ui.active();
    require(!ui.notebook.shows_tabs(), "one tab has no tab header")?;
    first.web.evaluate_javascript_future("document.querySelector('input').value = 'retained form'; window.retainedObject = {}; true", None, None).await.map_err(|e| e.to_string())?;
    first.web.set_zoom_level(1.44);
    key(ui, k::l, M::CONTROL_MASK);
    first.entry.set_text("first tab draft");
    // Shifted uppercase keyvals and ISO_Left_Tab are what real GDK layouts emit.
    key(ui, k::t, M::CONTROL_MASK);
    require(
        ui.tabs.borrow().len() == 2 && ui.notebook.shows_tabs(),
        "Ctrl+T shows two-tab strip",
    )?;
    require(
        ui.active().entry.has_focus() && ui.active().entry.selection_bounds().is_some(),
        "new tab selects address",
    )?;
    navigate(ui, &format!("{base}/two")).await?;
    wait("second tab", || {
        ui.active().web.title().as_deref() == Some("/two") && !ui.active().web.is_loading()
    })
    .await?;
    let second = ui.active();
    key(ui, k::ISO_Left_Tab, M::CONTROL_MASK | M::SHIFT_MASK);
    require(
        Rc::ptr_eq(&ui.active(), &first) && first.entry.text() == "first tab draft",
        "previous tab restores draft",
    )?;
    key(ui, k::Escape, M::empty());
    require(
        js(
            ui,
            "document.querySelector('input').value === 'retained form' && !!retainedObject",
        )
        .await?
            && first.web.zoom_level() == 1.44,
        "switch preserves live form JS and zoom",
    )?;
    key(ui, k::Page_Up, M::CONTROL_MASK);
    require(Rc::ptr_eq(&ui.active(), &second), "previous wraps")?;
    key(ui, k::Page_Down, M::CONTROL_MASK);
    require(Rc::ptr_eq(&ui.active(), &first), "next wraps")?;
    key(ui, k::_9, M::CONTROL_MASK);
    require(Rc::ptr_eq(&ui.active(), &second), "Ctrl+9 selects last")?;
    key(ui, k::_8, M::CONTROL_MASK);
    require(
        Rc::ptr_eq(&ui.active(), &second),
        "out-of-range direct selection is ignored",
    )?;
    key(ui, k::_1, M::CONTROL_MASK);
    require(Rc::ptr_eq(&ui.active(), &first), "Ctrl+1 selects first")?;
    key(ui, k::Tab, M::CONTROL_MASK);
    require(Rc::ptr_eq(&ui.active(), &second), "Ctrl+Tab selects next")?;
    // Tauri window commands are queued; establish the active title before testing
    // whether a later background callback changes it.
    wait("active native title after switch", || {
        ui.native.title().as_deref() == Some("/two — Omawser")
    })
    .await?;
    // A background failure updates its own prompt without changing active focus/title.
    first.web.load_uri(&format!("{base}/always-fail"));
    wait("background error", || {
        first.prompt.borrow().failed_target.as_deref()
            == Some(format!("{base}/always-fail").as_str())
    })
    .await?;
    require(
        second.web.has_focus()
            && !second.prompt.borrow().open
            && ui.native.title().as_deref() == Some("/two — Omawser"),
        "background error does not steal active UI",
    )?;
    // Close the inactive tab through the actual close-button callback.
    first.close.emit_clicked();
    require(
        Rc::ptr_eq(&ui.active(), &second) && !ui.notebook.shows_tabs(),
        "close inactive preserves page and hides strip",
    )?;
    key(ui, k::T, M::CONTROL_MASK | M::SHIFT_MASK);
    require(ui.tabs.borrow().len() == 2, "Ctrl+Shift+T reopens URL")?;
    wait("reopened error", || {
        ui.active().prompt.borrow().failed_target.is_some()
    })
    .await?;
    navigate(ui, &format!("{base}/one")).await?;
    wait("reopened tab fixture", || {
        ui.active().web.title().as_deref() == Some("/one") && !ui.active().web.is_loading()
    })
    .await?;
    require(js(ui, "!window.__TAURI_INTERNALS__ && !window.ipc && !(window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc)").await?, "reopened tab has no privileged endpoint")?;
    ui.active().web.load_uri("file:///etc/passwd");
    settle().await;
    require(
        ui.active()
            .web
            .uri()
            .as_deref()
            .is_none_or(navigation_allowed),
        "reopened tab rejects local navigation",
    )?;
    key(ui, k::w, M::CONTROL_MASK);
    require(
        Rc::ptr_eq(&ui.active(), &second),
        "close active restores neighbor",
    )?;
    require(js(ui, "!window.__TAURI_INTERNALS__ && !window.ipc && !(window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc)").await?, "new tab has no privileged endpoint")?;
    // Repeat creation/closure to exercise Tauri and GTK removal, with no hidden extra page.
    for _ in 0..4 {
        ui.add_tab(&format!("{base}/one"), false, false)
            .map_err(|e| e.to_string())?;
        let last = ui.tabs.borrow().len() - 1;
        ui.close_tab(last);
    }
    wait("closed native views", || {
        ui.window.webviews().len() == 1 && ui.notebook.n_pages() == 1
    })
    .await?;
    require(
        ui.window.webviews().len() == 1 && ui.notebook.n_pages() == 1,
        "closed tabs release runtime views and native pages",
    )?;
    for whole_window in [true, false] {
        key(ui, k::n, M::CONTROL_MASK);
        wait("Ctrl+N creates independent window", || {
            ui.window.app_handle().windows().len() == 2
        })
        .await?;
        let other = WINDOWS
            .with(|windows| {
                windows
                    .borrow()
                    .iter()
                    .filter_map(std::rc::Weak::upgrade)
                    .find(|w| w.window.label() != ui.window.label())
            })
            .ok_or("new window UI absent")?;
        wait("new window mapped and fixture loaded", || {
            other.native.is_mapped()
                && other.native.window().is_some()
                && !other.active().web.is_loading()
                && other.active().web.title().as_deref() == Some("/one")
        })
        .await?;
        require(
            other.tabs.borrow().len() == 1 && !other.notebook.shows_tabs(),
            "new window begins with independent one-tab UI",
        )?;
        if whole_window {
            key(&other, k::W, M::CONTROL_MASK | M::SHIFT_MASK);
        } else {
            key(&other, k::w, M::CONTROL_MASK);
        }
        wait("window closes through native shortcut", || {
            ui.window.app_handle().windows().len() == 1
        })
        .await?;
        require(
            other.destroyed.get(),
            if whole_window {
                "Ctrl+Shift+W destroys window"
            } else {
                "Ctrl+W on last tab destroys window"
            },
        )?;
    }

    second.web.set_zoom_level(1.0);
    Ok(())
}
pub fn start(ui: Rc<Ui>, target: String) {
    glib::MainContext::default().spawn_local(async move {
        let app = ui.window.app_handle().clone();
        match run(ui, target).await {
            Ok(()) => app.exit(0),
            Err(e) => {
                eprintln!("OMA_SMOKE_FAILED: {e}");
                app.exit(1);
            }
        }
    });
}
