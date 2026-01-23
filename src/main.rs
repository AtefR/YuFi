use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image,
    Label, ListBox, ListBoxRow, Orientation, SearchEntry, Switch};

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

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(14);
    root.set_margin_bottom(14);
    root.set_margin_start(14);
    root.set_margin_end(14);

    let header = build_header();
    let search = build_search();
    let list = build_network_list();
    let footer = build_footer();

    root.append(&header);
    root.append(&search);
    root.append(&list);
    root.append(&footer);

    window.set_child(Some(&root));
    window.present();
}

fn build_header() -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 10);
    header.add_css_class("yufi-card");
    header.set_margin_bottom(2);
    header.set_hexpand(true);

    let title = Label::new(Some("WiFi"));
    title.add_css_class("yufi-title");
    title.set_halign(Align::Start);
    title.set_hexpand(true);

    let refresh = Button::builder()
        .icon_name("view-refresh")
        .build();
    refresh.add_css_class("yufi-icon-button");

    let toggle = Switch::builder().active(true).build();

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

fn build_network_list() -> ListBox {
    let list = ListBox::new();
    list.add_css_class("yufi-list");
    list.set_selection_mode(gtk4::SelectionMode::None);

    let rows = [
        ("Home_Fiber_5G", true),
        ("Office_Main", false),
        ("Coffee_Shop_Free", false),
        ("Guest_Network", false),
        ("Linksys_502", false),
    ];

    for (name, connected) in rows {
        list.append(&build_network_row(name, connected));
    }

    list
}

fn build_network_row(name: &str, connected: bool) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("yufi-card");

    let container = GtkBox::new(Orientation::Vertical, 8);
    container.set_margin_top(10);
    container.set_margin_bottom(10);
    container.set_margin_start(12);
    container.set_margin_end(12);

    let top = GtkBox::new(Orientation::Horizontal, 8);
    top.set_hexpand(true);

    let label = Label::new(Some(name));
    label.add_css_class("yufi-network-name");
    label.set_halign(Align::Start);
    label.set_hexpand(true);

    let icon = Image::from_icon_name("network-wireless-signal-excellent");
    icon.add_css_class("yufi-network-icon");

    top.append(&label);
    top.append(&icon);

    let action = if connected {
        let button = Button::with_label("Disconnect");
        button.add_css_class("yufi-primary");
        button
    } else {
        let button = Button::with_label("Connect");
        button.add_css_class("yufi-primary");
        button
    };

    container.append(&top);
    container.append(&action);

    row.set_child(Some(&container));
    row
}

fn build_footer() -> GtkBox {
    let footer = GtkBox::new(Orientation::Vertical, 0);
    footer.set_hexpand(true);
    footer.set_vexpand(true);

    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);

    let hidden = Button::with_label("Connect to Hidden Network...");
    hidden.add_css_class("yufi-footer");

    footer.append(&spacer);
    footer.append(&hidden);
    footer
}

fn load_css() {
    let css = r#"
    .yufi-window {
        background: #2c2c2c;
        color: #e6e6e6;
        font-family: "Cantarell", "Noto Sans", sans-serif;
    }

    .yufi-card {
        background: #333333;
        border-radius: 14px;
        padding: 10px;
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
    provider.load_from_data(css.as_bytes());

    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
