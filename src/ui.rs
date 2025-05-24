use crate::config::{Config, read_css_file};
use adw::glib::ExitCode;
use adw::prelude::*;
use adw::{ActionRow, Application};
use anyhow::Result;
use gtk4::gio::{self};
use gtk4::{self as gtk, Align, Box, Image, Label, ListBox, Orientation, SelectionMode, Window};
use std::cell::RefCell;
use std::rc::Rc;
use tracing::{debug, error, info, warn};

pub fn start_ui(application_id: &str, application_name: &str, cfg: &Config) -> Result<()> {
    let application = Application::builder()
        .application_id(application_id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let shared_files: Rc<RefCell<Option<Vec<gio::File>>>> = Rc::new(RefCell::new(None));
    // connect to the 'open' signal, which is triggered when the application is launched with URIs/files.
    let shared_files_clone_open = Rc::clone(&shared_files);
    let cfg_clone_open = cfg.clone();
    application.connect_open(move |app, files, hint| {
        if !hint.is_empty() {
            info!("xdg-open provided us an hint: {:?}", hint);
        }
        debug!("app connected");
        if let Some(file) = files.first() {
            debug!("received `open` signal with file: {:?}", file);
            for desktop_file_config in cfg_clone_open.desktop_files.iter() {
                match desktop_file_config.open_on_match(files, None) {
                    Ok(true) => {
                        app.quit();
                        return;
                    }
                    Ok(false) => {
                        // do nothing
                    }
                    Err(e) => {
                        error!("failed on open_on_match: {}", e);
                    }
                }
            }
            *shared_files_clone_open.borrow_mut() = Some(files.to_vec());
        }
        // this ensures the window is shown even if the open signal is used.
        app.activate();
    });

    let application_name_clone = application_name.to_string();
    let cfg_clone = cfg.clone();
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
        for desktop_file_config in cfg_clone.desktop_files.iter() {
            let app_info = desktop_file_config.app_info.clone();
            let row = ActionRow::builder()
                .activatable(true)
                .title(desktop_file_config.alias.as_ref().map_or(app_info.name(), |alias| alias.into()))
                .css_classes(vec![String::from("row")])
                .build();
            if let Some(icon) = app_info.icon() {
                    let icon_image = Image::builder()
                    .gicon(&icon)
                        .pixel_size(48)
                        .margin_end(12)
                        .build();
                    row.add_prefix(&icon_image);
            }

            let name_for_closure = app_info.name().clone();
            let shared_files_clone_active = Rc::clone(&shared_files);
            let app_for_closure = app.clone();
            row.connect_activated(move |_| {
                let files = shared_files_clone_active.borrow().clone().unwrap_or_default();
                if let Err(e) = app_info.launch(files.as_slice(), None::<&gio::AppLaunchContext>) {
                    // TODO: dialog
                    error!("failed to launch '{}' via GIO: {}", name_for_closure, e);
                }
                app_for_closure.quit();
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
                app_clone.quit();
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

        // floating window initially and then become resizable in WMs like Sway.
        app.connect_active_window_notify(|app| {
            if let Some(active_window) = app.active_window() {
                debug!("window is active");
                // TODO: sync.Once here (otherwise this code triggers everytime the window is focused)
                active_window.set_resizable(true);

                // TODO: make this optional (via a config file)
                // if let Some((x, y)) = get_cursor_position() {
                //     let target_x = x; // TODO: maybe config offset?
                //     let target_y = y; // TODO: maybe config offset?
                //     if let Some( app_id) = app.application_id() {
                //         let cmd = format!(
                //             "[app_id=\"{}\"] floating enable, move position {} {}",
                //             app_id, target_x, target_y
                //         );
                //         let _ = Command::new("swaymsg").arg(&cmd).status();
                //     }
                // } else {
                //     eprintln!("Failed to get cursor position.");
                // }
            }
        });

        window.present();
    });

    debug!("application is initialized and connected to activate signal");

    let exit_code = application.run();
    if exit_code != ExitCode::SUCCESS {
        return Err(anyhow::anyhow!(
            "application exited with code {:?}",
            exit_code
        ));
    }
    Ok(())
}
