mod backend;
mod models;

use backend::{Backend, BackendError};
use backend::mock::MockBackend;
use backend::nm::NetworkManagerBackend;
use gtk4::gdk::Display;
use gtk4::glib::Propagation;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Dialog, Entry, Image,
    Label, ListBox, ListBoxRow, Orientation, ResponseType, SearchEntry, Switch,
};
use models::{AppState, Network, NetworkAction, NetworkDetails};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
    let list = build_network_list();
    let action_handler: Rc<RefCell<Option<ActionHandler>>> = Rc::new(RefCell::new(None));
    populate_network_list(&list, &state, &action_handler);
    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    let hidden = build_hidden_button();

    panel.append(&header.container);
    panel.append(&search);
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
    );

    let list_action = list.clone();
    let toggle_action = header.toggle.clone();
    let nm_action = nm_backend.clone();
    let mock_action = mock_backend.clone();
    let guard_action = toggle_guard.clone();
    let handler_ref = action_handler.clone();
    let window_ref = window.clone();

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
                show_password_dialog(&window_ref, &ssid, move |password| {
                    if let Err(err) =
                        nm_action.connect_network(&ssid_clone, password.as_deref())
                    {
                        eprintln!("Connect failed: {err:?}");
                    }
                    refresh_ui(
                        &list_action,
                        &toggle_action,
                        &nm_action,
                        &mock_action,
                        &guard_action,
                        &handler_ref,
                    );
                });
            }
            RowAction::Disconnect(ssid) => {
                if let Err(err) = nm_action.disconnect_network(&ssid) {
                    eprintln!("Disconnect failed: {err:?}");
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
    hidden.connect_clicked(move |_| {
        let nm = hidden_nm.clone();
        let list = hidden_list.clone();
        let toggle = hidden_toggle.clone();
        let mock = hidden_mock.clone();
        let guard = hidden_guard.clone();
        let handler = hidden_handler.clone();
        show_hidden_network_dialog(&hidden_window, move |ssid, password| {
            if let Err(err) = nm.connect_hidden(&ssid, "wpa-psk", password.as_deref()) {
                eprintln!("Hidden connect failed: {err:?}");
            }
            refresh_ui(&list, &toggle, &nm, &mock, &guard, &handler);
        });
    });

    window.set_child(Some(&root));
    window.present();
}

struct HeaderWidgets {
    container: GtkBox,
    toggle: Switch,
    refresh: Button,
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

    let toggle = Switch::builder().active(state.wifi_enabled).build();

    header.append(&title);
    header.append(&refresh);
    header.append(&toggle);

    HeaderWidgets {
        container: header,
        toggle,
        refresh,
    }
}

fn build_search() -> SearchEntry {
    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search networks..."));
    search.add_css_class("yufi-search");
    search
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
) {
    let list_refresh = list.clone();
    let toggle_refresh = header.toggle.clone();
    let nm_refresh = nm_backend.clone();
    let mock_refresh = mock_backend.clone();
    let guard_refresh = toggle_guard.clone();
    let handler_refresh = action_handler.clone();
    header.refresh.connect_clicked(move |_| {
        if let Err(err) = nm_refresh.request_scan() {
            eprintln!("Scan request failed: {err:?}");
        }
        refresh_ui(
            &list_refresh,
            &toggle_refresh,
            &nm_refresh,
            &mock_refresh,
            &guard_refresh,
            &handler_refresh,
        );
    });

    let list_toggle = list.clone();
    let toggle_toggle = header.toggle.clone();
    let nm_toggle = nm_backend.clone();
    let mock_toggle = mock_backend.clone();
    let guard_toggle = toggle_guard.clone();
    let handler_toggle = action_handler.clone();
    header.toggle.connect_state_set(move |_switch, state| {
        if guard_toggle.get() {
            return Propagation::Proceed;
        }

        if let Err(err) = nm_toggle.set_wifi_enabled(state) {
            eprintln!("Failed to set Wi‑Fi: {err:?}");
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
    list.connect_row_activated(move |_list, row| {
        if let Some(ssid) = ssid_from_row(row) {
            show_network_details_dialog(&window_details, &ssid, nm_details.clone());
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

fn ssid_from_row(row: &ListBoxRow) -> Option<String> {
    let name = row.widget_name();
    let name = name.as_str();
    name.strip_prefix("ssid:").map(|s| s.to_string())
}

fn show_network_details_dialog(
    parent: &ApplicationWindow,
    ssid: &str,
    backend: Rc<NetworkManagerBackend>,
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
            }
            Err(err) => {
                eprintln!("Failed to load password: {err:?}");
            }
        }
    });

    password_row.append(&password_entry);
    password_row.append(&reveal_button);

    let ip_label = Label::new(Some("IP Address"));
    ip_label.set_halign(Align::Start);
    let ip_entry = Entry::new();
    ip_entry.set_placeholder_text(Some("e.g. 192.168.1.124"));

    let dns_label = Label::new(Some("DNS Server"));
    dns_label.set_halign(Align::Start);
    let dns_entry = Entry::new();
    dns_entry.set_placeholder_text(Some("e.g. 1.1.1.1"));

    let auto_row = GtkBox::new(Orientation::Horizontal, 8);
    let auto_label = Label::new(Some("Auto‑reconnect"));
    auto_label.set_halign(Align::Start);
    auto_label.set_hexpand(true);
    let auto_switch = Switch::builder().active(true).build();
    auto_row.append(&auto_label);
    auto_row.append(&auto_switch);

    box_.append(&title);
    box_.append(&password_label);
    box_.append(&password_row);
    box_.append(&ip_label);
    box_.append(&ip_entry);
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
    if let Some(dns) = details.dns_server {
        dns_entry.set_text(&dns);
    }
    if let Some(auto) = details.auto_reconnect {
        auto_switch.set_active(auto);
    }

    let ip_entry = ip_entry.clone();
    let dns_entry = dns_entry.clone();
    let auto_switch = auto_switch.clone();
    let ssid = ssid.to_string();
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            let ip = ip_entry.text().to_string();
            let dns = dns_entry.text().to_string();
            let ip_opt = if ip.is_empty() { None } else { Some(ip.as_str()) };
            let dns_opt = if dns.is_empty() { None } else { Some(dns.as_str()) };

            if let Err(err) = backend.set_ip_dns(&ssid, ip_opt, dns_opt) {
                eprintln!("Failed to set IP/DNS: {err:?}");
            }
            if let Err(err) = backend.set_autoreconnect(&ssid, auto_switch.is_active()) {
                eprintln!("Failed to set auto‑reconnect: {err:?}");
            }
        }
        dialog.close();
    });
    dialog.show();
}

fn show_password_dialog<F: Fn(Option<String>) + 'static>(
    parent: &ApplicationWindow,
    ssid: &str,
    on_submit: F,
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

    let label = Label::new(Some(&format!("Password for {ssid}")));
    label.set_halign(Align::Start);
    let entry = Entry::new();
    entry.set_visibility(false);
    entry.set_placeholder_text(Some("Required"));

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
        dialog.close();
    });
    dialog.show();
}

fn show_hidden_network_dialog<F: Fn(String, Option<String>) + 'static>(
    parent: &ApplicationWindow,
    on_submit: F,
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

    let ssid_label = Label::new(Some("Network Name (SSID)"));
    ssid_label.set_halign(Align::Start);
    let ssid_entry = Entry::new();
    ssid_entry.set_placeholder_text(Some("e.g. Home_WiFi"));

    let pass_label = Label::new(Some("Password"));
    pass_label.set_halign(Align::Start);
    let pass_entry = Entry::new();
    pass_entry.set_visibility(false);
    pass_entry.set_placeholder_text(Some("Optional"));

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
