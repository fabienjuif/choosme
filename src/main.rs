mod config;
mod daemon;
mod dbus;
mod desktop_files;
mod ui;

use adw::gio::prelude::ApplicationExtManual;
use adw::glib::ExitCode;
use anyhow::Result;
use daemon::register_dbus;
use desktop_files::run_desktop_file_opener;
use gtk4::prelude::{GtkApplicationExt, GtkWindowExt};
use std::env;
use std::path::PathBuf;
use std::sync::mpsc;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;
use ui::start_ui;
use xdg::BaseDirectories;

const DAEMON_MODE: bool = true; // TODO: use clap to parse this

fn main() {
    debug!("start main");
    let application_name = env!("CARGO_PKG_NAME");
    // I have to make a different name otherwise it collides with daemon mode.
    // Which makes me think I could reuse the ui application to register dbus methods maybe?
    //     ui_application.dbus_connection()

    let application_id = format!("juif.fabien.ui.{}", application_name);

    // we keep the guard around for the duration of the application
    // to ensure that all logs are flushed before the application exits.
    let _guard = match init_logging(application_name) {
        Ok(g) => g,
        Err(e) => {
            error!("failed to initialize logging: {}", e);
            std::process::exit(1);
        }
    };
    debug!("logging is initialized");

    // read config
    let cfg = match config::Config::read() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("failed to read config file: {}", e);
            std::process::exit(1);
        }
    };

    let (ui_tx, ui_rx) = mpsc::channel::<String>();
    let (jh_dekstop_files, desktop_files_tx) = run_desktop_file_opener(cfg.clone());

    // register dbus in daemon mode
    // TODO: parse with clap if we really need it
    let jh_dbus = register_dbus(
        application_name,
        cfg.clone(),
        desktop_files_tx.clone(),
        ui_tx,
    )
    .unwrap_or_else(|e| {
        error!("failed to register dbus: {}", e);
        std::process::exit(1);
    });

    // start the ui
    // TODO: only if NOT a client mode, aka no daemon mode, not able to contact the daemon
    let ui_application = start_ui(
        &application_id,
        application_name,
        &cfg,
        desktop_files_tx,
        ui_rx,
    );

    // only if NOT daemon mode NOR connected to a daemon
    if !DAEMON_MODE {
        ui_application.connect_window_added(|app, _| {
            info!("OPENED");
            if let Some(window) = app.active_window() {
                window.present();
            } else {
                error!("app opened but no active window found");
            }
        });
    }

    info!("running application: {}", application_id);
    let exit_code = ui_application.run();
    if exit_code != ExitCode::SUCCESS {
        error!("UI exited with code {:?}", exit_code);
    }

    // waiting threads
    // TODO: use tokio maybe later?
    jh_dekstop_files.join().unwrap_or_else(|e| {
        error!("desktop file opener thread failed: {:?}", e);
    });
    jh_dbus.join().unwrap_or_else(|e| {
        error!("dbus thread failed: {:?}", e);
    });
}

// the returned guard must be held for the duration you want logging to occur.
// when it is dropped, any buffered logs are flushed.
fn init_logging(application_name: &str) -> Result<WorkerGuard> {
    let xdg_dirs = BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"));
    let log_directory: PathBuf = xdg_dirs.create_state_directory("logs")?;
    let file_appender = tracing_appender::rolling::daily(log_directory, application_name);
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let file_subscriber = tracing_subscriber::fmt::layer().with_writer(non_blocking_writer);
    let console_subscriber = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(console_subscriber)
        .with(env_filter)
        .init();

    Ok(_guard)
}
