use adw::prelude::*;
use adw::{ActionRow, Application};
use anyhow::Result;
use gtk4::gio::{self, DesktopAppInfo};
use gtk4::{self as gtk, Align, Box, Image, Label, ListBox, Orientation, SelectionMode, Window};
use serde::Deserialize;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::{env, fs};
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;
use xdg::BaseDirectories;

fn main() {
    let application_name = env!("CARGO_PKG_NAME");
    let application_id = format!("juif.fabien.{}", application_name);

    // we keep the guard around for the duration of the application
    // to ensure that all logs are flushed before the application exits.
    let _guard = match init_logging(application_name) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("failed to initialize logging: {}", e);
            std::process::exit(1);
        }
    };

    let cfg = read_config_file()
        .map_err(|e| {
            error!("failed to read config: {}", e);
        })
        .ok();

    if cfg.is_none() {
        error!("app need to be configured for now.");
        return;
    }
    let cfg = cfg.unwrap();
    let cfg = Config::new_from_config_file(cfg);

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

    application.connect_activate(move |app| {
         // --- Inject hardcoded CSS ---
        let provider = gtk::CssProvider::new();
        provider.load_from_data(
            "
            ",
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("could not connect to a display."),
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
        for desktop_file_config in cfg.desktop_files.iter() {
            let app_info = desktop_file_config.app_info.clone();
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
            .title(application_name)
            .default_width(300)
            .default_height(100)
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

#[derive(Debug, Deserialize)]
struct DesktopFileConfigFile {
    // TODO: make path optional, and just resolve by name
    path: String,
    prefixes: Option<Vec<String>>,
    // TODO:
    // regexps: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(rename = "application")]
    desktop_files: Vec<DesktopFileConfigFile>,
}

fn read_config_file() -> Result<ConfigFile> {
    let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
    let config_path = xdg_dirs.place_config_file("config.toml")?;
    info!("config path: {}", config_path.display());

    let config_content = fs::read_to_string(&config_path)?;
    let config: ConfigFile = toml::from_str(&config_content)?;

    Ok(config)
}

#[derive(Clone)]
struct DesktopFileConfig {
    app_info: DesktopAppInfo,
    prefixes: Vec<String>,
}

impl DesktopFileConfig {
    fn open_on_match(
        &self,
        files: &[gio::File],
        context: Option<&gio::AppLaunchContext>,
    ) -> Result<bool> {
        if self.prefixes.is_empty() {
            return Ok(false);
        }
        let Some(file) = files.first() else {
            return Ok(false);
        };
        for prefix in &self.prefixes {
            let file_path = file.uri().to_string();
            if file_path.starts_with(prefix) {
                return self.try_open(files, context).map(|_| Ok(true))?;
            }
        }
        Ok(false)
    }

    fn try_open(&self, files: &[gio::File], context: Option<&gio::AppLaunchContext>) -> Result<()> {
        // let app_info = self.app_info.clone();
        self.app_info.launch(files, context)?;
        Ok(())
    }
}

#[derive(Clone)]
struct Config {
    desktop_files: Vec<DesktopFileConfig>,
}

impl Config {
    fn new_from_config_file(config_file: ConfigFile) -> Self {
        let home_dir_str = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();

        let mut desktop_files = Vec::new();
        for file in config_file.desktop_files {
            let desktop_file_path_str = &file.path;
            let mut desktop_file_path_buf = PathBuf::from(desktop_file_path_str);

            if let Some(end) = desktop_file_path_str.strip_prefix("~/") {
                if let Some(h_dir_path_str) = home_dir_str.as_ref() {
                    let mut h_dir_path_buf = PathBuf::from(h_dir_path_str);
                    h_dir_path_buf.push(end);
                    desktop_file_path_buf = h_dir_path_buf;
                } else {
                    warn!(
                        "unable to to resolve '~' in path: {}",
                        desktop_file_path_str
                    );
                    continue;
                }
            }
            let desktop_file_path = desktop_file_path_buf.as_path();
            if !desktop_file_path.exists() {
                warn!(
                    "desktop file not found, skipping: {}",
                    desktop_file_path_str
                );
                continue;
            }
            let Some(app_info) = DesktopAppInfo::from_filename(desktop_file_path) else {
                warn!(
                    "unknown or corrupted desktop file '{:?}'",
                    desktop_file_path
                );
                continue;
            };
            desktop_files.push(DesktopFileConfig {
                app_info,
                prefixes: file.prefixes.unwrap_or_default(),
            });
        }
        Config { desktop_files }
    }
}

// the returned guard must be held for the duration you want logging to occur.
// when it is dropped, any buffered logs are flushed.
fn init_logging(application_name: &str) -> Result<WorkerGuard> {
    let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
    let log_directory: PathBuf = xdg_dirs.create_state_directory("logs")?;
    let file_appender = tracing_appender::rolling::daily(log_directory, application_name);
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);
    let env_filter = EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into());
    let file_subscriber = tracing_subscriber::fmt::layer().with_writer(non_blocking_writer);
    let console_subscriber = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(console_subscriber)
        .with(env_filter)
        .init();

    Ok(_guard)
}
