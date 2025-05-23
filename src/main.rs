use adw::{ActionRow, Application};
use adw::{MessageDialog, prelude::*};
use gtk4::gio::{self, DesktopAppInfo};
use gtk4::{self as gtk, Align, Box, Image, Label, ListBox, Orientation, SelectionMode, Window};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

const DESKTOP_FILES: &[&str] = &[
    "/home/fabien/.local/share/applications/firefox.desktop",
    "/home/fabien/.local/share/applications/firefox-cantina.desktop",
    "/usr/share/applications/thunar.desktop",
];

fn main() {
    let application_name = env!("CARGO_PKG_NAME");
    let application_id = format!("juif.fabien.{}", application_name);

    let application = Application::builder()
        .application_id(application_id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let shared_files: Rc<RefCell<Option<Vec<gio::File>>>> = Rc::new(RefCell::new(None));
    // connect to the 'open' signal, which is triggered when the application is launched with URIs/files.
    let shared_files_clone_open = Rc::clone(&shared_files);
    application.connect_open(move |app, files, _hint| {
        if let Some(file) = files.first() {
            println!("Choosme received `open` signal with file: {:?}", file);
            *shared_files_clone_open.borrow_mut() = Some(files.to_vec());
        }
        // this ensures the window is shown even if the open signal is used.
        app.activate();
    });

    application.connect_activate(move |app| {
         // --- Inject hardcoded CSS ---
        let provider = gtk::CssProvider::new();
        provider.load_from_data(
            "
            ",
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("Could not connect to a display."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        // --- End CSS Injection ---

        let list_box = ListBox::builder()
            .margin_top(12)
            .margin_end(12)
            .margin_bottom(12)
            .margin_start(12)
            .selection_mode(SelectionMode::None)
            .css_classes(vec![String::from("boxed-list")])
            .build();

        let mut items_added = 0;
        for desktop_file_path_str in DESKTOP_FILES.iter() {
            let desktop_file_path = Path::new(desktop_file_path_str);

            if !desktop_file_path.exists() {
                eprintln!(
                    "Info: Desktop file not found, skipping: {}",
                    desktop_file_path_str
                );
                continue;
            }

            let Some(app_info) = DesktopAppInfo::from_filename(desktop_file_path_str) else {
                eprintln!("Unknown or corrupted desktop file '{}'", desktop_file_path_str);
                let dialog = MessageDialog::builder()
                    .heading(format!("Error for {}", desktop_file_path_str))
                    .body(format!("Unknown or corrupted desktop file: {}", desktop_file_path_str))
                    // .transient_for(&window)
                    .modal(true)
                    .build();
                dialog.add_response("ok", "OK");
                dialog.set_default_response(Some("ok"));
                dialog.connect_response(None, |d, _| d.close());
                dialog.present();
                continue;
            };

            let row = ActionRow::builder()
                .activatable(true)
                .title(app_info.name())
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
                    eprintln!("Failed to launch '{}' via GIO: {}", name_for_closure, e);
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
            .title(application_name)
            .default_width(300)
            .default_height(100)
            .decorated(false)
            .resizable(false)
            .css_classes(vec!["main-window"])
            .child(&content)
            .build();

        // floating window initially and then become resizable in WMs like Sway.
        app.connect_active_window_notify(|app| {
            if let Some(active_window) = app.active_window() {
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

    application.run();
}

// fn get_cursor_position() -> Option<(i32, i32)> {
//     let out = Command::new("slurp")
//         .arg("-p")
//         .arg("-f")
//         .arg("%x %y") // only get cursor coordinates, no selection
//         .output();

//     if let Err(e) = out {
//         eprintln!("Failed to execute swaymsg: {}", e);
//         return None;
//     }

//     println!("out: {:?}", out);

//     let json: serde_json::Value = serde_json::from_slice(&out.ok()?.stdout).ok()?;
//     let cursor = &json[0]["cursor"];
//     Some((cursor["x"].as_f64()? as i32, cursor["y"].as_f64()? as i32))
// }
