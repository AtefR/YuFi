mod backend;
mod models;

use backend::{Backend, BackendError};
use backend::mock::MockBackend;
use backend::nm::NetworkManagerBackend;
use gtk4::gdk::Display;
use gtk4::glib::Propagation;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image, Label,
    ListBox, ListBoxRow, Orientation, SearchEntry, Switch,
};
use models::{AppState, Network, NetworkAction};
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
    );

    let list_action = list.clone();
    let toggle_action = header.toggle.clone();
    let nm_action = nm_backend.clone();
    let mock_action = mock_backend.clone();
    let guard_action = toggle_guard.clone();
    let handler_ref = action_handler.clone();

    *action_handler.borrow_mut() = Some(Rc::new(move |action| {
        match action {
            RowAction::Connect(ssid) => {
                if let Err(err) = nm_action.connect_network(&ssid, None) {
                    eprintln!("Connect failed: {err:?}");
                }
            }
            RowAction::Disconnect(ssid) => {
                if let Err(err) = nm_action.disconnect_network(&ssid) {
                    eprintln!("Disconnect failed: {err:?}");
                }
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
    }));

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
    row.set_activatable(false);

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
