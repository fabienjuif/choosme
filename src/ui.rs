use crate::config::{Config, read_css_file};
use crate::desktop_files::{DesktopFileOpenerCommand, OpenParams, resolve_desktop_files};
use adw::{ActionRow, Application};
use adw::{glib, prelude::*};
use gtk4::gio::{self};
use gtk4::{self as gtk, Align, Box, Image, Label, ListBox, Orientation, SelectionMode, Window};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use tracing::{debug, error, info, warn};

pub fn start_ui(
    application_id: &str,
    application_name: &str,
    cfg: &Config,
    desktop_files_tx: Sender<DesktopFileOpenerCommand>,
    ui_rx: Receiver<String>,
) -> Application {
    let application = Application::builder()
        .application_id(application_id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let shared_files: Rc<RefCell<Option<Vec<gio::File>>>> = Rc::new(RefCell::new(None));
    let shared_files_clone_open = Rc::clone(&shared_files);

    // connect to the 'open' signal, which is triggered when the application is launched with URIs/files.
    // let cfg_clone = cfg.clone();
    // let desktop_files_tx_clone = desktop_files_tx.clone();
    application.connect_open(move |app, _, _| {
        // just to avoid a GIO critical and force activation
        // the args are handled via clap in the main.rs
        app.activate();
    });
    //     if !hint.is_empty() {
    //         info!("xdg-open provided us an hint: {:?}", hint);
    //     }
    //     debug!("app opened");
    //     if let Some(file) = files.first() {
    //         debug!("received `open` signal with file: {:?}", file);
    //         if let Some(desktop_file) = cfg_clone.find_matching_desktop_file(file.uri().as_str()) {
    //             info!("found matching desktop file: {:?}", desktop_file.id);
    //             // send command to desktop file opener
    //             if let Err(e) =
    //                 desktop_files_tx_clone.send(DesktopFileOpenerCommand::Open(OpenParams {
    //                     uris: files.iter().map(|f| f.uri().as_str().to_string()).collect(),
    //                     desktop_file_id: desktop_file.id.clone(),
    //                 }))
    //             {
    //                 error!("failed to send command to desktop file opener: {}", e);
    //             }
    //             app.quit();
    //         }
    //     }
    // });

    let application_name_clone = application_name.to_string();
    let cfg_clone = cfg.clone();
    let desktop_files_clone = desktop_files_tx.clone();
    application.connect_activate(move |app| {
        debug!("app activated");

         // css
         match read_css_file() {
            Err(e) => {
                warn!("failed to read css file: {}", e);
            }
            Ok(css_content) => {
                let provider = gtk::CssProvider::new();
                provider.load_from_data(&css_content);
                gtk::style_context_add_provider_for_display(
                    &gtk::gdk::Display::default().expect("could not connect to a display."),
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        };

        debug!("CSS is loaded");

        let list_box = ListBox::builder()
            .margin_top(12)
            .margin_end(12)
            .margin_bottom(12)
            .margin_start(12)
            .selection_mode(SelectionMode::None)
            .css_classes(vec![String::from("boxed-list")])
            .build();

        let mut items_added = 0;
        let desktop_files = resolve_desktop_files(&cfg_clone);
        for desktop_file_config in &cfg_clone.desktop_files{
            let Some(desktop_file) = desktop_files.get(&desktop_file_config.id) else {
                warn!("no desktop file found for id: {}", desktop_file_config.id);
                continue;
            };
            let row = ActionRow::builder()
                .activatable(true)
                .title(desktop_file_config.alias.as_ref().map_or(desktop_file.name(), |alias| alias.into()))
                .css_classes(vec![String::from("row")])
                .build();
            if let Some(icon) = desktop_file.icon() {
                    let icon_image = Image::builder()
                    .gicon(&icon)
                        .pixel_size(48)
                        .margin_end(12)
                        .build();
                    row.add_prefix(&icon_image);
            }

            let desktop_id_for_closure = desktop_file_config.id.clone();
            let desktop_files_tx_for_closure = desktop_files_clone.clone();
            let shared_files_clone_active = Rc::clone(&shared_files);
            let app_for_closure = app.clone();
            row.connect_activated(move |_| {
                let files = shared_files_clone_active.borrow().clone().unwrap_or_default();
                if let Err(e) = desktop_files_tx_for_closure.send(DesktopFileOpenerCommand::Open(
                    OpenParams {
                        uris: files.iter().map(|f| f.uri().as_str().to_string()).collect(),
                        desktop_file_id: desktop_id_for_closure.clone(),
                    },
                )) {
                    error!("failed to send command to desktop file opener: {}", e);
                }
                info!("after sending command, quitting the app");
                app_for_closure.windows()
                    .iter()
                    .for_each(|window| window.hide());
            });
            list_box.append(&row);
            items_added += 1;
        }

        let content = Box::new(Orientation::Vertical, 0);

        if items_added == 0 {
            let label = Label::builder()
                .label("No desktop entries found or processed from the list.\nPlease check the paths in `DESKTOP_FILES` constant.")
                .halign(Align::Center)
                .valign(Align::Center)
                .margin_top(20)
                .margin_bottom(20)
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
                    window.hide();
                } else {
                    error!("no active window found to hide");
                }
                return gtk::glib::Propagation::Stop;
            }
            if let Some(digit) = keyval.to_unicode().and_then(|c| c.to_digit(10)) {
                // adjust for 0-based indexing (key '1' maps to index 0)
                let index = digit.saturating_sub(1) as i32;

                if let Some(row) = list_box_clone.row_at_index(index) {
                    if let Some(action_row) = row.downcast_ref::<ActionRow>() {
                        adw::prelude::ActionRowExt::activate(action_row);
                        return gtk::glib::Propagation::Stop;
                    }
                }
            }
            gtk::glib::Propagation::Proceed
        });
        window.add_controller(keys_controller);

        debug!("window is connected to key controller");
    });

    let app_clone = application.clone();
    glib::source::idle_add_local(move || {
        match ui_rx.try_recv() {
            Ok(uri) => {
                *shared_files_clone_open.borrow_mut() = Some(vec![gio::File::for_uri(&uri)]);
                if let Some(win) = app_clone.active_window() {
                    win.show();
                } else {
                    error!("no active window found");
                }
            }
            Err(TryRecvError::Empty) => {
                // nothing to do, continue looping
            }
            Err(TryRecvError::Disconnected) => {
                info!("ui_rx disconnected");
                return glib::ControlFlow::Break;
            }
        }
        glib::ControlFlow::Continue
    });

    debug!("application is initialized and connected to activate signal");
    application
}
