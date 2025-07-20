use crate::config::read_css_file;
use crate::realm::Realm;
use crate::runner::{ApplicationOpenerCommand, OpenParams};
use gtk4::gio::{self};
use gtk4::{self as gtk, Align, Box, Image, Label, ListBox, Orientation, SelectionMode, Window};
use gtk4::{Application, Button};
use gtk4::{glib, prelude::*};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use tracing::{debug, error, info, warn};

pub enum UICommand {
    OpenWindow(OpenWindowParams),
}

#[derive(Clone)]
pub struct OpenWindowParams {
    pub realm_id: String,
    pub uris: Vec<String>,
}

pub fn start_ui(
    application_id: &str,
    application_name: &str,
    realms: HashMap<String, Realm>,
    applications_opener_tx: Sender<ApplicationOpenerCommand>,
    ui_rx: async_channel::Receiver<UICommand>,
    daemon_mode: bool,
) -> (Application, gio::ApplicationHoldGuard) {
    let application = Application::builder()
        .application_id(application_id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let shared_files: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let shared_files_clone_open = Rc::clone(&shared_files);

    // connect to the 'open' signal, which is triggered when the application is launched with URIs/files.
    application.connect_open(move |app, _, _| {
        // just to avoid a GIO critical and force activation
        // the args are handled via clap in the main.rs
        app.activate();
    });

    let application_name_clone = application_name.to_string();
    application.connect_activate(move |_app| {
        debug!("app activated");

        // css
        let display = &gtk::gdk::Display::default().expect("could not connect to a display.");
        match read_css_file() {
            Err(e) => {
                warn!("failed to read css file: {}", e);
            }
            Ok(css_content) => {
                let provider = gtk::CssProvider::new();
                provider.load_from_data(&css_content);
                gtk::style_context_add_provider_for_display(
                    display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_USER,
                );
            }
        };

        debug!("CSS is loaded");
    });

    let app_clone = application.clone();
    glib::spawn_future_local(async move {
        loop {
            match ui_rx.recv().await {
                Ok(cmd) => match cmd {
                    UICommand::OpenWindow(OpenWindowParams { realm_id, uris }) => {
                        debug!("received command to open window for realm: {}", realm_id);
                        let Some(realm) = realms.get(&realm_id) else {
                            error!("realm not found: {}", realm_id);
                            continue;
                        };
                        let title = format!("{application_name_clone} - {realm_id}");
                        let window = build_window(
                            realm,
                            daemon_mode,
                            applications_opener_tx.clone(),
                            shared_files.clone(),
                            &app_clone,
                            &title,
                        );
                        if !uris.is_empty() {
                            *shared_files_clone_open.borrow_mut() = Some(uris[0].clone());
                        }
                        window.show();
                    }
                },
                Err(e) => {
                    error!("error receiving URI from UI: {}", e);
                    break;
                }
            }
        }
    });

    debug!("application is initialized and connected to activate signal");
    let hold_guard = application.hold();
    (application, hold_guard)
}

fn build_window(
    realm: &Realm,
    daemon_mode: bool,
    applications_opener_tx: Sender<ApplicationOpenerCommand>,
    shared_files: Rc<RefCell<Option<String>>>,
    app: &Application,
    title: &str,
) -> gtk4::Window {
    let list_box = ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec![String::from("list")])
        .build();

    let realm_clone = realm.clone();
    let applications_len = realm_clone.applications.len();
    for (idx, application) in realm_clone.applications.iter().enumerate() {
        let mut button_css_classes = vec![String::from("application")];
        if idx == 0 {
            button_css_classes.push("first".into());
        } else if idx == applications_len - 1 {
            button_css_classes.push("last".into());
        }
        let button = Button::builder()
            .css_classes(button_css_classes)
            .label(application.display_name.clone())
            .build();

        let button_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .css_classes(vec![String::from("box")])
            .build();
        button.set_child(Some(&button_box));

        if let Some(icon) = application.desktop_app_info().and_then(|info| info.icon()) {
            let icon_image = Image::builder()
                .gicon(&icon)
                .css_classes(vec![String::from("icon")])
                .pixel_size(48)
                .margin_end(12)
                .build();
            button_box.append(&icon_image);
        }

        button_box.append(
            &Label::builder()
                .label(application.display_name.clone())
                .css_classes(vec![String::from("label")])
                .build(),
        );

        let desktop_id_for_closure = application.id.clone();
        let applications_opener_tx_cl = applications_opener_tx.clone();
        let shared_uri_clone_active = Rc::clone(&shared_files);
        let app_for_closure = app.clone();
        let realm_clone = realm_clone.clone();
        button.connect_clicked(move |_| {
            let uri = shared_uri_clone_active.borrow().clone().unwrap_or_default();
            if let Err(e) =
                applications_opener_tx_cl.send(ApplicationOpenerCommand::Open(OpenParams {
                    realm_id: realm_clone.id.clone(),
                    uris: vec![uri],
                    application_id: desktop_id_for_closure.clone(),
                }))
            {
                error!("failed to send command to desktop file opener: {}", e);
            }
            info!("after sending command, quitting the app");
            if daemon_mode {
                app_for_closure
                    .windows()
                    .iter()
                    .for_each(|window| window.hide());
            } else {
                app_for_closure.quit();
            }
        });
        list_box.append(&button);
    }

    let content = Box::builder()
        .orientation(Orientation::Vertical)
        .css_classes(vec!["main-box".to_string()])
        .build();

    if applications_len == 0 {
        let label = Label::builder()
                .label("No desktop entries found or processed from the list.\nPlease check the paths in `DESKTOP_FILES` constant.")
                .halign(Align::Center)
                .valign(Align::Center)
                .wrap(true)
                .build();
        content.append(&label);
    } else {
        content.append(&list_box);
    }

    let window = Window::builder()
        .application(app)
        .title(title)
        .default_width(10)
        .default_height(10)
        .decorated(false)
        .resizable(false)
        .css_classes(vec!["main-window"])
        .child(&content)
        .build();

    // mapping keyboard shortcuts
    let keys_controller = gtk::EventControllerKey::new();
    let list_box_clone = list_box.clone();
    let app_clone = app.clone();
    keys_controller.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Escape {
            if let Some(window) = app_clone.active_window() {
                if daemon_mode {
                    window.hide();
                } else {
                    app_clone.quit();
                }
            } else {
                error!("no active window found to hide");
            }
            return gtk::glib::Propagation::Stop;
        }
        if let Some(digit) = keyval.to_unicode().and_then(|c| c.to_digit(10)) {
            // adjust for 0-based indexing (key '1' maps to index 0)
            let index = digit.saturating_sub(1) as i32;

            if let Some(row) = list_box_clone.row_at_index(index) {
                info!("activating row at index: {:?}", row);

                let Some(widget) = row.child() else {
                    warn!("no child widget found in row at index: {}", index);
                    return gtk::glib::Propagation::Stop;
                };
                if let Some(button) = widget.downcast_ref::<Button>() {
                    gtk4::prelude::ButtonExt::emit_clicked(button);
                } else {
                    warn!("no button found in row at index: {}", index);
                }
                return gtk::glib::Propagation::Stop;
            }
        }
        gtk::glib::Propagation::Proceed
    });
    window.add_controller(keys_controller);

    debug!("window is connected to key controller");

    window
}
