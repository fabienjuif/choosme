use crate::config::{Config, read_css_file};
use crate::desktop_files::{DesktopFileOpenerCommand, OpenParams, resolve_desktop_files};
use gtk4::gio::{self};
use gtk4::{self as gtk, Align, Box, Image, Label, ListBox, Orientation, SelectionMode, Window};
use gtk4::{Application, Button};
use gtk4::{glib, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use tracing::{debug, error, info, warn};

pub fn start_ui(
    application_id: &str,
    application_name: &str,
    cfg: &Config,
    desktop_files_tx: Sender<DesktopFileOpenerCommand>,
    ui_rx: async_channel::Receiver<String>,
    daemon_mode: bool,
    uri: Option<String>,
) -> Application {
    let application = Application::builder()
        .application_id(application_id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let shared_files: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(uri));
    let shared_files_clone_open = Rc::clone(&shared_files);

    // connect to the 'open' signal, which is triggered when the application is launched with URIs/files.
    application.connect_open(move |app, _, _| {
        // just to avoid a GIO critical and force activation
        // the args are handled via clap in the main.rs
        app.activate();
    });

    let application_name_clone = application_name.to_string();
    let cfg_clone = cfg.clone();
    let desktop_files_clone = desktop_files_tx.clone();
    application.connect_activate(move |app| {
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

        let list_box = ListBox::builder()
            .margin_top(0)
            .margin_end(0)
            .margin_bottom(0)
            .margin_start(0)
            .selection_mode(SelectionMode::None)
            .css_classes(vec![String::from("list")])
            .build();

        let desktop_files = resolve_desktop_files(&cfg_clone);
        let desktop_files_len = desktop_files.len();
        for (idx, desktop_file_config) in cfg_clone.desktop_files.iter().enumerate(){
            let Some(desktop_file) = desktop_files.get(&desktop_file_config.id) else {
                warn!("no desktop file found for id: {}", desktop_file_config.id);
                continue;
            };
            let mut button_css_classes = vec![String::from("application")];
            if idx == 0 {
                button_css_classes.push("first".into());
            } else if idx == desktop_files_len - 1 {
                button_css_classes.push("last".into());
            }
            let button = Button::builder()
                .css_classes(button_css_classes)
                .label(desktop_file_config.alias.as_ref().map_or(desktop_file.name(), |alias| alias.into()))
                .build();

            let button_box = Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(12)
                .css_classes(vec![String::from("box")])
                .build();
            button.set_child(Some(&button_box));

            if let Some(icon) = desktop_file.icon() {
                let icon_image = Image::builder()
                .gicon(&icon)
                .css_classes(vec![String::from("icon")])
                    .pixel_size(48)
                    .margin_end(12)
                    .build();
                button_box.append(&icon_image);
            }

            button_box.append(&Label::builder()
                .label(desktop_file_config.alias.as_ref().map_or(desktop_file.name(), |alias| alias.into()))
                .css_classes(vec![String::from("label")])
                .build());

            let desktop_id_for_closure = desktop_file_config.id.clone();
            let desktop_files_tx_for_closure = desktop_files_clone.clone();
            let shared_uri_clone_active = Rc::clone(&shared_files);
            let app_for_closure = app.clone();
            button.connect_clicked(move |_| {
                let uri = shared_uri_clone_active.borrow().clone().unwrap_or_default();
                if let Err(e) = desktop_files_tx_for_closure.send(DesktopFileOpenerCommand::Open(
                    OpenParams {
                        uris: vec![uri],
                        desktop_file_id: desktop_id_for_closure.clone(),
                    },
                )) {
                    error!("failed to send command to desktop file opener: {}", e);
                }
                info!("after sending command, quitting the app");
                if daemon_mode {
                app_for_closure.windows()
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

        if desktop_files_len == 0 {
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
            .title(&application_name_clone)
            .default_width(300)
            .default_height(100)
            .decorated(false)
            .resizable(false)
            .css_classes(vec!["main-window"])
            .child(&content)
            .build();

        debug!("window is built");

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
    });

    application.connect_window_added(move |app, _| {
        debug!("window added");
        if let Some(window) = app.active_window() {
            window.connect_close_request(move |win| {
                if daemon_mode {
                    debug!("close request received, hiding window instead of closing");
                    win.hide();
                    gtk4::glib::Propagation::Stop
                } else {
                    debug!("close request received, closing window");
                    let Some(application) = win.application() else {
                        error!("no application found for window");
                        std::process::exit(1);
                    };
                    application.quit();
                    gtk4::glib::Propagation::Proceed
                }
            });
            if !daemon_mode {
                window.present();
            }
        } else {
            error!("app opened but no active window found");
        }
    });

    let app_clone = application.clone();
    glib::spawn_future_local(async move {
        loop {
            match ui_rx.recv().await {
                Ok(uri) => {
                    debug!("received URI from UI: {}", uri);
                    *shared_files_clone_open.borrow_mut() = Some(uri);
                    if let Some(win) = app_clone.active_window() {
                        win.show();
                    } else {
                        error!("no active window found");
                    }
                }
                Err(e) => {
                    error!("error receiving URI from UI: {}", e);
                    break;
                }
            }
        }
    });

    debug!("application is initialized and connected to activate signal");
    application
}
