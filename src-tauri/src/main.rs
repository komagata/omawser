#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gtk::{gdk, glib, prelude::*};
use omawser::{
    address, navigation_allowed, tabs::Tabs, theme, theme_watch::ThemeWatch, Palette, Prompt,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};
use tauri::{webview::WebviewBuilder, Manager, WebviewUrl, WindowBuilder};
use webkit2gtk::{
    DownloadExt, NavigationPolicyDecisionExt, PermissionRequestExt, PolicyDecisionExt,
    ResponsePolicyDecisionExt, SettingsExt, URIRequestExt, URIResponseExt, UserContentManagerExt,
    WebContextExt, WebViewExt,
};

#[cfg(feature = "gui-smoke")]
mod smoke;
const HOME_URL: &str = "https://x.com/home";
static NEXT_WINDOW: AtomicUsize = AtomicUsize::new(1);
thread_local! {
    // Weak entries avoid keeping native widgets alive after their window is destroyed.
    static WINDOWS: RefCell<Vec<std::rc::Weak<Ui>>> = const { RefCell::new(Vec::new()) };
    static DOWNLOAD_CONTEXTS: RefCell<Vec<DownloadContext>> = const { RefCell::new(Vec::new()) };
}
fn xdg(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(name)
        .filter(|v| PathBuf::from(v).is_absolute())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("HOME is required")).join(fallback)
        })
}
fn theme_paths() -> Vec<PathBuf> {
    vec![
        xdg("XDG_STATE_HOME", ".local/state").join("omarchy/current/theme/colors.toml"),
        xdg("XDG_CONFIG_HOME", ".config").join("omarchy/current/theme/colors.toml"),
    ]
}
fn palette() -> Palette {
    theme(
        &theme_paths()
            .iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default(),
    )
}
fn download_owner(
    context: &webkit2gtk::WebContext,
    download: &webkit2gtk::Download,
) -> Option<Rc<Ui>> {
    let windows = WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .filter(|ui| {
                !ui.destroyed.get()
                    && ui
                        .tabs
                        .borrow()
                        .items()
                        .iter()
                        .any(|t| t.web.context().as_ref() == Some(context))
            })
            .collect::<Vec<_>>()
    });
    if let Some(origin) = download.web_view() {
        return windows
            .into_iter()
            .find(|ui| ui.tabs.borrow().items().iter().any(|t| t.web == origin));
    }
    // WebContext/WebView.download_uri can omit its origin. Never guess between
    // inactive windows; a unique active owner or sole matching window is required.
    let active = windows
        .iter()
        .filter(|ui| ui.native.is_active())
        .collect::<Vec<_>>();
    if active.len() == 1 {
        return Some(active[0].clone());
    }
    if windows.len() == 1 {
        return windows.into_iter().next();
    }
    None
}
fn download_allowed(download: &webkit2gtk::Download) -> bool {
    download
        .request()
        .and_then(|r| r.uri())
        .as_deref()
        .is_some_and(navigation_allowed)
        && download
            .response()
            .and_then(|r| r.uri())
            .as_deref()
            .is_some_and(navigation_allowed)
}
struct DownloadContext {
    context: glib::WeakRef<webkit2gtk::WebContext>,
    handler: glib::SignalHandlerId,
}
// Even with no Tauri on_download callback, pinned Wry 0.57 registers an
// auto-accept download-started hook for EVERY child. Remove just that new hook,
// while keeping our one trusted hook and every unrelated context signal intact.
fn remove_default_download(
    context: &webkit2gtk::WebContext,
    ours: Option<&glib::SignalHandlerId>,
) -> bool {
    use glib::{gobject_ffi as ffi, translate::IntoGlib};
    if let Some(ours) = ours {
        context.block_signal(ours);
    }
    let valid = unsafe {
        let instance = context.as_ptr().cast::<ffi::GObject>();
        let signal = ffi::g_signal_lookup(
            b"download-started\0".as_ptr().cast(),
            context.type_().into_glib(),
        );
        let find = || {
            ffi::g_signal_handler_find(
                instance,
                ffi::G_SIGNAL_MATCH_ID | ffi::G_SIGNAL_MATCH_UNBLOCKED,
                signal,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let first = if signal != 0 { find() } else { 0 };
        if first != 0 {
            ffi::g_signal_handler_block(instance, first);
        }
        if first != 0 && find() == 0 {
            ffi::g_signal_handler_disconnect(instance, first);
            true
        } else {
            // Unknown wiring: block download-started handlers on this context
            // and install an unconditional cancellation hook. No remote new tab.
            if signal != 0 {
                ffi::g_signal_handlers_block_matched(
                    instance,
                    ffi::G_SIGNAL_MATCH_ID,
                    signal,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
            false
        }
    };
    if valid {
        if let Some(ours) = ours {
            context.unblock_signal(ours);
        }
    } else {
        context.connect_download_started(|_, download| download.cancel());
        eprintln!("refusing tab: unexpected Wry download-started handlers");
    }
    valid
}
fn install_downloads(web: &webkit2gtk::WebView) -> bool {
    let Some(context) = web.context() else {
        return false;
    };
    let existing = DOWNLOAD_CONTEXTS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        contexts.retain(|record| record.context.upgrade().is_some());
        contexts
            .iter()
            .find(|record| record.context.upgrade().as_ref() == Some(&context))
            .map(|record| remove_default_download(&context, Some(&record.handler)))
    });
    if let Some(valid) = existing {
        return valid;
    }
    if !remove_default_download(&context, None) {
        return false;
    }
    // Context callbacks never retain a tab or a window and survive their closure.
    let handler = context.connect_download_started(|context, download| {
        let context = context.downgrade();
        download.connect_decide_destination(move |download, suggested| {
            let owner = context
                .upgrade()
                .and_then(|context| download_owner(&context, download));
            let Some(owner) = owner.filter(|_| download_allowed(download)) else {
                download.cancel();
                return true;
            };
            let parent = owner.native.clone();
            let weak_owner = Rc::downgrade(&owner);
            drop(owner);
            let chooser = gtk::FileChooserDialog::new(
                Some("Save download"),
                Some(&parent),
                gtk::FileChooserAction::Save,
            );
            chooser.set_destroy_with_parent(true);
            chooser.add_buttons(&[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Save", gtk::ResponseType::Accept),
            ]);
            chooser.set_do_overwrite_confirmation(true);
            if let Some(folder) = std::env::var_os("XDG_DOWNLOAD_DIR")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| glib::user_special_dir(glib::UserDirectory::Downloads))
                .or_else(|| std::env::current_dir().ok())
            {
                let _ = chooser.set_current_folder(folder);
            }
            // Suggested names are filenames, never a destination path supplied by a site.
            let basename = std::path::Path::new(suggested)
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| !n.is_empty())
                .unwrap_or("download");
            chooser.set_current_name(basename);
            let accepted = chooser.run() == gtk::ResponseType::Accept;
            let destination = chooser.filename().filter(|p| p.is_absolute());
            chooser.close();
            let owner_live = weak_owner.upgrade().is_some_and(|ui| {
                !ui.destroyed.get()
                    && download.web_view().is_none_or(|origin| {
                        ui.tabs.borrow().items().iter().any(|t| t.web == origin)
                    })
            });
            if accepted && owner_live && download_allowed(download) {
                if let Some(path) = destination.and_then(|path| path.to_str().map(str::to_owned)) {
                    download.set_allow_overwrite(true);
                    download.set_destination(&path);
                    return true;
                }
            }
            download.cancel();
            true
        });
    });
    DOWNLOAD_CONTEXTS.with(|contexts| {
        contexts.borrow_mut().push(DownloadContext {
            context: context.downgrade(),
            handler,
        })
    });
    true
}
// Wry 0.57 (locked through Tauri 2.12) installs one NavigationAction handler
// that consumes the signal before later handlers can inspect native modifiers.
// Own only this instance's decide-policy signal; keep all other Wry hooks intact.
fn own_navigation_policy(web: &webkit2gtk::WebView) -> bool {
    use glib::{gobject_ffi as ffi, translate::IntoGlib};
    unsafe {
        let signal =
            ffi::g_signal_lookup(b"decide-policy\0".as_ptr().cast(), web.type_().into_glib());
        if signal == 0 {
            return false;
        }
        let instance = web.as_ptr().cast::<ffi::GObject>();
        let count = ffi::g_signal_handlers_block_matched(
            instance,
            ffi::G_SIGNAL_MATCH_ID,
            signal,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if count != 1 {
            eprintln!("refusing tab: expected one Wry decide-policy handler, found {count}");
            return false;
        }
        ffi::g_signal_handlers_disconnect_matched(
            instance,
            ffi::G_SIGNAL_MATCH_ID,
            signal,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 1
    }
}
struct Tab {
    handle: tauri::Webview,
    web: webkit2gtk::WebView,
    page: gtk::Box,
    label: gtk::Label,
    close: gtk::Button,
    strip: gtk::Box,
    entry: gtk::Entry,
    error: gtk::Label,
    prompt: RefCell<Prompt>,
    load_failed: Cell<bool>,
    target: String,
}
impl Tab {
    fn title(&self) -> String {
        self.web
            .title()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                url::Url::parse(self.web.uri().as_deref().unwrap_or(&self.target))
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_else(|| "Loading…".into())
            })
    }
    fn render_prompt(&self, focus: bool, select: bool) {
        let p = self.prompt.borrow();
        if p.open {
            if self.entry.text().as_str() != p.draft {
                self.entry.set_text(&p.draft);
            }
            self.error.set_text(&p.error);
            self.error.set_visible(!p.error.is_empty());
            self.strip.show();
            if focus {
                self.entry.grab_focus();
                if select {
                    self.entry.select_region(0, -1);
                }
            }
        } else {
            self.strip.hide();
            if focus {
                self.web.grab_focus();
            }
        }
    }
}
struct Ui {
    window: tauri::Window,
    native: gtk::ApplicationWindow,
    notebook: gtk::Notebook,
    tabs: RefCell<Tabs<Rc<Tab>>>,
    css: gtk::CssProvider,
    fullscreen: Cell<bool>,
    updating: Cell<bool>,
    destroyed: Cell<bool>,
    #[cfg(feature = "gui-smoke")]
    fixture_home: String,
}
impl Ui {
    fn home_url(&self) -> &str {
        #[cfg(feature = "gui-smoke")]
        if std::env::args().any(|a| a == "--gui-smoke") {
            return &self.fixture_home;
        }
        HOME_URL
    }
    fn active(&self) -> Rc<Tab> {
        self.tabs
            .borrow()
            .active()
            .expect("live window has a tab")
            .clone()
    }
    fn is_active(&self, tab: &Rc<Tab>) -> bool {
        !self.destroyed.get()
            && self
                .tabs
                .borrow()
                .active()
                .is_some_and(|t| Rc::ptr_eq(t, tab))
    }
    fn refresh_title(&self) {
        if let Some(tab) = self.tabs.borrow().active() {
            let _ = self.window.set_title(&format!("{} — Omawser", tab.title()));
        }
    }
    fn sync_selection(&self) {
        self.notebook.set_show_tabs(self.tabs.borrow().len() >= 2);
        let index = self.tabs.borrow().active_index();
        if let Some(index) = index {
            self.updating.set(true);
            self.notebook.set_current_page(Some(index as u32));
            self.updating.set(false);
            self.refresh_title();
            self.active().render_prompt(true, false);
        }
    }
    fn select(&self, index: usize) {
        self.tabs.borrow_mut().select(index);
        self.sync_selection();
    }
    fn close_tab(&self, index: usize) {
        let tab = self.tabs.borrow().items().get(index).cloned();
        let Some(tab) = tab else {
            return;
        };
        if self.tabs.borrow().len() == 1 {
            let _ = self.window.close();
            return;
        }
        let url = tab
            .prompt
            .borrow()
            .failed_target
            .clone()
            .or_else(|| tab.web.uri().map(|u| u.to_string()))
            .unwrap_or_else(|| tab.target.clone());
        self.tabs.borrow_mut().close(index, &url);
        self.updating.set(true);
        self.notebook.remove_page(Some(index as u32));
        self.updating.set(false);
        tab.web.stop_loading();
        let _ = tab.handle.close();
        self.sync_selection();
    }
    fn show(&self) {
        let tab = self.active();
        tab.prompt
            .borrow_mut()
            .show(tab.web.uri().as_deref().unwrap_or(&tab.target));
        tab.render_prompt(true, true);
    }
    fn hide(&self) {
        let tab = self.active();
        tab.prompt.borrow_mut().hide();
        tab.render_prompt(true, false);
    }
    fn submit(&self) {
        let tab = self.active();
        tab.prompt.borrow_mut().draft = tab.entry.text().into();
        let target = tab.prompt.borrow_mut().submit();
        tab.render_prompt(true, false);
        if let Some(target) = target {
            tab.web.load_uri(&target);
        }
    }
    fn reload(&self) {
        let tab = self.active();
        let retry = tab.prompt.borrow().retry_target();
        if let Some(target) = retry {
            tab.web.load_uri(&target);
        } else {
            tab.web.reload();
        }
    }
    fn apply_theme(&self, p: &Palette) {
        let css = format!("window#oma-window, #oma-frame, #oma-tabs, #oma-tabs stack {{ background-color: {bg}; color: {fg}; }} #oma-frame, #oma-tabs {{ border: none; border-radius: 0; padding: 0; }} #oma-tabs > header {{ background: {bg}; border: none; border-bottom: 1px solid alpha({fg}, 0.12); padding: 0; margin: 0; }} #oma-tabs > header tabs {{ padding: 0; margin: 0; }} #oma-tabs > header tab {{ background: transparent; color: mix({bg}, {fg}, 0.75); border: none; border-bottom: 1px solid transparent; border-radius: 0; box-shadow: none; padding: 4px 8px; margin: 0; min-height: 24px; font-family: monospace; }} #oma-tabs > header tab:hover {{ background: alpha({fg}, 0.08); }} #oma-tabs > header tab:checked {{ background: alpha({fg}, 0.08); color: {fg}; border-bottom-color: {fg}; }} #oma-tabs > header tab:focus {{ outline: 1px solid alpha({fg}, 0.4); outline-offset: -2px; }} #oma-tab-close {{ background: transparent; color: inherit; border: none; border-radius: 0; box-shadow: none; padding: 0 4px; min-height: 20px; min-width: 16px; }} #oma-tab-close:hover, #oma-tab-close:focus {{ background: alpha({fg}, 0.18); }} #oma-strip {{ padding: 6px; }} #oma-address {{ background: {surface}; color: {fg}; border: 1px solid {accent}; border-radius: 0; box-shadow: none; min-height: 24px; padding: 2px 6px; }} #oma-error {{ color: {fg}; }}", bg=p.background, fg=p.foreground, surface=p.lighter_background, accent=p.accent);
        if let Err(e) = self.css.load_from_data(css.as_bytes()) {
            eprintln!("theme CSS: {e}");
        }
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(p.mode == "dark");
        }
        if let Ok(color) = gdk::RGBA::parse(&p.background) {
            for tab in self.tabs.borrow().items() {
                tab.web.set_background_color(&color);
            }
        }
        let _ = self.window.set_theme(Some(if p.mode == "light" {
            tauri::Theme::Light
        } else {
            tauri::Theme::Dark
        }));
    }
    fn key(self: &Rc<Self>, e: &gdk::EventKey) -> glib::Propagation {
        use gdk::{keys::constants as k, ModifierType as M};
        let mods = e.state()
            & (M::CONTROL_MASK | M::MOD1_MASK | M::SHIFT_MASK | M::SUPER_MASK | M::META_MASK);
        let key = e.keyval();
        let lower = key.to_lower();
        let plain = mods.is_empty();
        let ctrl = mods == M::CONTROL_MASK;
        let shift_ctrl = mods == (M::CONTROL_MASK | M::SHIFT_MASK);
        let alt = mods == M::MOD1_MASK;
        if ctrl && lower == k::l {
            self.show();
        } else if ctrl && lower == k::t {
            self.add_tab(self.home_url(), true, true)
                .unwrap_or_else(|e| eprintln!("new tab: {e}"));
        } else if shift_ctrl && lower == k::t {
            let url = self.tabs.borrow_mut().reopen();
            if let Some(url) = url {
                self.add_tab(&url, true, false)
                    .unwrap_or_else(|e| eprintln!("reopen tab: {e}"));
            }
        } else if shift_ctrl && lower == k::w {
            let _ = self.window.close();
        } else if ctrl && lower == k::w {
            let index = self.tabs.borrow().active_index();
            if let Some(i) = index {
                self.close_tab(i);
            }
        } else if (ctrl && [k::Tab, k::Page_Down].contains(&key))
            || (shift_ctrl && [k::Tab, k::ISO_Left_Tab].contains(&key))
            || (ctrl && key == k::Page_Up)
        {
            let previous = shift_ctrl || key == k::Page_Up;
            self.tabs.borrow_mut().cycle(if previous { -1 } else { 1 });
            self.sync_selection();
        } else if ctrl && key >= k::_1 && key <= k::_9 {
            let index = if key == k::_9 {
                self.tabs.borrow().len() - 1
            } else {
                (*key - *k::_1) as usize
            };
            self.select(index);
        } else if ctrl && lower == k::n {
            if let Err(e) = create_window(self.window.app_handle(), self.home_url().into()) {
                eprintln!("new window: {e}");
            }
        } else if (ctrl && lower == k::r) || (plain && key == k::F5) {
            self.reload();
        } else if ctrl && key == k::_0 {
            self.active().web.set_zoom_level(1.0);
        } else if (ctrl || shift_ctrl) && [k::plus, k::equal, k::minus].contains(&key) {
            let web = &self.active().web;
            web.set_zoom_level(
                (web.zoom_level() * if key == k::minus { 1.0 / 1.2 } else { 1.2 }).clamp(0.25, 5.0),
            );
        } else if alt && key == k::Left {
            self.active().web.go_back();
        } else if alt && key == k::Right {
            self.active().web.go_forward();
        } else if alt && key == k::Home {
            self.hide();
            self.active().web.load_uri(self.home_url());
        } else if plain && key == k::F11 {
            let full = !self.fullscreen.get();
            self.fullscreen.set(full);
            let _ = self.window.set_fullscreen(full);
        } else if plain && key == k::Escape {
            if self.active().prompt.borrow().open {
                self.hide();
            } else if self.active().web.is_loading() {
                self.active().web.stop_loading();
            } else {
                return glib::Propagation::Proceed;
            }
        } else {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    }
    fn add_tab(self: &Rc<Self>, target: &str, foreground: bool, prompt: bool) -> tauri::Result<()> {
        if self.destroyed.get() || !navigation_allowed(target) {
            return Ok(());
        }
        static NEXT_TAB: AtomicUsize = AtomicUsize::new(1);
        let label = format!("tab-{}", NEXT_TAB.fetch_add(1, Ordering::Relaxed));
        let window_label = self.window.label().to_string();
        let popup_label = window_label.clone();
        let source_label = label.clone();
        let builder =
            WebviewBuilder::new(label, WebviewUrl::External("about:blank".parse().unwrap()))
                .devtools(false)
                .focused(false)
                .disable_drag_drop_handler()
                .data_directory(xdg("XDG_DATA_HOME", ".local/share").join("omawser/webkit"))
                .on_navigation(|url| navigation_allowed(url.as_str()))
                .on_new_window(move |url, _| {
                    if navigation_allowed(url.as_str()) {
                        let label = popup_label.clone();
                        let source = source_label.clone();
                        glib::idle_add_once(move || {
                            with_window(&label, |ui| {
                                if !ui
                                    .tabs
                                    .borrow()
                                    .items()
                                    .iter()
                                    .any(|t| t.handle.label() == source)
                                {
                                    return;
                                }
                                if let Err(e) = ui.add_tab(url.as_str(), true, false) {
                                    eprintln!("popup tab: {e}");
                                }
                            })
                        });
                    }
                    tauri::webview::NewWindowResponse::Deny
                })
                .on_permission_request(|_, _| tauri::webview::PermissionResponse::Deny);
        let handle = self.window.add_child(
            builder,
            tauri::LogicalPosition::new(0., 0.),
            tauri::LogicalSize::new(800., 600.),
        )?;
        let owned = handle.clone();
        let target = target.to_owned();
        handle.with_webview(move |platform| {
            with_window(&window_label, |ui| {
                ui.install_tab(owned, platform.inner(), target, foreground, prompt);
            })
        })?;
        Ok(())
    }
    fn install_tab(
        self: &Rc<Self>,
        handle: tauri::Webview,
        web: webkit2gtk::WebView,
        target: String,
        foreground: bool,
        prompt: bool,
    ) {
        if self.destroyed.get() {
            let _ = handle.close();
            return;
        }
        if !own_navigation_policy(&web) {
            // Unknown upstream policy wiring must never create an unguarded remote tab.
            web.connect_decide_policy(|_, decision, _| {
                decision.ignore();
                true
            });
            let _ = handle.close();
            if self.tabs.borrow().is_empty() {
                let _ = self.window.close();
            }
            return;
        }
        if let Some(manager) = web.user_content_manager() {
            manager.remove_all_scripts();
            manager.unregister_script_message_handler("ipc");
        }
        if let Some(settings) = WebViewExt::settings(&web) {
            settings.set_enable_developer_extras(false);
            settings.set_javascript_can_access_clipboard(false);
            settings.set_allow_file_access_from_file_urls(false);
            settings.set_allow_universal_access_from_file_urls(false);
        }
        if !install_downloads(&web) {
            web.connect_decide_policy(|_, decision, _| {
                decision.ignore();
                true
            });
            let _ = handle.close();
            if self.tabs.borrow().is_empty() {
                let _ = self.window.close();
            }
            return;
        }
        web.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        // Tauri initially packs child views into its vbox. GTK owns the layout after this move.
        if let Some(parent) = web
            .parent()
            .and_then(|p| p.downcast::<gtk::Container>().ok())
        {
            parent.remove(&web);
        }
        web.set_size_request(-1, -1);
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let strip = gtk::Box::new(gtk::Orientation::Vertical, 3);
        strip.set_widget_name("oma-strip");
        strip.set_no_show_all(true);
        let entry = gtk::Entry::new();
        entry.set_widget_name("oma-address");
        entry.set_hexpand(true);
        if let Some(accessible) = entry.accessible() {
            accessible.set_name("Address");
        }
        let error = gtk::Label::new(None);
        error.set_widget_name("oma-error");
        error.set_xalign(0.0);
        error.set_selectable(true);
        error.set_ellipsize(gtk::pango::EllipsizeMode::End);
        strip.pack_start(&entry, false, false, 0);
        strip.pack_start(&error, false, false, 0);
        page.pack_start(&strip, false, false, 0);
        page.pack_start(&web, true, true, 0);
        let label = gtk::Label::new(None);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_width_chars(16);
        label.set_max_width_chars(24);
        let close = gtk::Button::with_label("×");
        close.set_widget_name("oma-tab-close");
        close.set_tooltip_text(Some("Close tab (Ctrl+W)"));
        close.set_focus_on_click(false);
        close.set_can_focus(false);
        if let Some(accessible) = close.accessible() {
            accessible.set_name("Close tab");
        }
        let header_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header_content.pack_start(&label, true, true, 0);
        header_content.pack_start(&close, false, false, 0);
        let header = gtk::EventBox::new();
        header.set_visible_window(false);
        header.add(&header_content);
        header.show_all();
        for widget in [
            page.upcast_ref::<gtk::Widget>(),
            strip.upcast_ref(),
            entry.upcast_ref(),
            error.upcast_ref(),
            header.upcast_ref(),
            label.upcast_ref(),
            close.upcast_ref(),
        ] {
            widget
                .style_context()
                .add_provider(&self.css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
        let tab = Rc::new(Tab {
            handle,
            web,
            page,
            label,
            close,
            strip,
            entry,
            error,
            prompt: RefCell::new(Prompt::default()),
            load_failed: Cell::new(false),
            target,
        });
        self.tabs.borrow_mut().push(tab.clone(), foreground);
        self.updating.set(true);
        self.notebook.append_page(&tab.page, Some(&header));
        self.updating.set(false);
        tab.page.show_all();
        tab.entry.show();
        tab.strip.hide();
        wire_tab(self, &tab, &header);
        self.apply_theme(&palette());
        update_tab_title(self, &tab);
        if prompt {
            tab.prompt.borrow_mut().show(&tab.target);
        }
        if foreground || self.tabs.borrow().len() == 1 {
            self.sync_selection();
            if prompt {
                tab.render_prompt(true, true);
            }
        } else {
            self.notebook.set_show_tabs(true);
        }
        tab.web.load_uri(&tab.target);
    }
}
fn with_window(label: &str, action: impl FnOnce(Rc<Ui>)) {
    let ui = WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .find(|ui| ui.window.label() == label && !ui.destroyed.get())
    });
    if let Some(ui) = ui {
        action(ui);
    }
}
fn update_tab_title(ui: &Ui, tab: &Rc<Tab>) {
    let title = tab.title();
    tab.label.set_text(&title);
    tab.label.set_tooltip_text(Some(&title));
    if let Some(accessible) = tab.label.accessible() {
        accessible.set_name(&title);
    }
    if ui.is_active(tab) {
        ui.refresh_title();
    }
}
fn wire_tab(ui: &Rc<Ui>, tab: &Rc<Tab>, header: &gtk::EventBox) {
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    tab.close.connect_clicked(move |_| {
        if let (Some(ui), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
            let index = ui
                .tabs
                .borrow()
                .items()
                .iter()
                .position(|t| Rc::ptr_eq(t, &tab));
            if let Some(index) = index {
                ui.close_tab(index);
            }
        }
    });
    header.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    header.connect_button_press_event(move |_, event| {
        if event.button() == 2 {
            if let (Some(ui), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
                let index = ui
                    .tabs
                    .borrow()
                    .items()
                    .iter()
                    .position(|t| Rc::ptr_eq(t, &tab));
                if let Some(index) = index {
                    ui.close_tab(index);
                }
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    tab.entry.connect_activate(move |_| {
        if let (Some(ui), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
            if ui.is_active(&tab) {
                ui.submit();
            }
        }
    });
    let weak_tab = Rc::downgrade(tab);
    tab.entry.connect_changed(move |entry| {
        if let Some(tab) = weak_tab.upgrade() {
            if let Ok(mut p) = tab.prompt.try_borrow_mut() {
                p.draft = entry.text().into();
            }
        }
    });
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    tab.web.connect_load_failed(move |_, _, uri, error| {
        if error.matches(webkit2gtk::NetworkError::Cancelled) {
            return true;
        }
        if let (Some(ui), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
            tab.load_failed.set(true);
            if navigation_allowed(uri) {
                tab.prompt.borrow_mut().failed(uri, error.message());
                tab.render_prompt(ui.is_active(&tab), false);
            }
        }
        true
    });
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    tab.web.connect_load_changed(move |_, event| {
        if let (Some(ui), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
            match event {
                webkit2gtk::LoadEvent::Started => tab.load_failed.set(false),
                webkit2gtk::LoadEvent::Finished if !tab.load_failed.get() => {
                    tab.prompt.borrow_mut().succeeded();
                    if !tab.prompt.borrow().open {
                        tab.render_prompt(ui.is_active(&tab) && !ui.notebook.has_focus(), false);
                    }
                }
                _ => {}
            }
            update_tab_title(&ui, &tab);
        }
    });
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    tab.web.connect_title_notify(move |_| {
        if let (Some(ui), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
            update_tab_title(&ui, &tab);
        }
    });
    let weak = Rc::downgrade(ui);
    let weak_tab = Rc::downgrade(tab);
    tab.web.connect_decide_policy(move |_, decision, kind| {
        let navigation = decision
            .downcast_ref::<webkit2gtk::NavigationPolicyDecision>()
            .and_then(|d| d.navigation_action());
        let uri = if let Some(d) = decision.downcast_ref::<webkit2gtk::ResponsePolicyDecision>() {
            d.request().and_then(|r| r.uri())
        } else {
            navigation
                .as_ref()
                .and_then(|a| a.request())
                .and_then(|r| r.uri())
        };
        if uri.as_deref().is_none_or(|u| !navigation_allowed(u)) {
            decision.ignore();
            return true;
        }
        if matches!(
            kind,
            webkit2gtk::PolicyDecisionType::NavigationAction
                | webkit2gtk::PolicyDecisionType::NewWindowAction
        ) {
            if let Some(action) = navigation {
                if action.navigation_type() == webkit2gtk::NavigationType::LinkClicked
                    && (action.mouse_button() == 2
                        || action.modifiers() & gdk::ModifierType::CONTROL_MASK.bits() != 0)
                {
                    decision.ignore();
                    if let (Some(ui), Some(uri)) = (weak.upgrade(), uri) {
                        let weak = Rc::downgrade(&ui);
                        let weak_tab = weak_tab.clone();
                        glib::idle_add_local_once(move || {
                            if let (Some(ui), Some(source)) = (weak.upgrade(), weak_tab.upgrade()) {
                                if !ui
                                    .tabs
                                    .borrow()
                                    .items()
                                    .iter()
                                    .any(|t| Rc::ptr_eq(t, &source))
                                {
                                    return;
                                }
                                if let Err(e) = ui.add_tab(&uri, false, false) {
                                    eprintln!("background tab: {e}");
                                }
                            }
                        });
                    }
                    return true;
                }
            }
        }
        false
    });
}
fn create_window(app: &tauri::AppHandle, target: String) -> tauri::Result<()> {
    let label = format!("browser-{}", NEXT_WINDOW.fetch_add(1, Ordering::Relaxed));
    let window = WindowBuilder::new(app, label)
        .title("Omawser")
        .inner_size(1200.0, 850.0)
        .min_inner_size(480.0, 320.0)
        .decorations(false)
        .visible(false)
        .build()?;
    let native = window.gtk_window()?;
    native.set_widget_name("oma-window");
    let frame = window.default_vbox()?;
    frame.set_widget_name("oma-frame");
    let notebook = gtk::Notebook::new();
    notebook.set_widget_name("oma-tabs");
    notebook.set_scrollable(true);
    notebook.set_show_border(false);
    notebook.set_show_tabs(false);
    notebook.set_hexpand(true);
    notebook.set_vexpand(true);
    frame.pack_start(&notebook, true, true, 0);
    notebook.show();
    let css = gtk::CssProvider::new();
    for widget in [
        native.upcast_ref::<gtk::Widget>(),
        frame.upcast_ref(),
        notebook.upcast_ref(),
    ] {
        widget
            .style_context()
            .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let ui = Rc::new(Ui {
        window,
        native: native.clone(),
        notebook,
        tabs: RefCell::new(Tabs::default()),
        css,
        fullscreen: Cell::new(false),
        updating: Cell::new(false),
        destroyed: Cell::new(false),
        #[cfg(feature = "gui-smoke")]
        fixture_home: target.clone(),
    });
    WINDOWS.with(|windows| windows.borrow_mut().push(Rc::downgrade(&ui)));
    let weak = Rc::downgrade(&ui);
    native.connect_key_press_event(move |_, event| {
        weak.upgrade()
            .map_or(glib::Propagation::Proceed, |ui| ui.key(event))
    });
    let weak = Rc::downgrade(&ui);
    ui.notebook.connect_switch_page(move |_, _, index| {
        if let Some(ui) = weak.upgrade() {
            if !ui.updating.get() {
                ui.tabs.borrow_mut().select(index as usize);
                ui.refresh_title();
                // Native notebook arrows keep the keyboard within its peer group.
                // Pointer selection and explicit browser shortcuts return to the page.
                let keyboard_header = ui.notebook.has_focus()
                    && gtk::current_event()
                        .is_some_and(|e| e.event_type() == gdk::EventType::KeyPress);
                let weak = Rc::downgrade(&ui);
                glib::idle_add_local_once(move || {
                    if let Some(ui) = weak.upgrade() {
                        if !ui.destroyed.get() {
                            ui.active().render_prompt(!keyboard_header, false);
                        }
                    }
                });
            }
        }
    });
    let weak = Rc::downgrade(&ui);
    native.connect_focus_in_event(move |_, _| {
        let weak = weak.clone();
        glib::idle_add_local_once(move || {
            if let Some(ui) = weak.upgrade() {
                if ui.native.is_active() && !ui.destroyed.get() && !ui.notebook.has_focus() {
                    ui.active().render_prompt(true, false);
                }
            }
        });
        glib::Propagation::Proceed
    });
    ui.add_tab(&target, true, false)?;
    ui.apply_theme(&palette());
    let _ = ui.window.show();
    #[cfg(feature = "gui-smoke")]
    if std::env::args().any(|a| a == "--gui-smoke") && ui.window.label() == "browser-1" {
        smoke::start(ui.clone(), target);
    }
    // Window destruction releases every native tab, including callbacks whose weak refs expire.
    let holder = Rc::new(RefCell::new(Some(ui)));
    native.connect_destroy(move |_| {
        if let Some(ui) = holder.borrow_mut().take() {
            ui.destroyed.set(true);
            for tab in ui.tabs.borrow().items() {
                tab.web.stop_loading();
            }
        }
    });
    Ok(())
}
fn main() {
    let target = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| HOME_URL.into());
    let Some(target) = address(&target) else {
        eprintln!("Expected an HTTP(S) address");
        std::process::exit(2);
    };
    #[cfg(feature = "gui-smoke")]
    if std::env::args().any(|a| a == "--gui-smoke") {
        smoke::validate_environment(&target);
    }
    let app = tauri::Builder::default()
        // Replace Tauri's local asset and invoke protocols with an unconditional rejection.
        .register_uri_scheme_protocol("tauri", |_, _| {
            tauri::http::Response::builder()
                .status(403)
                .body(Vec::<u8>::new())
                .unwrap()
        })
        .register_uri_scheme_protocol("ipc", |_, _| {
            tauri::http::Response::builder()
                .status(403)
                .body(Vec::<u8>::new())
                .unwrap()
        })
        .setup(move |app| {
            create_window(app.handle(), target.clone())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("build Tauri application");
    let handle = app.handle().clone();
    let mut watcher = ThemeWatch::new(theme_paths(), move || {
        let _ = handle.run_on_main_thread(|| {
            let p = palette();
            WINDOWS.with(|windows| {
                windows.borrow_mut().retain(|weak| {
                    if let Some(ui) = weak.upgrade() {
                        ui.apply_theme(&p);
                        true
                    } else {
                        false
                    }
                });
            });
        });
    })
    .map_err(|e| eprintln!("theme watch unavailable: {e}"))
    .ok();
    app.run(move |_, event| {
        if let tauri::RunEvent::Exit = event {
            drop(watcher.take());
        }
    });
}
