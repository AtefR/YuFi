mod backend;
mod models;

use backend::mock::MockBackend;
use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image, Label,
    ListBox, ListBoxRow, Orientation, SearchEntry, Switch,
};
use models::{AppState, Network, NetworkAction};

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

    let backend = MockBackend::new();
    let state = backend.state();

    let header = build_header(&state);
    let search = build_search();
    let list = build_network_list(&state);
    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    let hidden = build_hidden_button();

    panel.append(&header);
    panel.append(&search);
    panel.append(&list);
    panel.append(&spacer);
    panel.append(&hidden);

    root.append(&panel);

    window.set_child(Some(&root));
    window.present();
}

fn build_header(state: &AppState) -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 10);
    header.add_css_class("yufi-header");
    header.set_hexpand(true);

    let title = Label::new(Some("WiFi"));
    title.add_css_class("yufi-title");
    title.set_halign(Align::Start);
    title.set_hexpand(true);

    let refresh = Button::builder()
        .icon_name("view-refresh")
        .build();
    refresh.add_css_class("yufi-icon-button");

    let toggle = Switch::builder().active(state.wifi_enabled).build();

    header.append(&title);
    header.append(&refresh);
    header.append(&toggle);
    header
}

fn build_search() -> SearchEntry {
    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search networks..."));
    search.add_css_class("yufi-search");
    search
}

fn build_network_list(state: &AppState) -> ListBox {
    let list = ListBox::new();
    list.add_css_class("yufi-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_show_separators(false);

    for network in &state.networks {
        list.append(&build_network_row(network));
    }

    list
}

fn build_network_row(network: &Network) -> ListBoxRow {
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
            container.append(&button);
        }
        NetworkAction::Disconnect => {
            let button = Button::with_label("Disconnect");
            button.add_css_class("yufi-primary");
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
