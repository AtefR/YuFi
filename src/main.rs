mod backend;
mod models;

use backend::{Backend, BackendError};
use backend::nm::NetworkManagerBackend;
use gtk4::gdk::Display;
use gtk4::glib::ControlFlow;
use gtk4::glib::Propagation;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Dialog, Entry, Image,
    Label, ListBox, ListBoxRow, MessageDialog, MessageType, Orientation, Overlay, ResponseType,
    SearchEntry, Spinner, Switch,
};
use models::{AppState, Network, NetworkAction, NetworkDetails};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;
use std::thread;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

fn main() {
    let app = Application::builder()
        .application_id("com.yufi.app")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    load_css();

    let (ui_tx, ui_rx) = mpsc::channel::<UiEvent>();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("YuFi Network Manager Dashboard")
        .default_width(360)
        .default_height(720)
        .build();

    window.add_css_class("yufi-window");

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let panel = GtkBox::new(Orientation::Vertical, 12);
    panel.add_css_class("yufi-panel");

    let nm_backend = Rc::new(NetworkManagerBackend::new());
    let toggle_guard = Rc::new(Cell::new(false));
    let loading = LoadingTracker::new();

    let (status_bar, status_label) = build_status();
    let status_handler = build_status_handler(&status_label);
    let state = load_state_with_backend(&nm_backend, &status_handler);
    let state_cache = Rc::new(RefCell::new(state.clone()));

    let header = build_header(&state);
    let header_ref = Rc::new(header.clone());
    let search = build_search();
    let list = build_network_list();
    let legend = build_lock_legend();
    let action_handler: Rc<RefCell<Option<ActionHandler>>> = Rc::new(RefCell::new(None));
    let optimistic_active = Rc::new(RefCell::new(None::<String>));
    let pending_connect = Rc::new(RefCell::new(None::<PendingConnect>));
    let failed_connects = Rc::new(RefCell::new(HashSet::<String>::new()));
    let filtered_state = filter_state(&state, &search.text().to_string());
    let empty_label = empty_label_for(
        &state,
        &search.text().to_string(),
        filtered_state.networks.len(),
    );
    populate_network_list(
        &list,
        &filtered_state,
        &action_handler,
        optimistic_active.borrow().as_deref(),
        empty_label,
        pending_connect
            .borrow()
            .as_ref()
            .map(|pending| pending.ssid.as_str()),
        &failed_connects.borrow(),
    );
    let status_container = Rc::new(StatusContainer {
        dialog_label: Rc::new(RefCell::new(None)),
    });
    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    let hidden = build_hidden_button();

    panel.append(&header.container);
    panel.append(&search);
    panel.append(&status_bar);
    panel.append(&list);
    panel.append(&legend);
    panel.append(&spacer);
    panel.append(&hidden);

    root.append(&panel);

    wire_actions(
        &header,
        &list,
        &nm_backend,
        &state_cache,
        &failed_connects,
        &toggle_guard,
        &window,
        &status_handler,
        &status_container,
        &loading,
        &header_ref,
        &ui_tx,
    );

    let list_search = list.clone();
    let handler_search = action_handler.clone();
    let state_search = state_cache.clone();
    let optimistic_search = optimistic_active.clone();
    let pending_search = pending_connect.clone();
    let failed_search = failed_connects.clone();
    search.connect_changed(move |entry| {
        let query = entry.text().to_string();
        let state = state_search.borrow().clone();
        let filtered = filter_state(&state, &query);
        let empty_label = empty_label_for(&state, &query, filtered.networks.len());
        populate_network_list(
            &list_search,
            &filtered,
            &handler_search,
            optimistic_search.borrow().as_deref(),
            empty_label,
            pending_search
                .borrow()
                .as_ref()
                .map(|pending| pending.ssid.as_str()),
            &failed_search.borrow(),
        );
    });

    let loading_action = loading.clone();
    let header_action = header_ref.clone();
    let ui_tx_action = ui_tx.clone();
    let window_action = window.clone();
    let status_container_connect = status_container.clone();

    *action_handler.borrow_mut() = Some(Rc::new(move |action| {
        match action {
            RowAction::Connect { ssid, is_saved } => {
                if is_saved {
                    let ssid_clone = ssid.clone();
                    loading_action.start();
                    update_loading_ui(header_action.as_ref(), &loading_action);
                    spawn_connect_task(&ui_tx_action, ssid_clone, None, false, true);
                } else {
                    prompt_connect_dialog(
                        &window_action,
                        &ssid,
                        &loading_action,
                        &header_action,
                        &ui_tx_action,
                        &status_container_connect,
                        false,
                        None,
                    );
                }
            }
            RowAction::Disconnect(ssid) => {
                let ssid_clone = ssid.clone();
                loading_action.start();
                update_loading_ui(header_action.as_ref(), &loading_action);
                spawn_disconnect_task(&ui_tx_action, ssid_clone);
            }
        }
    }));

    let hidden_window = window.clone();
    let loading_hidden = loading.clone();
    let header_hidden = header_ref.clone();
    let ui_tx_hidden = ui_tx.clone();
    let status_container_action = status_container.clone();
    hidden.connect_clicked(move |_| {
        let loading_hidden = loading_hidden.clone();
        let header_hidden = header_hidden.clone();
        let status_container_dialog = status_container_action.clone();
        let ui_tx_hidden = ui_tx_hidden.clone();
        show_hidden_network_dialog(
            &hidden_window,
            move |ssid, password| {
                loading_hidden.start();
                update_loading_ui(header_hidden.as_ref(), &loading_hidden);
                spawn_hidden_task(&ui_tx_hidden, ssid, password);
            },
            (*status_container_dialog).clone(),
        );
    });

    let list_rx = list.clone();
    let toggle_rx = header.toggle.clone();
    let guard_rx = toggle_guard.clone();
    let handler_rx = action_handler.clone();
    let status_rx = status_handler.clone();
    let status_container_rx = status_container.clone();
    let loading_rx = loading.clone();
    let header_rx = header_ref.clone();
    let refresh_button_rx = header.refresh.clone();
    let spinner_rx = header.spinner.clone();
    let refresh_overlay_rx = header.refresh_overlay.clone();
    let window_rx = window.clone();
    let ui_tx_rx = ui_tx.clone();
    let ui_rx = Rc::new(RefCell::new(ui_rx));
    let optimistic_active_rx = optimistic_active.clone();
    let pending_connect_rx = pending_connect.clone();
    let failed_connects_rx = failed_connects.clone();
    let refresh_guard = Rc::new(Cell::new(false));
    let refresh_guard_rx = refresh_guard.clone();
    let refresh_guard_signal = refresh_guard.clone();
    let ui_tx_signal = ui_tx.clone();
    spawn_nm_signal_listeners(&ui_tx_signal);
    let state_cache_rx = state_cache.clone();
    let search_rx = search.clone();

    gtk4::glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(event) = ui_rx.borrow().try_recv() {
            match event {
                UiEvent::StateLoaded(result) => {
                    let state = match result {
                        Ok(state) => state,
                        Err(err) => {
                            status_rx(StatusKind::Error, format!("NetworkManager error: {err:?}"));
                            fallback_state(err)
                        }
                    };
                    guard_rx.set(true);
                    toggle_rx.set_active(state.wifi_enabled);
                    guard_rx.set(false);
                    if state.networks.iter().any(|n| matches!(n.action, NetworkAction::Disconnect)) {
                        *optimistic_active_rx.borrow_mut() = None;
                    }
                    let pending = pending_connect_rx.borrow().clone();
                    if let Some(pending) = pending {
                        let is_active = state.networks.iter().any(|network| {
                            network.ssid == pending.ssid
                                && matches!(network.action, NetworkAction::Disconnect)
                        });
                        if is_active {
                            status_rx(StatusKind::Info, String::new());
                            *pending_connect_rx.borrow_mut() = None;
                            *optimistic_active_rx.borrow_mut() = None;
                            failed_connects_rx.borrow_mut().remove(&pending.ssid);
                        }
                    }
                    *state_cache_rx.borrow_mut() = state.clone();
                    let query = search_rx.text().to_string();
                    let filtered = filter_state(&state, &query);
                    let empty_label = empty_label_for(&state, &query, filtered.networks.len());
                    let pending_ssid_owned = pending_connect_rx
                        .borrow()
                        .as_ref()
                        .map(|pending| pending.ssid.clone());
                    let pending_ssid = pending_ssid_owned.as_deref();
                    populate_network_list(
                        &list_rx,
                        &filtered,
                        &handler_rx,
                        optimistic_active_rx.borrow().as_deref(),
                        empty_label,
                        pending_ssid,
                        &failed_connects_rx.borrow(),
                    );
                }
                UiEvent::ScanDone(result) => {
                    loading_rx.stop();
                    update_loading_ui(header_rx.as_ref(), &loading_rx);
                    spinner_rx.stop();
                    spinner_rx.set_visible(false);
                    refresh_overlay_rx.set_visible(true);
                    refresh_button_rx.set_sensitive(true);
                    refresh_button_rx.set_visible(true);
                    refresh_button_rx.set_opacity(1.0);
                    match result {
        Ok(_) => status_rx(StatusKind::Info, "Scan complete".to_string()),
        Err(err) => {
            status_rx(StatusKind::Error, format!("Scan failed: {}", friendly_error(&err)))
        }
    }
                    // Updates should arrive via D-Bus signals.
                }
                UiEvent::WifiSet { enabled, result } => {
                    loading_rx.stop();
                    update_loading_ui(header_rx.as_ref(), &loading_rx);
                    let is_err = result.is_err();
                    match result {
                        Ok(_) => {
                            let label = if enabled { "Wi‑Fi enabled" } else { "Wi‑Fi disabled" };
                            status_rx(StatusKind::Success, label.to_string());
                        }
                        Err(err) => {
                            status_rx(
                                StatusKind::Error,
                                format!("Failed to set Wi‑Fi: {}", friendly_error(&err)),
                            );
                        }
                    }
                    if is_err {
                        request_state_refresh(&ui_tx_rx);
                    }
                }
                UiEvent::ConnectDone { ssid, result, from_password, was_saved } => {
                    loading_rx.stop();
                    update_loading_ui(header_rx.as_ref(), &loading_rx);
                    match result {
                        Ok(active_path) => {
                            *pending_connect_rx.borrow_mut() = Some(PendingConnect {
                                ssid: ssid.clone(),
                                was_saved,
                                from_password,
                            });
                            status_rx(StatusKind::Info, String::new());
                            if let Some(path) = active_path {
                                spawn_active_connection_listener(&ui_tx_rx, ssid.clone(), path);
                            } else {
                                request_state_refresh(&ui_tx_rx);
                            }
                        }
                        Err(err) => {
                            *optimistic_active_rx.borrow_mut() = None;
                            *pending_connect_rx.borrow_mut() = None;
                            if !from_password && needs_password(&err) {
                                let loading_retry = loading_rx.clone();
                                let header_retry = header_rx.clone();
                                let ui_tx_retry = ui_tx_rx.clone();
                                let ssid_retry = ssid.clone();
                                let status_container_retry = status_container_rx.clone();
                                show_password_dialog(
                                    &window_rx,
                                    &ssid,
                                    None,
                                    move |password| {
                                        loading_retry.start();
                                        update_loading_ui(header_retry.as_ref(), &loading_retry);
                                        spawn_connect_task(
                                            &ui_tx_retry,
                                            ssid_retry.clone(),
                                            password.clone(),
                                            password.is_some(),
                                            true,
                                        );
                                    },
                                    (*status_container_retry).clone(),
                                );
                            } else {
                                let message = connect_error_message(&err, from_password);
                                status_rx(
                                    StatusKind::Error,
                                    format!("Connect failed: {message}"),
                                );
                                if from_password {
                                    let loading_retry = loading_rx.clone();
                                    let header_retry = header_rx.clone();
                                    let ui_tx_retry = ui_tx_rx.clone();
                                    let ssid_retry = ssid.clone();
                                    let ssid_label = ssid.clone();
                                    let status_container_retry = status_container_rx.clone();
                                    show_password_dialog(
                                        &window_rx,
                                        &ssid_label,
                                        Some(message),
                                        move |password| {
                                            loading_retry.start();
                                            update_loading_ui(header_retry.as_ref(), &loading_retry);
                                            spawn_connect_task(
                                                &ui_tx_retry,
                                                ssid_retry.clone(),
                                                password.clone(),
                                                password.is_some(),
                                                true,
                                            );
                                        },
                                        (*status_container_retry).clone(),
                                    );
                                }
                            }
                        }
                    }
                }
                UiEvent::DisconnectDone { ssid, result } => {
                    loading_rx.stop();
                    update_loading_ui(header_rx.as_ref(), &loading_rx);
                    match result {
                        Ok(_) => status_rx(StatusKind::Success, format!("Disconnected from {ssid}")),
                        Err(err) => status_rx(
                            StatusKind::Error,
                            format!("Disconnect failed: {}", friendly_error(&err)),
                        ),
                    }
                    *optimistic_active_rx.borrow_mut() = None;
                    *pending_connect_rx.borrow_mut() = None;
                    failed_connects_rx.borrow_mut().remove(&ssid);
                    // Updates should arrive via D-Bus signals.
                }
                UiEvent::HiddenDone { ssid, result } => {
                    loading_rx.stop();
                    update_loading_ui(header_rx.as_ref(), &loading_rx);
                    match result {
                        Ok(active_path) => {
                            *pending_connect_rx.borrow_mut() = Some(PendingConnect {
                                ssid: ssid.clone(),
                                was_saved: false,
                                from_password: true,
                            });
                            status_rx(StatusKind::Info, String::new());
                            if let Some(path) = active_path {
                                spawn_active_connection_listener(&ui_tx_rx, ssid.clone(), path);
                            } else {
                                request_state_refresh(&ui_tx_rx);
                            }
                        }
                        Err(err) => {
                            status_rx(
                                StatusKind::Error,
                                format!("Hidden connect failed: {}", friendly_error(&err)),
                            );
                        }
                    }
                }
                UiEvent::ActiveState { ssid, state } => {
                    let pending = pending_connect_rx.borrow().clone();
                    if let Some(pending) = pending {
                        if pending.ssid != ssid {
                            continue;
                        }
                        let is_secure = state_cache_rx
                            .borrow()
                            .networks
                            .iter()
                            .find(|network| network.ssid == ssid)
                            .map(|network| network.is_secure)
                            .unwrap_or(false);
                        if state == 2 {
                            status_rx(StatusKind::Info, String::new());
                            *pending_connect_rx.borrow_mut() = None;
                            *optimistic_active_rx.borrow_mut() = None;
                            failed_connects_rx.borrow_mut().remove(&ssid);
                            request_state_refresh(&ui_tx_rx);
                        } else if state == 4 {
                            let message = if pending.from_password || is_secure {
                                "Incorrect password. Try again.".to_string()
                            } else {
                                "Failed to connect. Check signal and try again.".to_string()
                            };
                            status_rx(
                                StatusKind::Error,
                                format!("Failed to connect to {}. {message}", ssid),
                            );
                            *pending_connect_rx.borrow_mut() = None;
                            *optimistic_active_rx.borrow_mut() = None;
                            if pending.from_password || is_secure {
                                failed_connects_rx.borrow_mut().insert(ssid.clone());
                            }
                            if !pending.was_saved {
                                let ssid_cleanup = ssid.clone();
                                spawn_task(&ui_tx_rx, move || {
                                    let backend = NetworkManagerBackend::new();
                                    let result = backend.forget_network(&ssid_cleanup);
                                    UiEvent::CleanupResult { ssid: ssid_cleanup, result }
                                });
                            }
                            request_state_refresh(&ui_tx_rx);
                            if pending.from_password || is_secure {
                                let loading_retry = loading_rx.clone();
                                let header_retry = header_rx.clone();
                                let ui_tx_retry = ui_tx_rx.clone();
                                let status_container_retry = status_container_rx.clone();
                                let ssid_retry = ssid.clone();
                                let ssid_label = ssid.clone();
                                let was_saved = pending.was_saved;
                                show_password_dialog(
                                    &window_rx,
                                    &ssid_label,
                                    Some("Incorrect password. Try again.".to_string()),
                                    move |password| {
                                        loading_retry.start();
                                        update_loading_ui(header_retry.as_ref(), &loading_retry);
                                        spawn_connect_task(
                                            &ui_tx_retry,
                                            ssid_retry.clone(),
                                            password.clone(),
                                            password.is_some(),
                                            was_saved,
                                        );
                                    },
                                    (*status_container_retry).clone(),
                                );
                            }
                        }
                    }
                }
                UiEvent::CleanupResult { ssid, result } => {
                    if let Err(err) = result {
                        status_rx(
                            StatusKind::Error,
                            format!(
                                "Failed to remove saved profile for {ssid}: {}",
                                friendly_error(&err)
                            ),
                        );
                    }
                }
                UiEvent::RefreshRequested => {
                    if refresh_guard_rx.get() {
                        continue;
                    }
                    refresh_guard_rx.set(true);
                    let ui_tx = ui_tx_rx.clone();
                    let guard = refresh_guard_signal.clone();
                    gtk4::glib::timeout_add_local(Duration::from_millis(150), move || {
                        request_state_refresh(&ui_tx);
                        guard.set(false);
                        ControlFlow::Break
                    });
                }
            }
        }
        ControlFlow::Continue
    });

    window.set_child(Some(&root));
    window.present();
}

#[derive(Clone)]
struct HeaderWidgets {
    container: GtkBox,
    toggle: Switch,
    refresh: Button,
    spinner: Spinner,
    refresh_overlay: Overlay,
}

#[derive(Clone)]
struct LoadingTracker {
    active: Rc<Cell<u32>>,
}

impl LoadingTracker {
    fn new() -> Self {
        Self {
            active: Rc::new(Cell::new(0)),
        }
    }

    fn start(&self) {
        let count = self.active.get().saturating_add(1);
        self.active.set(count);
    }

    fn stop(&self) {
        let count = self.active.get();
        self.active.set(count.saturating_sub(1));
    }

    fn is_active(&self) -> bool {
        self.active.get() > 0
    }
}

fn build_header(state: &AppState) -> HeaderWidgets {
    let header = GtkBox::new(Orientation::Horizontal, 10);
    header.add_css_class("yufi-header");
    header.set_hexpand(true);

    let title = Label::new(Some("WiFi"));
    title.add_css_class("yufi-title");
    title.set_halign(Align::Start);
    title.set_hexpand(true);

    let refresh = Button::builder().icon_name("view-refresh").build();
    refresh.add_css_class("yufi-icon-button");
    refresh.add_css_class("flat");

    let spinner = Spinner::new();
    spinner.set_visible(false);
    spinner.add_css_class("yufi-spinner");
    spinner.set_halign(Align::Center);
    spinner.set_valign(Align::Center);

    let refresh_overlay = Overlay::new();
    refresh_overlay.add_css_class("yufi-refresh-slot");
    refresh_overlay.set_halign(Align::Center);
    refresh_overlay.set_size_request(36, -1);
    refresh_overlay.set_child(Some(&refresh));
    refresh_overlay.add_overlay(&spinner);

    let toggle = Switch::builder().active(state.wifi_enabled).build();

    header.append(&title);
    header.append(&refresh_overlay);
    header.append(&toggle);

    HeaderWidgets {
        container: header,
        toggle,
        refresh,
        spinner,
        refresh_overlay,
    }
}

fn update_loading_ui(header: &HeaderWidgets, loading: &LoadingTracker) {
    if loading.is_active() {
        header.spinner.start();
    } else {
        header.spinner.stop();
    }
}

fn build_search() -> SearchEntry {
    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search networks..."));
    search.add_css_class("yufi-search");
    search
}

fn build_status() -> (GtkBox, Label) {
    let status_bar = GtkBox::new(Orientation::Horizontal, 0);
    status_bar.add_css_class("yufi-status-bar");
    status_bar.set_visible(false);

    let status = Label::new(None);
    status.add_css_class("yufi-status");
    status.add_css_class("dim-label");
    status.set_halign(Align::Start);
    status.set_hexpand(true);
    status.set_visible(false);

    status_bar.append(&status);
    (status_bar, status)
}

fn build_network_list() -> ListBox {
    let list = ListBox::new();
    list.add_css_class("yufi-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_show_separators(false);

    list
}

fn build_network_row(
    network: &Network,
    action_handler: &Rc<RefCell<Option<ActionHandler>>>,
    effective_action: NetworkAction,
    is_connecting: bool,
    has_error: bool,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("yufi-row");
    if has_error {
        row.add_css_class("yufi-row-error");
    }
    row.set_activatable(true);
    row.set_widget_name(&format!("ssid:{}", network.ssid));

    let container = GtkBox::new(Orientation::Vertical, 8);
    container.set_margin_top(10);
    container.set_margin_bottom(10);
    container.set_margin_start(12);
    container.set_margin_end(12);

    let top = GtkBox::new(Orientation::Horizontal, 8);
    top.set_hexpand(true);

    let label = Label::new(Some(&network.ssid));
    label.add_css_class("yufi-network-name");
    label.set_halign(Align::Start);
    label.set_hexpand(true);

    let icon = Image::from_icon_name(network.signal_icon);
    icon.add_css_class("yufi-network-icon");
    let icon_row = GtkBox::new(Orientation::Horizontal, 6);
    icon_row.set_halign(Align::End);
    if network.is_saved {
        let saved_dot = GtkBox::new(Orientation::Horizontal, 0);
        saved_dot.add_css_class("yufi-saved-dot");
        icon_row.append(&saved_dot);
    }
    let lock_icon = if network.is_secure {
        "changes-prevent-symbolic"
    } else {
        "changes-allow-symbolic"
    };
    let lock = Image::from_icon_name(lock_icon);
    lock.add_css_class(if network.is_secure {
        "yufi-network-lock"
    } else {
        "yufi-network-lock-open"
    });
    icon_row.append(&lock);
    icon_row.append(&icon);

    top.append(&label);
    top.append(&icon_row);

    container.append(&top);

    match effective_action {
        NetworkAction::Connect => {
            if is_connecting {
                let loading = GtkBox::new(Orientation::Horizontal, 0);
                loading.set_hexpand(true);
                loading.set_halign(Align::Center);
                let spinner = Spinner::new();
                spinner.start();
                spinner.set_tooltip_text(Some("Connecting…"));
                loading.append(&spinner);
                container.append(&loading);
            } else {
                let button = Button::with_label("Connect");
                button.add_css_class("yufi-primary");
                button.add_css_class("suggested-action");
                button.set_hexpand(true);
                button.set_halign(Align::Fill);
                let ssid = network.ssid.clone();
                let is_saved = network.is_saved;
                let handler = action_handler.clone();
                button.connect_clicked(move |_| {
                    invoke_action(
                        &handler,
                        RowAction::Connect {
                            ssid: ssid.clone(),
                            is_saved,
                        },
                    )
                });
                container.append(&button);
            }
        }
        NetworkAction::Disconnect => {
            let button = Button::with_label("Disconnect");
            button.add_css_class("yufi-primary");
            button.add_css_class("suggested-action");
            button.set_hexpand(true);
            button.set_halign(Align::Fill);
            let ssid = network.ssid.clone();
            let handler = action_handler.clone();
            button.connect_clicked(move |_| {
                invoke_action(&handler, RowAction::Disconnect(ssid.clone()))
            });
            container.append(&button);
        }
        NetworkAction::None => {}
    }

    row.set_child(Some(&container));
    row
}

fn build_hidden_button() -> Button {
    let hidden = Button::with_label("Connect to Hidden Network...");
    hidden.add_css_class("yufi-footer");
    hidden.add_css_class("yufi-secondary");
    hidden
}

fn build_lock_legend() -> GtkBox {
    let legend = GtkBox::new(Orientation::Horizontal, 6);
    legend.add_css_class("yufi-legend");
    legend.set_halign(Align::Start);

    let secure_icon = Image::from_icon_name("changes-prevent-symbolic");
    secure_icon.add_css_class("yufi-network-lock");
    let secure_label = Label::new(Some("Secure"));
    secure_label.add_css_class("yufi-legend-label");

    let open_icon = Image::from_icon_name("changes-allow-symbolic");
    open_icon.add_css_class("yufi-network-lock-open");
    let open_label = Label::new(Some("Open"));
    open_label.add_css_class("yufi-legend-label");

    legend.append(&secure_icon);
    legend.append(&secure_label);
    legend.append(&open_icon);
    legend.append(&open_label);

    legend
}

fn effective_action_for(
    state: &AppState,
    network: &Network,
    optimistic_active: Option<&str>,
) -> NetworkAction {
    if !state.wifi_enabled {
        return NetworkAction::None;
    }

    if let Some(active) = optimistic_active {
        if network.ssid == active {
            return NetworkAction::Disconnect;
        }
        return NetworkAction::Connect;
    }

    network.action.clone()
}

fn populate_network_list(
    list: &ListBox,
    state: &AppState,
    action_handler: &Rc<RefCell<Option<ActionHandler>>>,
    optimistic_active: Option<&str>,
    empty_label: Option<&str>,
    pending_ssid: Option<&str>,
    failed_connects: &HashSet<String>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    if state.networks.is_empty() {
        if let Some(label) = empty_label {
            list.append(&build_empty_row(label));
        }
        return;
    }

    for network in &state.networks {
        let effective_action = effective_action_for(state, network, optimistic_active);
        let is_connecting = pending_ssid == Some(network.ssid.as_str());
        let has_error = failed_connects.contains(&network.ssid);
        list.append(&build_network_row(
            network,
            action_handler,
            effective_action,
            is_connecting,
            has_error,
        ));
    }
}

fn filter_state(state: &AppState, query: &str) -> AppState {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return state.clone();
    }

    let networks = state
        .networks
        .iter()
        .filter(|network| network.ssid.to_lowercase().contains(&query))
        .cloned()
        .collect();

    AppState {
        wifi_enabled: state.wifi_enabled,
        networks,
    }
}

fn empty_label_for(state: &AppState, query: &str, filtered_len: usize) -> Option<&'static str> {
    if !state.wifi_enabled {
        return Some("Wi-Fi is disabled");
    }
    if state.networks.is_empty() {
        return Some("No networks found");
    }
    if !query.trim().is_empty() && filtered_len == 0 {
        return Some("No matching networks");
    }
    None
}

fn build_empty_row(text: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.add_css_class("yufi-empty-row");

    let label = Label::new(Some(text));
    label.add_css_class("yufi-empty-label");
    label.add_css_class("dim-label");
    label.set_halign(Align::Start);
    label.set_margin_top(6);
    label.set_margin_bottom(6);
    label.set_margin_start(6);
    label.set_margin_end(6);

    row.set_child(Some(&label));
    row
}

fn wire_actions(
    header: &HeaderWidgets,
    list: &ListBox,
    nm_backend: &Rc<NetworkManagerBackend>,
    state_cache: &Rc<RefCell<AppState>>,
    failed_connects: &Rc<RefCell<HashSet<String>>>,
    toggle_guard: &Rc<Cell<bool>>,
    parent: &ApplicationWindow,
    status: &StatusHandler,
    status_container: &Rc<StatusContainer>,
    loading: &LoadingTracker,
    header_ref: &Rc<HeaderWidgets>,
    ui_tx: &mpsc::Sender<UiEvent>,
) {
    let status_refresh = status.clone();
    let spinner_refresh = header_ref.spinner.clone();
    let refresh_button = header_ref.refresh.clone();
    let refresh_overlay = header_ref.refresh_overlay.clone();
    let loading_refresh = loading.clone();
    let header_refresh = header_ref.clone();
    let ui_tx_refresh = ui_tx.clone();
    header.refresh.connect_clicked(move |_| {
        loading_refresh.start();
        update_loading_ui(header_refresh.as_ref(), &loading_refresh);
        spinner_refresh.start();
        refresh_button.set_sensitive(false);
        refresh_overlay.set_visible(true);
        refresh_button.set_opacity(0.0);
        spinner_refresh.set_visible(true);
        status_refresh(StatusKind::Info, "Scan requested".to_string());
        spawn_scan_task(&ui_tx_refresh);
    });

    let guard_toggle = toggle_guard.clone();
    let loading_toggle = loading.clone();
    let header_toggle = header_ref.clone();
    let ui_tx_toggle = ui_tx.clone();
    header.toggle.connect_state_set(move |_switch, state| {
        if guard_toggle.get() {
            return Propagation::Proceed;
        }

        loading_toggle.start();
        update_loading_ui(header_toggle.as_ref(), &loading_toggle);
        spawn_toggle_task(&ui_tx_toggle, state);
        Propagation::Proceed
    });

    let nm_details = nm_backend.clone();
    let window_details = parent.clone();
    let status_details = status.clone();
    let status_details_container = status_container.clone();
    let loading_details = loading.clone();
    let header_details = header_ref.clone();
    let ui_tx_details = ui_tx.clone();
    let state_details = state_cache.clone();
    let failed_details = failed_connects.clone();
    list.connect_row_activated(move |_list, row| {
        if let Some(ssid) = ssid_from_row(row) {
            let pending_error = failed_details
                .borrow()
                .get(&ssid)
                .map(|_| "Incorrect password. Try again.".to_string());
            let is_saved = state_details
                .borrow()
                .networks
                .iter()
                .find(|network| network.ssid == ssid)
                .map(|network| network.is_saved)
                .unwrap_or(false);

            if is_saved && pending_error.is_none() {
                show_network_details_dialog(
                    &window_details,
                    &ssid,
                    nm_details.clone(),
                    ui_tx_details.clone(),
                    status_details.clone(),
                    (*status_details_container).clone(),
                    failed_details.clone(),
                );
            } else {
                prompt_connect_dialog(
                    &window_details,
                    &ssid,
                    &loading_details,
                    &header_details,
                    &ui_tx_details,
                    &status_details_container,
                    false,
                    pending_error,
                );
            }
        }
    });
}

type ActionHandler = Rc<dyn Fn(RowAction)>;

#[derive(Clone, Copy)]
enum StatusKind {
    Info,
    Success,
    Error,
}

type StatusHandler = Rc<dyn Fn(StatusKind, String)>;

enum UiEvent {
    StateLoaded(Result<AppState, BackendError>),
    ScanDone(Result<(), BackendError>),
    WifiSet {
        enabled: bool,
        result: Result<(), BackendError>,
    },
    ConnectDone {
        ssid: String,
        result: Result<Option<String>, BackendError>,
        from_password: bool,
        was_saved: bool,
    },
    DisconnectDone {
        ssid: String,
        result: Result<(), BackendError>,
    },
    HiddenDone {
        ssid: String,
        result: Result<Option<String>, BackendError>,
    },
    ActiveState {
        ssid: String,
        state: u32,
    },
    CleanupResult {
        ssid: String,
        result: Result<(), BackendError>,
    },
    RefreshRequested,
}

enum RowAction {
    Connect { ssid: String, is_saved: bool },
    Disconnect(String),
}

#[derive(Clone)]
struct PendingConnect {
    ssid: String,
    was_saved: bool,
    from_password: bool,
}

const NM_BUS_NAME: &str = "org.freedesktop.NetworkManager";
const NM_OBJECT_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_DEVICE_TYPE_WIFI: u32 = 2;

fn invoke_action(action_handler: &Rc<RefCell<Option<ActionHandler>>>, action: RowAction) {
    let handler = action_handler.borrow().clone();
    if let Some(handler) = handler {
        handler(action);
    }
}

#[derive(Clone)]
struct StatusContainer {
    dialog_label: Rc<RefCell<Option<Label>>>,
}

impl StatusContainer {
    fn register_dialog_label(&self, label: &Label) {
        *self.dialog_label.borrow_mut() = Some(label.clone());
    }

    fn clear_dialog_label(&self) {
        *self.dialog_label.borrow_mut() = None;
    }

    fn show_dialog_error(&self, text: String) {
        if let Some(label) = self.dialog_label.borrow().clone() {
            label.set_text(&text);
            label.set_visible(true);
        }
    }
}

fn build_status_handler(label: &Label) -> StatusHandler {
    let label = label.clone();
    Rc::new(move |kind, text| {
        show_status(&label, kind, &text);
    })
}

fn show_status(label: &Label, kind: StatusKind, text: &str) {
    if text.is_empty() || matches!(kind, StatusKind::Info) {
        return;
    }
    label.set_text(text);
    label.set_visible(true);
    label.remove_css_class("yufi-status-ok");
    label.remove_css_class("yufi-status-error");

    match kind {
        StatusKind::Success => label.add_css_class("yufi-status-ok"),
        StatusKind::Error => label.add_css_class("yufi-status-error"),
        StatusKind::Info => {}
    }

    let timeout = match kind {
        StatusKind::Error => 5000,
        _ => 3000,
    };

    let label = label.clone();
    gtk4::glib::timeout_add_local(Duration::from_millis(timeout), move || {
        label.set_text("");
        label.set_visible(false);
        ControlFlow::Break
    });
}

fn spawn_task<F>(ui_tx: &mpsc::Sender<UiEvent>, task: F)
where
    F: FnOnce() -> UiEvent + Send + 'static,
{
    let tx = ui_tx.clone();
    thread::spawn(move || {
        let event = task();
        let _ = tx.send(event);
    });
}

fn request_state_refresh(ui_tx: &mpsc::Sender<UiEvent>) {
    spawn_task(ui_tx, || {
        let backend = NetworkManagerBackend::new();
        UiEvent::StateLoaded(backend.load_state())
    });
}

fn spawn_scan_task(ui_tx: &mpsc::Sender<UiEvent>) {
    spawn_task(ui_tx, || {
        let backend = NetworkManagerBackend::new();
        UiEvent::ScanDone(backend.request_scan())
    });
}

fn spawn_toggle_task(ui_tx: &mpsc::Sender<UiEvent>, enabled: bool) {
    spawn_task(ui_tx, move || {
        let backend = NetworkManagerBackend::new();
        UiEvent::WifiSet {
            enabled,
            result: backend.set_wifi_enabled(enabled),
        }
    });
}

fn spawn_connect_task(
    ui_tx: &mpsc::Sender<UiEvent>,
    ssid: String,
    password: Option<String>,
    from_password: bool,
    was_saved: bool,
) {
    spawn_task(ui_tx, move || {
        let backend = NetworkManagerBackend::new();
        let result = backend.connect_network(&ssid, password.as_deref());
        UiEvent::ConnectDone {
            ssid,
            result,
            from_password,
            was_saved,
        }
    });
}

fn spawn_disconnect_task(ui_tx: &mpsc::Sender<UiEvent>, ssid: String) {
    spawn_task(ui_tx, move || {
        let backend = NetworkManagerBackend::new();
        let result = backend.disconnect_network(&ssid);
        UiEvent::DisconnectDone { ssid, result }
    });
}

fn spawn_hidden_task(
    ui_tx: &mpsc::Sender<UiEvent>,
    ssid: String,
    password: Option<String>,
) {
    spawn_task(ui_tx, move || {
        let backend = NetworkManagerBackend::new();
        let result = backend.connect_hidden(&ssid, "wpa-psk", password.as_deref());
        UiEvent::HiddenDone { ssid, result }
    });
}

fn spawn_nm_signal_listeners(ui_tx: &mpsc::Sender<UiEvent>) {
    spawn_nm_properties_listener(ui_tx.clone());
    spawn_nm_state_listener(ui_tx.clone());
    spawn_wifi_device_listener(ui_tx.clone());
}

fn spawn_nm_properties_listener(ui_tx: mpsc::Sender<UiEvent>) {
    thread::spawn(move || {
        let Ok(conn) = Connection::system() else { return };
        let Ok(props) = Proxy::new(
            &conn,
            NM_BUS_NAME,
            NM_OBJECT_PATH,
            "org.freedesktop.DBus.Properties",
        ) else {
            return;
        };
        let Ok(mut stream) = props.receive_signal("PropertiesChanged") else { return };
        while let Some(signal) = stream.next() {
            let Ok((iface, changed, _invalidated)) = signal
                .body()
                .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            else {
                continue;
            };
            if iface == "org.freedesktop.NetworkManager"
                && (changed.contains_key("ActiveConnections")
                    || changed.contains_key("WirelessEnabled")
                    || changed.contains_key("PrimaryConnection"))
            {
                let _ = ui_tx.send(UiEvent::RefreshRequested);
            }
        }
    });
}

fn spawn_nm_state_listener(ui_tx: mpsc::Sender<UiEvent>) {
    thread::spawn(move || {
        let Ok(conn) = Connection::system() else { return };
        let Ok(proxy) = Proxy::new(
            &conn,
            NM_BUS_NAME,
            NM_OBJECT_PATH,
            "org.freedesktop.NetworkManager",
        ) else {
            return;
        };
        let Ok(mut stream) = proxy.receive_signal("StateChanged") else { return };
        while stream.next().is_some() {
            let _ = ui_tx.send(UiEvent::RefreshRequested);
        }
    });
}

fn spawn_wifi_device_listener(ui_tx: mpsc::Sender<UiEvent>) {
    thread::spawn(move || {
        let Ok(conn) = Connection::system() else { return };
        let Some(device_path) = find_wifi_device_path(&conn) else { return };
        let Ok(props) = Proxy::new(
            &conn,
            NM_BUS_NAME,
            device_path.as_str(),
            "org.freedesktop.DBus.Properties",
        ) else {
            return;
        };
        let Ok(mut stream) = props.receive_signal("PropertiesChanged") else { return };
        while let Some(signal) = stream.next() {
            let Ok((iface, changed, _invalidated)) = signal
                .body()
                .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            else {
                continue;
            };
            if iface == "org.freedesktop.NetworkManager.Device.Wireless"
                || iface == "org.freedesktop.NetworkManager.Device"
            {
                if changed.contains_key("ActiveAccessPoint")
                    || changed.contains_key("ActiveConnection")
                    || changed.contains_key("LastScan")
                {
                    let _ = ui_tx.send(UiEvent::RefreshRequested);
                }
            }
        }
    });
}

fn find_wifi_device_path(conn: &Connection) -> Option<OwnedObjectPath> {
    let nm = Proxy::new(
        conn,
        NM_BUS_NAME,
        NM_OBJECT_PATH,
        "org.freedesktop.NetworkManager",
    )
    .ok()?;
    let devices: Vec<OwnedObjectPath> = nm.call("GetDevices", &()).ok()?;
    for path in devices {
        let device = Proxy::new(
            conn,
            NM_BUS_NAME,
            path.as_str(),
            "org.freedesktop.NetworkManager.Device",
        )
        .ok()?;
        let device_type: u32 = device.get_property("DeviceType").ok()?;
        if device_type == NM_DEVICE_TYPE_WIFI {
            drop(device);
            return Some(path);
        }
    }
    None
}

fn spawn_active_connection_listener(
    ui_tx: &mpsc::Sender<UiEvent>,
    ssid: String,
    path: String,
) {
    let tx = ui_tx.clone();
    thread::spawn(move || {
        let Ok(conn) = Connection::system() else { return };
        let Ok(proxy) = Proxy::new(
            &conn,
            NM_BUS_NAME,
            path.as_str(),
            "org.freedesktop.NetworkManager.Connection.Active",
        ) else {
            return;
        };

        if let Ok(state) = proxy.get_property::<u32>("State") {
            let _ = tx.send(UiEvent::ActiveState {
                ssid: ssid.clone(),
                state,
            });
            if state == 2 || state == 4 {
                return;
            }
        }

        let Ok(props) = Proxy::new(
            &conn,
            NM_BUS_NAME,
            path.as_str(),
            "org.freedesktop.DBus.Properties",
        ) else {
            return;
        };
        let Ok(mut stream) = props.receive_signal("PropertiesChanged") else { return };
        while let Some(signal) = stream.next() {
            let Ok((iface, changed, _invalidated)) =
                signal
                    .body()
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            else {
                continue;
            };
            if iface != "org.freedesktop.NetworkManager.Connection.Active" {
                continue;
            }
            let Some(value) = changed.get("State") else { continue };
            let Some(state) = owned_value_to_u32(value) else { continue };
            let _ = tx.send(UiEvent::ActiveState {
                ssid: ssid.clone(),
                state,
            });
            if state == 2 || state == 4 {
                break;
            }
        }
    });
}

fn owned_value_to_u32(value: &OwnedValue) -> Option<u32> {
    let owned = value.try_clone().ok()?;
    u32::try_from(owned).ok()
}

fn needs_password(err: &BackendError) -> bool {
    match err {
        BackendError::Unavailable(message) => {
            let msg = message.to_lowercase();
            msg.contains("secrets")
                || msg.contains("password")
                || msg.contains("psk")
                || msg.contains("wireless-security")
        }
    }
}

fn password_error_message(err: &BackendError) -> String {
    match err {
        BackendError::Unavailable(message) => {
            let msg = message.to_lowercase();
            if msg.contains("nosecrets") || msg.contains("no agents") || msg.contains("no agent") {
                return "Password unavailable: no secrets agent. Start a polkit agent (e.g. polkit-gnome)."
                    .to_string();
            }
            format!("Failed to load password: {err:?}")
        }
    }
}

fn friendly_error(err: &BackendError) -> String {
    match err {
        BackendError::Unavailable(message) => {
            let msg = message.to_lowercase();
            if msg.contains("nosecrets") || msg.contains("no agents") || msg.contains("no agent") {
                return "No secrets agent. Start a polkit agent (e.g. polkit-gnome).".to_string();
            }
            if msg.contains("no wi") && msg.contains("device") {
                return "No Wi‑Fi device found.".to_string();
            }
            message.clone()
        }
    }
}

fn connect_error_message(err: &BackendError, from_password: bool) -> String {
    if from_password {
        let BackendError::Unavailable(message) = err;
        let msg = message.to_lowercase();
        if msg.contains("auth") || msg.contains("password") || msg.contains("psk") {
            return "Incorrect password. Try again.".to_string();
        }
    }
    friendly_error(err)
}

struct ParsedNetworkInput {
    ip: Option<String>,
    prefix: Option<u32>,
    gateway: Option<String>,
    dns: Option<Vec<String>>,
}

fn parse_network_inputs(
    ip_text: &str,
    gateway_text: &str,
    dns_text: &str,
) -> Result<ParsedNetworkInput, String> {
    let ip_text = ip_text.trim();
    let gateway_text = gateway_text.trim();
    let dns_text = dns_text.trim();

    let mut ip = None;
    let mut prefix = None;

    if !ip_text.is_empty() {
        if let Some((addr, pre)) = ip_text.split_once('/') {
            let addr = addr.trim();
            let pre = pre.trim();
            if addr.is_empty() {
                return Err("IP address is required".to_string());
            }
            if !is_ipv4(addr) {
                return Err("Invalid IP address".to_string());
            }
            ip = Some(addr.to_string());
            prefix = Some(parse_prefix(pre)?);
        } else {
            if !is_ipv4(ip_text) {
                return Err("Invalid IP address".to_string());
            }
            ip = Some(ip_text.to_string());
        }
    }

    let gateway = if gateway_text.is_empty() {
        None
    } else {
        if !is_ip_or_ipv6(gateway_text) {
            return Err("Invalid gateway address".to_string());
        }
        if ip.is_none() {
            return Err("Gateway requires an IP address".to_string());
        }
        Some(gateway_text.to_string())
    };

    let dns = if dns_text.is_empty() {
        None
    } else {
        let mut list = Vec::new();
        for entry in dns_text.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if !is_ip_or_ipv6(entry) {
                return Err(format!("Invalid DNS server: {entry}"));
            }
            list.push(entry.to_string());
        }
        if list.is_empty() {
            None
        } else {
            Some(list)
        }
    };

    Ok(ParsedNetworkInput {
        ip,
        prefix,
        gateway,
        dns,
    })
}

fn set_manual_fields_enabled(ip: &Entry, gateway: &Entry, dns: &Entry, enabled: bool) {
    ip.set_sensitive(enabled);
    gateway.set_sensitive(enabled);
    dns.set_sensitive(enabled);
}

fn parse_prefix(input: &str) -> Result<u32, String> {
    let prefix = input
        .parse::<u32>()
        .map_err(|_| "Invalid prefix (0-32)".to_string())?;
    if prefix > 32 {
        return Err("Invalid prefix (0-32)".to_string());
    }
    Ok(prefix)
}

fn is_ipv4(input: &str) -> bool {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        if part.parse::<u8>().is_err() {
            return false;
        }
    }
    true
}

fn is_ip_or_ipv6(input: &str) -> bool {
    if is_ipv4(input) {
        return true;
    }
    // Allow basic IPv6 literals without strict validation.
    input.contains(':')
}

fn ssid_from_row(row: &ListBoxRow) -> Option<String> {
    let name = row.widget_name();
    let name = name.as_str();
    name.strip_prefix("ssid:").map(|s| s.to_string())
}

fn show_network_details_dialog(
    parent: &ApplicationWindow,
    ssid: &str,
    backend: Rc<NetworkManagerBackend>,
    ui_tx: mpsc::Sender<UiEvent>,
    status: StatusHandler,
    status_container: StatusContainer,
    failed_connects: Rc<RefCell<HashSet<String>>>,
) {
    let dialog = Dialog::new();
    dialog.set_title(Some("Network Details"));
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_default_width(380);
    dialog.set_resizable(true);

    let content = dialog.content_area();
    let box_ = GtkBox::new(Orientation::Vertical, 10);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let error_label = Label::new(None);
    error_label.add_css_class("yufi-dialog-error");
    error_label.set_halign(Align::Start);
        error_label.set_text("");
        error_label.set_visible(true);
    status_container.register_dialog_label(&error_label);

    let title = Label::new(Some(ssid));
    title.set_halign(Align::Start);
    title.add_css_class("yufi-title");

    let password_label = Label::new(Some("Password"));
    password_label.set_halign(Align::Start);
    let password_row = GtkBox::new(Orientation::Horizontal, 8);
    password_row.set_hexpand(true);
    password_row.set_halign(Align::Fill);
    let password_entry = Entry::new();
    password_entry.set_visibility(false);
    password_entry.set_placeholder_text(Some("Hidden"));
    password_entry.set_hexpand(true);
    let reveal_button = Button::builder()
        .icon_name("view-reveal-symbolic")
        .build();
    reveal_button.add_css_class("yufi-icon-button");
    reveal_button.add_css_class("flat");
    reveal_button.set_tooltip_text(Some("Show password"));

    let reveal_state = Rc::new(Cell::new(false));
    let reveal_state_clone = reveal_state.clone();
    let backend_clone = backend.clone();
    let ssid_clone = ssid.to_string();
    let password_entry_clone = password_entry.clone();
    let status_reveal = status.clone();
    let status_reveal_container = status_container.clone();
    reveal_button.connect_clicked(move |button| {
        if reveal_state_clone.get() {
            password_entry_clone.set_text("");
            password_entry_clone.set_visibility(false);
            button.set_icon_name("view-reveal-symbolic");
            button.set_tooltip_text(Some("Show password"));
            reveal_state_clone.set(false);
            return;
        }

        match backend_clone.get_saved_password(&ssid_clone) {
            Ok(Some(password)) => {
                password_entry_clone.set_text(&password);
                password_entry_clone.set_visibility(true);
                button.set_icon_name("view-conceal-symbolic");
                button.set_tooltip_text(Some("Hide password"));
                reveal_state_clone.set(true);
            }
            Ok(None) => {
                password_entry_clone.set_text("");
                password_entry_clone.set_visibility(false);
                status_reveal(StatusKind::Info, "No saved password".to_string());
            }
            Err(err) => {
                let message = password_error_message(&err);
                status_reveal_container.show_dialog_error(message.clone());
                status_reveal(StatusKind::Error, message);
            }
        }
    });

    password_row.append(&password_entry);
    password_row.append(&reveal_button);

    let manual_fields = GtkBox::new(Orientation::Vertical, 8);

    let ip_label = Label::new(Some("IP Address"));
    ip_label.set_halign(Align::Start);
    let ip_entry = Entry::new();
    ip_entry.set_placeholder_text(Some("e.g. 192.168.1.124"));

    let gateway_label = Label::new(Some("Gateway"));
    gateway_label.set_halign(Align::Start);
    let gateway_entry = Entry::new();
    gateway_entry.set_placeholder_text(Some("e.g. 192.168.1.1"));

    let dns_label = Label::new(Some("DNS Servers"));
    dns_label.set_halign(Align::Start);
    let dns_entry = Entry::new();
    dns_entry.set_placeholder_text(Some("e.g. 1.1.1.1, 8.8.8.8"));

    let dhcp_row = GtkBox::new(Orientation::Horizontal, 8);
    let dhcp_label = Label::new(Some("Use DHCP"));
    dhcp_label.set_halign(Align::Start);
    dhcp_label.set_hexpand(true);
    let dhcp_switch = Switch::builder().active(true).build();
    dhcp_row.append(&dhcp_label);
    dhcp_row.append(&dhcp_switch);

    let auto_row = GtkBox::new(Orientation::Horizontal, 8);
    let auto_label = Label::new(Some("Auto‑reconnect"));
    auto_label.set_halign(Align::Start);
    auto_label.set_hexpand(true);
    let auto_switch = Switch::builder().active(true).build();
    auto_row.append(&auto_label);
    auto_row.append(&auto_switch);

    box_.append(&error_label);
    box_.append(&title);
    manual_fields.append(&ip_label);
    manual_fields.append(&ip_entry);
    manual_fields.append(&gateway_label);
    manual_fields.append(&gateway_entry);
    manual_fields.append(&dns_label);
    manual_fields.append(&dns_entry);

    box_.append(&password_label);
    box_.append(&password_row);
    box_.append(&dhcp_row);
    box_.append(&manual_fields);
    box_.append(&auto_row);

    let actions = GtkBox::new(Orientation::Vertical, 8);
    actions.set_hexpand(true);

    let save_button = Button::with_label("Save");
    save_button.add_css_class("yufi-primary");
    save_button.add_css_class("suggested-action");
    save_button.set_hexpand(true);
    save_button.set_halign(Align::Fill);

    let cancel_button = Button::with_label("Cancel");
    cancel_button.set_hexpand(true);
    cancel_button.set_halign(Align::Fill);
    cancel_button.add_css_class("yufi-secondary");

    let forget_button = Button::with_label("Forget Network");
    forget_button.add_css_class("destructive-action");
    forget_button.add_css_class("yufi-secondary");
    forget_button.set_hexpand(true);
    forget_button.set_halign(Align::Fill);

    let save_row = GtkBox::new(Orientation::Horizontal, 8);
    save_row.set_hexpand(true);
    save_row.append(&cancel_button);
    save_row.append(&save_button);

    actions.append(&save_row);
    actions.append(&forget_button);

    box_.append(&actions);
    content.append(&box_);
    dialog.set_default_widget(Some(&save_button));

    let details = backend
        .get_network_details(ssid)
        .unwrap_or_else(|_| NetworkDetails::default());

    let mut has_manual = false;
    if let Some(ip) = details.ip_address {
        ip_entry.set_text(&ip);
        has_manual = true;
    }
    if let Some(gateway) = details.gateway {
        gateway_entry.set_text(&gateway);
        has_manual = true;
    }
    if !details.dns_servers.is_empty() {
        dns_entry.set_text(&details.dns_servers.join(", "));
        has_manual = true;
    }
    dhcp_switch.set_active(!has_manual);
    manual_fields.set_visible(!dhcp_switch.is_active());
    if let Some(auto) = details.auto_reconnect {
        auto_switch.set_active(auto);
    }

    let backend_forget = backend.clone();
    let ssid_forget = ssid.to_string();
    let status_forget = status.clone();
    let status_container_forget = status_container.clone();
    let dialog_forget = dialog.clone();
    let parent_forget = parent.clone();
    let ui_tx_forget = ui_tx.clone();
    let failed_forget_ref = failed_connects.clone();
    forget_button.connect_clicked(move |_| {
        let confirm = MessageDialog::builder()
            .transient_for(&parent_forget)
            .modal(true)
            .message_type(MessageType::Warning)
            .text("Forget this network?")
            .secondary_text("Saved credentials and settings will be removed.")
            .build();
        confirm.add_button("Cancel", ResponseType::Cancel);
        confirm.add_button("Forget", ResponseType::Accept);
        confirm.set_default_response(ResponseType::Cancel);
        if let Some(forget_action) = confirm.widget_for_response(ResponseType::Accept) {
            forget_action.add_css_class("destructive-action");
        }
        let backend_confirm = backend_forget.clone();
        let ssid_confirm = ssid_forget.clone();
        let status_confirm = status_forget.clone();
        let status_container_confirm = status_container_forget.clone();
        let dialog_close = dialog_forget.clone();
        let ui_tx_confirm = ui_tx_forget.clone();
        let failed_confirm = failed_forget_ref.clone();
        confirm.connect_response(move |dialog, response| {
            if response == ResponseType::Accept {
                match backend_confirm.forget_network(&ssid_confirm) {
                    Ok(_) => {
                        status_confirm(StatusKind::Success, "Network forgotten".to_string());
                        status_container_confirm.clear_dialog_label();
                        dialog_close.close();
                        failed_confirm.borrow_mut().remove(&ssid_confirm);
                        request_state_refresh(&ui_tx_confirm);
                    }
                    Err(err) => {
                        status_confirm(StatusKind::Error, format!("Failed to forget: {err:?}"));
                    }
                }
            }
            dialog.close();
        });
        confirm.present();
    });

    let ip_entry = ip_entry.clone();
    let gateway_entry = gateway_entry.clone();
    let dns_entry = dns_entry.clone();
    let manual_fields_toggle = manual_fields.clone();
    let dhcp_switch_clone = dhcp_switch.clone();
    let ip_toggle = ip_entry.clone();
    let gateway_toggle = gateway_entry.clone();
    let dns_toggle = dns_entry.clone();
    dhcp_switch.connect_state_set(move |_switch, state| {
        set_manual_fields_enabled(&ip_toggle, &gateway_toggle, &dns_toggle, !state);
        manual_fields_toggle.set_visible(!state);
        Propagation::Proceed
    });

    let ip_entry = ip_entry.clone();
    let gateway_entry = gateway_entry.clone();
    let dns_entry = dns_entry.clone();
    let auto_switch = auto_switch.clone();
    let ssid = ssid.to_string();
    let status_save = status.clone();
    let status_container = status_container.clone();
    let status_container_save = status_container.clone();
    let dialog_save = dialog.clone();
    let backend_save = backend.clone();
    save_button.connect_clicked(move |_| {
        let ip_text = ip_entry.text().to_string();
        let gateway_text = gateway_entry.text().to_string();
        let dns_text = dns_entry.text().to_string();

        let parsed = match parse_network_inputs(&ip_text, &gateway_text, &dns_text) {
            Ok(parsed) => parsed,
            Err(message) => {
                status_container_save.show_dialog_error(message);
                return;
            }
        };

        let mut failed = false;
        let use_manual = !dhcp_switch_clone.is_active();
        let ip = if use_manual { parsed.ip.as_deref() } else { None };
        let gateway = if use_manual { parsed.gateway.as_deref() } else { None };
        let dns = if use_manual { parsed.dns } else { None };
        if let Err(err) = backend_save.set_ip_dns(
            &ssid,
            ip,
            parsed.prefix,
            gateway,
            dns,
        ) {
            failed = true;
            status_save(StatusKind::Error, format!("Failed to set IP/DNS: {err:?}"));
        }
        if let Err(err) = backend_save.set_autoreconnect(&ssid, auto_switch.is_active()) {
            failed = true;
            status_save(StatusKind::Error, format!("Failed to set auto‑reconnect: {err:?}"));
        }
        if !failed {
            status_save(StatusKind::Success, "Saved network settings".to_string());
        }
        status_container_save.clear_dialog_label();
        dialog_save.close();
        request_state_refresh(&ui_tx);
    });

    let dialog_cancel = dialog.clone();
    let status_container_cancel = status_container.clone();
    cancel_button.connect_clicked(move |_| {
        status_container_cancel.clear_dialog_label();
        dialog_cancel.close();
    });
    dialog.present();
}

fn prompt_connect_dialog(
    parent: &ApplicationWindow,
    ssid: &str,
    loading: &LoadingTracker,
    header: &Rc<HeaderWidgets>,
    ui_tx: &mpsc::Sender<UiEvent>,
    status_container: &Rc<StatusContainer>,
    was_saved: bool,
    initial_error: Option<String>,
) {
    let ssid = ssid.to_string();
    let ssid_label = ssid.clone();
    let ssid_connect = ssid.clone();
    let loading = loading.clone();
    let header = header.clone();
    let ui_tx = ui_tx.clone();
    let status_container = (**status_container).clone();
    show_password_dialog(
        parent,
        &ssid_label,
        initial_error,
        move |password| {
            loading.start();
            update_loading_ui(header.as_ref(), &loading);
            spawn_connect_task(
                &ui_tx,
                ssid_connect.clone(),
                password.clone(),
                password.is_some(),
                was_saved,
            );
        },
        status_container,
    );
}

fn show_password_dialog<F: Fn(Option<String>) + 'static>(
    parent: &ApplicationWindow,
    ssid: &str,
    initial_error: Option<String>,
    on_submit: F,
    status_container: StatusContainer,
) {
    let dialog = Dialog::new();
    dialog.set_title(Some("Connect to network"));
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_default_width(380);

    let content = dialog.content_area();
    let box_ = GtkBox::new(Orientation::Vertical, 8);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let label = Label::new(Some(&format!("Password for {ssid}")));
    label.set_halign(Align::Start);
    let entry = Entry::new();
    entry.set_visibility(false);
    entry.set_placeholder_text(Some("Optional (leave empty for open network)"));
    entry.add_css_class("yufi-entry");
    if initial_error.is_some() {
        entry.add_css_class("yufi-entry-error");
    }
    entry.grab_focus();
    entry.select_region(0, -1);

    box_.append(&label);
    box_.append(&entry);

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    actions.set_hexpand(true);

    let cancel_button = Button::with_label("Cancel");
    cancel_button.set_hexpand(true);
    cancel_button.set_halign(Align::Fill);

    let connect_button = Button::with_label("Connect");
    connect_button.add_css_class("yufi-primary");
    connect_button.add_css_class("suggested-action");
    connect_button.set_hexpand(true);
    connect_button.set_halign(Align::Fill);

    actions.append(&cancel_button);
    actions.append(&connect_button);
    box_.append(&actions);
    content.append(&box_);
    dialog.set_default_widget(Some(&connect_button));
    let connect_activate = connect_button.clone();
    entry.connect_activate(move |_| {
        connect_activate.emit_clicked();
    });

    let entry_clone = entry.clone();

    let dialog_connect = dialog.clone();
    let status_connect = status_container.clone();
    connect_button.connect_clicked(move |_| {
        let text = entry_clone.text().to_string();
        let password = if text.trim().is_empty() { None } else { Some(text) };
        on_submit(password);
        status_connect.clear_dialog_label();
        dialog_connect.close();
    });

    let dialog_cancel = dialog.clone();
    cancel_button.connect_clicked(move |_| {
        status_container.clear_dialog_label();
        dialog_cancel.close();
    });
    dialog.present();
}

fn show_hidden_network_dialog<F: Fn(String, Option<String>) + 'static>(
    parent: &ApplicationWindow,
    on_submit: F,
    status_container: StatusContainer,
) {
    let dialog = Dialog::new();
    dialog.set_title(Some("Hidden Network"));
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_default_width(380);

    let content = dialog.content_area();
    let box_ = GtkBox::new(Orientation::Vertical, 8);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let error_label = Label::new(None);
    error_label.add_css_class("yufi-dialog-error");
    error_label.set_halign(Align::Start);
    error_label.set_text("");
    error_label.set_visible(true);
    status_container.register_dialog_label(&error_label);

    let ssid_label = Label::new(Some("Network Name (SSID)"));
    ssid_label.set_halign(Align::Start);
    let ssid_entry = Entry::new();
    ssid_entry.set_placeholder_text(Some("e.g. Home_WiFi"));

    let pass_label = Label::new(Some("Password"));
    pass_label.set_halign(Align::Start);
    let pass_entry = Entry::new();
    pass_entry.set_visibility(false);
    pass_entry.set_placeholder_text(Some("Optional"));

    box_.append(&error_label);
    box_.append(&ssid_label);
    box_.append(&ssid_entry);
    box_.append(&pass_label);
    box_.append(&pass_entry);
    content.append(&box_);

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    actions.set_hexpand(true);

    let cancel_button = Button::with_label("Cancel");
    cancel_button.set_hexpand(true);
    cancel_button.set_halign(Align::Fill);

    let connect_button = Button::with_label("Connect");
    connect_button.add_css_class("yufi-primary");
    connect_button.add_css_class("suggested-action");
    connect_button.set_hexpand(true);
    connect_button.set_halign(Align::Fill);

    actions.append(&cancel_button);
    actions.append(&connect_button);
    box_.append(&actions);
    dialog.set_default_widget(Some(&connect_button));

    let ssid_entry = ssid_entry.clone();
    let pass_entry = pass_entry.clone();
    let error_label_clone = error_label.clone();
    ssid_entry.connect_changed(move |_| {
        error_label_clone.set_visible(false);
    });

    let dialog_connect = dialog.clone();
    let status_connect = status_container.clone();
    connect_button.connect_clicked(move |_| {
        let ssid = ssid_entry.text().to_string();
        if ssid.trim().is_empty() {
            error_label.set_text("SSID is required");
            error_label.set_visible(true);
            return;
        }
        let password = pass_entry.text().to_string();
        let pw = if password.is_empty() { None } else { Some(password) };
        on_submit(ssid, pw);
        status_connect.clear_dialog_label();
        dialog_connect.close();
    });

    let dialog_cancel = dialog.clone();
    cancel_button.connect_clicked(move |_| {
        status_container.clear_dialog_label();
        dialog_cancel.close();
    });
    dialog.present();
}

fn load_state_with_backend(
    nm_backend: &NetworkManagerBackend,
    status: &StatusHandler,
) -> AppState {
    match nm_backend.load_state() {
        Ok(state) => state,
        Err(err) => {
            status(StatusKind::Error, format!("NetworkManager error: {err:?}"));
            fallback_state(err)
        }
    }
}

fn fallback_state(_error: BackendError) -> AppState {
    AppState {
        wifi_enabled: false,
        networks: Vec::new(),
    }
}

fn load_css() {
    let css = r#"
    .yufi-panel {
        border-radius: 18px;
        padding: 12px;
    }

    .yufi-header {
        padding: 6px 4px;
    }

    .yufi-title {
        font-weight: 700;
        font-size: 16px;
    }

    .yufi-search {
        border-radius: 10px;
        padding: 6px 10px;
    }

    .yufi-list {
        background: transparent;
    }

    .yufi-row {
        border-radius: 12px;
        margin-bottom: 8px;
    }

    .yufi-network-name {
        font-weight: 600;
    }

    .yufi-network-lock {
        opacity: 0.65;
    }

    .yufi-network-lock-open {
        opacity: 0.35;
    }

    .yufi-legend {
        margin-top: 4px;
        padding: 4px 6px;
    }

    .yufi-legend-label {
        font-size: 11px;
        color: @insensitive_fg_color;
    }

    .yufi-saved-dot {
        min-width: 6px;
        min-height: 6px;
        border-radius: 999px;
        background: @accent_color;
        margin-right: 4px;
    }

    .yufi-primary {
        border-radius: 10px;
        padding: 6px 10px;
    }

    .yufi-secondary {
        border-radius: 10px;
        padding: 6px 10px;
    }

    .yufi-status {
        font-size: 12px;
    }

    .yufi-status-bar {
        padding: 2px 4px;
    }

    .yufi-status-ok {
        color: @success_color;
    }

    .yufi-status-error {
        color: @error_color;
    }

    .yufi-dialog-error {
        color: @error_color;
        font-size: 12px;
        min-height: 16px;
    }

    .yufi-entry-error {
        box-shadow: 0 0 0 1px @error_color;
    }

    .yufi-row-error {
        border: 1px solid @error_color;
    }


    .yufi-footer {
        border-radius: 12px;
        padding: 10px;
    }

    .yufi-icon-button {
        background: transparent;
        border-radius: 10px;
    }

    .yufi-spinner {
        margin-right: 2px;
        background: transparent;
    }

    .yufi-refresh-slot {
        min-width: 36px;
    }

    .yufi-empty-row {
        background: transparent;
    }

    .yufi-empty-label {
        font-size: 12px;
    }
    "#;

    let provider = CssProvider::new();
    provider.load_from_data(css);

    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
