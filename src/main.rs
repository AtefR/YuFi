mod backend;
mod models;

use backend::{Backend, BackendError};
use backend::mock::MockBackend;
use backend::nm::NetworkManagerBackend;
use gtk4::gdk::Display;
use gtk4::glib::ControlFlow;
use gtk4::glib::Propagation;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Dialog, Entry, Image,
    Label, ListBox, ListBoxRow, Orientation, ResponseType, SearchEntry, Spinner, Switch,
};
use models::{AppState, Network, NetworkAction, NetworkDetails};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

fn main() {
    let app = Application::builder()
        .application_id("com.yufi.app")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    load_css();

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
    let mock_backend = Rc::new(MockBackend::new());
    let toggle_guard = Rc::new(Cell::new(false));

    let state = load_state_with_backend(&nm_backend, &mock_backend);

    let header = build_header(&state);
    let search = build_search();
    let (status_bar, status_label) = build_status();
    let status_handler = build_status_handler(&status_label);
    let list = build_network_list();
    let action_handler: Rc<RefCell<Option<ActionHandler>>> = Rc::new(RefCell::new(None));
    populate_network_list(&list, &state, &action_handler);
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
    panel.append(&spacer);
    panel.append(&hidden);

    root.append(&panel);

    wire_actions(
        &header,
        &list,
        &nm_backend,
        &mock_backend,
        &toggle_guard,
        &action_handler,
        &window,
        &status_handler,
        &status_container,
    );

    let list_action = list.clone();
    let toggle_action = header.toggle.clone();
    let nm_action = nm_backend.clone();
    let mock_action = mock_backend.clone();
    let guard_action = toggle_guard.clone();
    let handler_ref = action_handler.clone();
    let window_ref = window.clone();
    let status_action = status_handler.clone();
    let status_container_action = status_container.clone();

    *action_handler.borrow_mut() = Some(Rc::new(move |action| {
        match action {
            RowAction::Connect(ssid) => {
                let nm_action = nm_action.clone();
                let list_action = list_action.clone();
                let toggle_action = toggle_action.clone();
                let mock_action = mock_action.clone();
                let guard_action = guard_action.clone();
                let handler_ref = handler_ref.clone();
                let ssid_clone = ssid.clone();
                let status_action = status_action.clone();

                match nm_action.connect_network(&ssid_clone, None) {
                    Ok(_) => {
                        status_action(StatusKind::Success, format!("Connected to {ssid_clone}"));
                        refresh_ui(
                            &list_action,
                            &toggle_action,
                            &nm_action,
                            &mock_action,
                            &guard_action,
                            &handler_ref,
                        );
                    }
                    Err(err) => {
                        if needs_password(&err) {
                            let nm_action = nm_action.clone();
                            let list_action = list_action.clone();
                            let toggle_action = toggle_action.clone();
                            let mock_action = mock_action.clone();
                            let guard_action = guard_action.clone();
                            let handler_ref = handler_ref.clone();
                            let status_action = status_action.clone();
                            show_password_dialog(
                                &window_ref,
                                &ssid,
                                move |password| {
                                let Some(password) = password else {
                                    status_action(StatusKind::Info, "Password required".to_string());
                                    return;
                                };
                                match nm_action.connect_network(&ssid_clone, Some(&password)) {
                                    Ok(_) => status_action(
                                        StatusKind::Success,
                                        format!("Connected to {ssid_clone}"),
                                    ),
                                    Err(err) => {
                                        status_action(StatusKind::Error, format!("Connect failed: {err:?}"));
                                    }
                                }
                                    refresh_ui(
                                        &list_action,
                                        &toggle_action,
                                        &nm_action,
                                        &mock_action,
                                        &guard_action,
                                        &handler_ref,
                                    );
                                },
                                (*status_container_action).clone(),
                            );
                        } else {
                            status_action(StatusKind::Error, format!("Connect failed: {err:?}"));
                            status_container_action.show_dialog_error(format!("{err:?}"));
                        }
                    }
                }
            }
            RowAction::Disconnect(ssid) => {
                match nm_action.disconnect_network(&ssid) {
                    Ok(_) => status_action(StatusKind::Success, format!("Disconnected from {ssid}")),
                    Err(err) => {
                        status_action(StatusKind::Error, format!("Disconnect failed: {err:?}"));
                        status_container_action.show_dialog_error(format!("{err:?}"));
                    }
                }
                refresh_ui(
                    &list_action,
                    &toggle_action,
                    &nm_action,
                    &mock_action,
                    &guard_action,
                    &handler_ref,
                );
            }
        }
    }));

    let hidden_nm = nm_backend.clone();
    let hidden_mock = mock_backend.clone();
    let hidden_list = list.clone();
    let hidden_toggle = header.toggle.clone();
    let hidden_guard = toggle_guard.clone();
    let hidden_handler = action_handler.clone();
    let hidden_window = window.clone();
    let hidden_status = status_handler.clone();
    let hidden_status_container = status_container.clone();
    hidden.connect_clicked(move |_| {
        let nm = hidden_nm.clone();
        let list = hidden_list.clone();
        let toggle = hidden_toggle.clone();
        let mock = hidden_mock.clone();
        let guard = hidden_guard.clone();
        let handler = hidden_handler.clone();
        let status = hidden_status.clone();
        let status_container = hidden_status_container.clone();
        let status_container_for_dialog = (*status_container).clone();
        show_hidden_network_dialog(
            &hidden_window,
            move |ssid, password| {
                match nm.connect_hidden(&ssid, "wpa-psk", password.as_deref()) {
                    Ok(_) => status(StatusKind::Success, format!("Connected to {ssid}")),
                    Err(err) => {
                        status(StatusKind::Error, format!("Hidden connect failed: {err:?}"));
                        status_container.show_dialog_error(format!("{err:?}"));
                    }
                }
                refresh_ui(&list, &toggle, &nm, &mock, &guard, &handler);
            },
            status_container_for_dialog,
        );
    });

    window.set_child(Some(&root));
    window.present();
}

struct HeaderWidgets {
    container: GtkBox,
    toggle: Switch,
    refresh: Button,
    spinner: Spinner,
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

    let spinner = Spinner::new();
    spinner.set_visible(false);
    spinner.add_css_class("yufi-spinner");

    let toggle = Switch::builder().active(state.wifi_enabled).build();

    header.append(&title);
    header.append(&refresh);
    header.append(&spinner);
    header.append(&toggle);

    HeaderWidgets {
        container: header,
        toggle,
        refresh,
        spinner,
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

    let status = Label::new(None);
    status.add_css_class("yufi-status");
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
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("yufi-row");
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

    top.append(&label);
    top.append(&icon);

    container.append(&top);

    match network.action {
        NetworkAction::Connect => {
            let button = Button::with_label("Connect");
            button.add_css_class("yufi-primary");
            button.set_hexpand(true);
            button.set_halign(Align::Fill);
            let ssid = network.ssid.clone();
            let handler = action_handler.clone();
            button.connect_clicked(move |_| invoke_action(&handler, RowAction::Connect(ssid.clone())));
            container.append(&button);
        }
        NetworkAction::Disconnect => {
            let button = Button::with_label("Disconnect");
            button.add_css_class("yufi-primary");
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
    hidden
}

fn populate_network_list(
    list: &ListBox,
    state: &AppState,
    action_handler: &Rc<RefCell<Option<ActionHandler>>>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    for network in &state.networks {
        list.append(&build_network_row(network, action_handler));
    }
}

fn wire_actions(
    header: &HeaderWidgets,
    list: &ListBox,
    nm_backend: &Rc<NetworkManagerBackend>,
    mock_backend: &Rc<MockBackend>,
    toggle_guard: &Rc<Cell<bool>>,
    action_handler: &Rc<RefCell<Option<ActionHandler>>>,
    parent: &ApplicationWindow,
    status: &StatusHandler,
    status_container: &StatusContainer,
) {
    let list_refresh = list.clone();
    let toggle_refresh = header.toggle.clone();
    let nm_refresh = nm_backend.clone();
    let mock_refresh = mock_backend.clone();
    let guard_refresh = toggle_guard.clone();
    let handler_refresh = action_handler.clone();
    let status_refresh = status.clone();
    let spinner_refresh = header.spinner.clone();
    let refresh_button = header.refresh.clone();
    header.refresh.connect_clicked(move |_| {
        spinner_refresh.set_visible(true);
        spinner_refresh.start();
        refresh_button.set_sensitive(false);
        match nm_refresh.request_scan() {
            Ok(_) => status_refresh(StatusKind::Info, "Scan requested".to_string()),
            Err(err) => status_refresh(StatusKind::Error, format!("Scan failed: {err:?}")),
        }
        refresh_ui(
            &list_refresh,
            &toggle_refresh,
            &nm_refresh,
            &mock_refresh,
            &guard_refresh,
            &handler_refresh,
        );

        let list_refresh = list_refresh.clone();
        let toggle_refresh = toggle_refresh.clone();
        let nm_refresh = nm_refresh.clone();
        let mock_refresh = mock_refresh.clone();
        let guard_refresh = guard_refresh.clone();
        let handler_refresh = handler_refresh.clone();
        let spinner_refresh = spinner_refresh.clone();
        let refresh_button = refresh_button.clone();
        gtk4::glib::timeout_add_local(Duration::from_millis(2000), move || {
            refresh_ui(
                &list_refresh,
                &toggle_refresh,
                &nm_refresh,
                &mock_refresh,
                &guard_refresh,
                &handler_refresh,
            );
            spinner_refresh.stop();
            spinner_refresh.set_visible(false);
            refresh_button.set_sensitive(true);
            ControlFlow::Break
        });
    });

    let list_toggle = list.clone();
    let toggle_toggle = header.toggle.clone();
    let nm_toggle = nm_backend.clone();
    let mock_toggle = mock_backend.clone();
    let guard_toggle = toggle_guard.clone();
    let handler_toggle = action_handler.clone();
    let status_toggle = status.clone();
    header.toggle.connect_state_set(move |_switch, state| {
        if guard_toggle.get() {
            return Propagation::Proceed;
        }

        match nm_toggle.set_wifi_enabled(state) {
            Ok(_) => {
                let label = if state { "Wi‑Fi enabled" } else { "Wi‑Fi disabled" };
                status_toggle(StatusKind::Success, label.to_string());
            }
            Err(err) => status_toggle(StatusKind::Error, format!("Failed to set Wi‑Fi: {err:?}")),
        }

        refresh_ui(
            &list_toggle,
            &toggle_toggle,
            &nm_toggle,
            &mock_toggle,
            &guard_toggle,
            &handler_toggle,
        );
        Propagation::Proceed
    });

    let nm_details = nm_backend.clone();
    let window_details = parent.clone();
    let status_details = status.clone();
    let status_details_container = status_container.clone();
    list.connect_row_activated(move |_list, row| {
        if let Some(ssid) = ssid_from_row(row) {
            show_network_details_dialog(
                &window_details,
                &ssid,
                nm_details.clone(),
                status_details.clone(),
                status_details_container.clone(),
            );
        }
    });
}

fn refresh_ui(
    list: &ListBox,
    toggle: &Switch,
    nm_backend: &NetworkManagerBackend,
    mock_backend: &MockBackend,
    toggle_guard: &Cell<bool>,
    action_handler: &Rc<RefCell<Option<ActionHandler>>>,
) {
    let state = load_state_with_backend(nm_backend, mock_backend);
    toggle_guard.set(true);
    toggle.set_active(state.wifi_enabled);
    toggle_guard.set(false);
    populate_network_list(list, &state, action_handler);
}

type ActionHandler = Rc<dyn Fn(RowAction)>;

#[derive(Clone, Copy)]
enum StatusKind {
    Info,
    Success,
    Error,
}

type StatusHandler = Rc<dyn Fn(StatusKind, String)>;

enum RowAction {
    Connect(String),
    Disconnect(String),
}

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
    label.set_text(text);
    label.set_visible(!text.is_empty());
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

fn needs_password(err: &BackendError) -> bool {
    match err {
        BackendError::Unavailable(message) => {
            let msg = message.to_lowercase();
            msg.contains("secrets")
                || msg.contains("password")
                || msg.contains("psk")
                || msg.contains("wireless-security")
        }
        BackendError::PermissionDenied => true,
        BackendError::NotImplemented => false,
    }
}

fn parse_ip_input<'a>(ip: &'a str, prefix: &'a str) -> (Option<&'a str>, Option<u32>) {
    let ip = ip.trim();
    if ip.is_empty() {
        return (None, None);
    }

    if !prefix.trim().is_empty() {
        let prefix = prefix.trim().parse::<u32>().ok();
        return (Some(ip), prefix.or(Some(24)));
    }

    if let Some((addr, pre)) = ip.split_once('/') {
        if let Ok(prefix) = pre.trim().parse::<u32>() {
            let addr = addr.trim();
            return if addr.is_empty() {
                (None, Some(prefix))
            } else {
                (Some(addr), Some(prefix))
            };
        }
    }

    (Some(ip), Some(24))
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
    status: StatusHandler,
    status_container: StatusContainer,
) {
    let dialog = Dialog::with_buttons(
        Some("Network Details"),
        Some(parent),
        gtk4::DialogFlags::MODAL,
        &[("Cancel", ResponseType::Cancel), ("Save", ResponseType::Accept)],
    );

    let content = dialog.content_area();
    let box_ = GtkBox::new(Orientation::Vertical, 10);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let error_label = Label::new(None);
    error_label.add_css_class("yufi-dialog-error");
    error_label.set_halign(Align::Start);
    error_label.set_visible(false);
    status_container.register_dialog_label(&error_label);

    let title = Label::new(Some(ssid));
    title.set_halign(Align::Start);
    title.add_css_class("yufi-title");

    let password_label = Label::new(Some("Password"));
    password_label.set_halign(Align::Start);
    let password_row = GtkBox::new(Orientation::Horizontal, 8);
    let password_entry = Entry::new();
    password_entry.set_visibility(false);
    password_entry.set_placeholder_text(Some("Hidden"));
    let reveal_button = Button::with_label("Reveal");
    reveal_button.add_css_class("yufi-secondary");

    let reveal_state = Rc::new(Cell::new(false));
    let reveal_state_clone = reveal_state.clone();
    let backend_clone = backend.clone();
    let ssid_clone = ssid.to_string();
    let password_entry_clone = password_entry.clone();
    let status_reveal = status.clone();
    reveal_button.connect_clicked(move |button| {
        if reveal_state_clone.get() {
            password_entry_clone.set_text("");
            password_entry_clone.set_visibility(false);
            button.set_label("Reveal");
            reveal_state_clone.set(false);
            return;
        }

        match backend_clone.get_saved_password(&ssid_clone) {
            Ok(Some(password)) => {
                password_entry_clone.set_text(&password);
                password_entry_clone.set_visibility(true);
                button.set_label("Hide");
                reveal_state_clone.set(true);
            }
            Ok(None) => {
                password_entry_clone.set_text("");
                password_entry_clone.set_visibility(false);
                status_reveal(StatusKind::Info, "No saved password".to_string());
            }
            Err(err) => {
                status_reveal(StatusKind::Error, format!("Failed to load password: {err:?}"));
            }
        }
    });

    password_row.append(&password_entry);
    password_row.append(&reveal_button);

    let ip_label = Label::new(Some("IP Address"));
    ip_label.set_halign(Align::Start);
    let ip_entry = Entry::new();
    ip_entry.set_placeholder_text(Some("e.g. 192.168.1.124"));

    let prefix_label = Label::new(Some("Prefix"));
    prefix_label.set_halign(Align::Start);
    let prefix_entry = Entry::new();
    prefix_entry.set_placeholder_text(Some("e.g. 24"));

    let gateway_label = Label::new(Some("Gateway"));
    gateway_label.set_halign(Align::Start);
    let gateway_entry = Entry::new();
    gateway_entry.set_placeholder_text(Some("e.g. 192.168.1.1"));

    let dns_label = Label::new(Some("DNS Servers"));
    dns_label.set_halign(Align::Start);
    let dns_entry = Entry::new();
    dns_entry.set_placeholder_text(Some("e.g. 1.1.1.1, 8.8.8.8"));

    let auto_row = GtkBox::new(Orientation::Horizontal, 8);
    let auto_label = Label::new(Some("Auto‑reconnect"));
    auto_label.set_halign(Align::Start);
    auto_label.set_hexpand(true);
    let auto_switch = Switch::builder().active(true).build();
    auto_row.append(&auto_label);
    auto_row.append(&auto_switch);

    box_.append(&error_label);
    box_.append(&title);
    box_.append(&password_label);
    box_.append(&password_row);
    box_.append(&ip_label);
    box_.append(&ip_entry);
    box_.append(&prefix_label);
    box_.append(&prefix_entry);
    box_.append(&gateway_label);
    box_.append(&gateway_entry);
    box_.append(&dns_label);
    box_.append(&dns_entry);
    box_.append(&auto_row);
    content.append(&box_);

    let details = backend
        .get_network_details(ssid)
        .unwrap_or_else(|_| NetworkDetails::default());

    if let Some(ip) = details.ip_address {
        ip_entry.set_text(&ip);
    }
    if let Some(prefix) = details.prefix {
        prefix_entry.set_text(&prefix.to_string());
    }
    if let Some(gateway) = details.gateway {
        gateway_entry.set_text(&gateway);
    }
    if !details.dns_servers.is_empty() {
        dns_entry.set_text(&details.dns_servers.join(", "));
    }
    if let Some(auto) = details.auto_reconnect {
        auto_switch.set_active(auto);
    }

    let ip_entry = ip_entry.clone();
    let prefix_entry = prefix_entry.clone();
    let gateway_entry = gateway_entry.clone();
    let dns_entry = dns_entry.clone();
    let auto_switch = auto_switch.clone();
    let ssid = ssid.to_string();
    let status_save = status.clone();
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            let ip_text = ip_entry.text().to_string();
            let prefix_text = prefix_entry.text().to_string();
            let gateway_text = gateway_entry.text().to_string();
            let dns_text = dns_entry.text().to_string();

            let (ip_opt, prefix_opt) = parse_ip_input(&ip_text, &prefix_text);
            let gateway_opt = if gateway_text.is_empty() {
                None
            } else {
                Some(gateway_text.as_str())
            };

            let dns_list = dns_text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let dns_opt = if dns_list.is_empty() { None } else { Some(dns_list) };

            let mut failed = false;
            if let Err(err) = backend.set_ip_dns(&ssid, ip_opt, prefix_opt, gateway_opt, dns_opt) {
                failed = true;
                status_save(StatusKind::Error, format!("Failed to set IP/DNS: {err:?}"));
            }
            if let Err(err) = backend.set_autoreconnect(&ssid, auto_switch.is_active()) {
                failed = true;
                status_save(StatusKind::Error, format!("Failed to set auto‑reconnect: {err:?}"));
            }
            if !failed {
                status_save(StatusKind::Success, "Saved network settings".to_string());
            }
        }
        status_container.clear_dialog_label();
        dialog.close();
    });
    dialog.show();
}

fn show_password_dialog<F: Fn(Option<String>) + 'static>(
    parent: &ApplicationWindow,
    ssid: &str,
    on_submit: F,
    status_container: StatusContainer,
) {
    let dialog = Dialog::with_buttons(
        Some("Connect to network"),
        Some(parent),
        gtk4::DialogFlags::MODAL,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Connect", ResponseType::Accept),
        ],
    );

    let content = dialog.content_area();
    let box_ = GtkBox::new(Orientation::Vertical, 8);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let error_label = Label::new(None);
    error_label.add_css_class("yufi-dialog-error");
    error_label.set_halign(Align::Start);
    error_label.set_visible(false);
    status_container.register_dialog_label(&error_label);

    let label = Label::new(Some(&format!("Password for {ssid}")));
    label.set_halign(Align::Start);
    let entry = Entry::new();
    entry.set_visibility(false);
    entry.set_placeholder_text(Some("Required"));

    box_.append(&error_label);
    box_.append(&label);
    box_.append(&entry);
    content.append(&box_);

    let entry_clone = entry.clone();
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            let text = entry_clone.text().to_string();
            let value = if text.is_empty() { None } else { Some(text) };
            on_submit(value);
        }
        status_container.clear_dialog_label();
        dialog.close();
    });
    dialog.show();
}

fn show_hidden_network_dialog<F: Fn(String, Option<String>) + 'static>(
    parent: &ApplicationWindow,
    on_submit: F,
    status_container: StatusContainer,
) {
    let dialog = Dialog::with_buttons(
        Some("Hidden Network"),
        Some(parent),
        gtk4::DialogFlags::MODAL,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Connect", ResponseType::Accept),
        ],
    );
    dialog.set_default_width(340);

    let content = dialog.content_area();
    let box_ = GtkBox::new(Orientation::Vertical, 8);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let error_label = Label::new(None);
    error_label.add_css_class("yufi-dialog-error");
    error_label.set_halign(Align::Start);
    error_label.set_visible(false);
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

    let ssid_entry = ssid_entry.clone();
    let pass_entry = pass_entry.clone();
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            let ssid = ssid_entry.text().to_string();
            if !ssid.is_empty() {
                let password = pass_entry.text().to_string();
                let pw = if password.is_empty() { None } else { Some(password) };
                on_submit(ssid, pw);
            }
        }
        status_container.clear_dialog_label();
        dialog.close();
    });
    dialog.show();
}

fn load_state_with_backend(
    nm_backend: &NetworkManagerBackend,
    mock_backend: &MockBackend,
) -> AppState {
    match nm_backend.load_state() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("NetworkManager backend unavailable: {err:?}. Falling back to mock data.");
            mock_backend
                .load_state()
                .unwrap_or_else(|_| fallback_state(err))
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
    .yufi-window {
        background: #2b2b2b;
        color: #e6e6e6;
        font-family: "Cantarell", "Noto Sans", sans-serif;
    }

    .yufi-panel {
        background: #2f2f2f;
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
        background: #3a3a3a;
        color: #e6e6e6;
        border-radius: 10px;
        padding: 6px 10px;
    }

    .yufi-list {
        background: transparent;
    }

    .yufi-row {
        background: #333333;
        border-radius: 12px;
        margin-bottom: 8px;
    }

    .yufi-network-name {
        font-weight: 600;
    }

    .yufi-primary {
        background: #2f7ae5;
        color: #ffffff;
        border-radius: 10px;
        padding: 6px 10px;
    }

    .yufi-secondary {
        background: #3a3a3a;
        color: #e6e6e6;
        border-radius: 10px;
        padding: 6px 10px;
    }

    .yufi-status {
        font-size: 12px;
        color: #bfbfbf;
    }

    .yufi-status-bar {
        padding: 2px 4px;
    }

    .yufi-status-ok {
        color: #9fd49f;
    }

    .yufi-status-error {
        color: #f2a3a3;
    }

    .yufi-dialog-error {
        color: #f2a3a3;
        font-size: 12px;
    }

    .yufi-footer {
        background: #3a3a3a;
        color: #cfcfcf;
        border-radius: 12px;
        padding: 10px;
    }

    .yufi-icon-button {
        background: transparent;
        border-radius: 10px;
    }

    .yufi-spinner {
        margin-right: 2px;
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
