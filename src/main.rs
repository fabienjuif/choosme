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

fn main() {
    let application_name = env!("CARGO_PKG_NAME");
    // I have to make a different name otherwise it collides with daemon mode.
    // Which makes me think I could reuse the ui application to register dbus methods maybe?
    //     ui_application.dbus_connection()
    let application_id = format!("juif.fabien.{}.client", application_name);

    // we keep the guard around for the duration of the application
    // to ensure that all logs are flushed before the application exits.
    let _guard = match init_logging(application_name) {
        Ok(g) => g,
        Err(e) => {
            error!("failed to initialize logging: {}", e);
            std::process::exit(1);
        }
    };

    // parsing arguments
    let args: Vec<String> = std::env::args().collect();
    let mut uri = None;
    if args.len() > 1 {
        uri = Some(args[1].clone());
    }

    // TODO: use clap
    let daemon_mode = match &uri {
        Some(u) if u == "daemon" => {
            // daemon mode, no uri provided
            uri = None;
            true
        }
        Some(u) => {
            // client mode, uri provided
            uri = Some(u.clone());
            false
        }
        _ => {
            // default to daemon mode
            true
        }
    };

    debug!("start main: daemon_mode: {}, uri: {:?}", daemon_mode, uri);

    // read config
    let cfg = match config::Config::read() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("failed to read config file: {}", e);
            std::process::exit(1);
        }
    };

    let (jh_dekstop_files, desktop_files_tx) = run_desktop_file_opener(cfg.clone());

    // if we have an uri maybe we can open it?
    let resolved = if let Some(uri) = &uri {
        let mut found = false;
        for desktop_file in &cfg.desktop_files {
            if desktop_file.match_uri(uri) {
                debug!("found matching desktop file: {}", desktop_file.id);
                // we have a matching desktop file, we can open the url
                if let Err(e) = desktop_files_tx.send(
                    desktop_files::DesktopFileOpenerCommand::Open(desktop_files::OpenParams {
                        uris: vec![uri.clone()],
                        desktop_file_id: desktop_file.id.clone(),
                    }),
                ) {
                    error!("failed to send open command: {}", e);
                    std::process::exit(1);
                }
                found = true;
                break;
            }
        }
        found
    } else {
        false
    };

    let (shutdown_signal_tx, shutdown_signal_rx) = mpsc::channel::<()>();
    let (ui_tx, ui_rx) = mpsc::channel::<String>();

    // register dbus in daemon mode
    let desktop_files_tx_clone = desktop_files_tx.clone();
    let jh_dbus = if daemon_mode && !resolved {
        Some(
            register_dbus(
                application_name,
                cfg.clone(),
                desktop_files_tx_clone,
                ui_tx.clone(),
                shutdown_signal_rx,
            )
            .unwrap_or_else(|e| {
                error!("failed to register dbus: {}", e);
                std::process::exit(1);
            }),
        )
    } else {
        None
    };

    // start the ui
    if !resolved {
        let desktop_files_tx_clone = desktop_files_tx.clone();
        let ui_application = start_ui(
            &application_id,
            application_name,
            &cfg,
            desktop_files_tx_clone,
            ui_rx,
            daemon_mode,
            uri,
        );

        info!("running application: {}", application_id);
        let exit_code = ui_application.run();
        if exit_code != ExitCode::SUCCESS {
            error!("UI exited with code {:?}", exit_code);
        } else {
            debug!("UI exited with code: {:?}", exit_code);
        }
    }

    // if we are here it means we want to exit the whole app
    debug!("dropping shutdown_signal_tx");
    drop(shutdown_signal_tx);

    // waiting threads
    // TODO: use tokio maybe later?
    if let Some(jh_dbus) = jh_dbus {
        info!("waiting for dbus thread to close...");
        jh_dbus.join().unwrap_or_else(|e| {
            error!("dbus thread failed: {:?}", e);
        });
        info!("dbus thread closed!");
    } else {
        info!("no dbus thread to wait for");
    }
    desktop_files_tx
        .send(desktop_files::DesktopFileOpenerCommand::Quit)
        .unwrap_or_else(|e| {
            error!("failed to send quit command to desktop file opener: {}", e);
            std::process::exit(1);
        });
    jh_dekstop_files.join().unwrap_or_else(|e| {
        error!("desktop file opener thread failed: {:?}", e);
    });
    info!("desktop file opener thread closed!");
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
