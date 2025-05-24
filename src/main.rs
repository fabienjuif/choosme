mod config;
mod daemon;
mod dbus;
mod ui;

use anyhow::Result;
use config::read_config;
use daemon::register_dbus;
use std::env;
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;
use ui::start_ui;
use xdg::BaseDirectories;

fn main() {
    debug!("start main");
    let application_name = env!("CARGO_PKG_NAME");
    let application_id = format!("juif.fabien.{}", application_name);

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

    let cfg = read_config().unwrap_or_else(|e| {
        error!("failed to read config: {}", e);
        std::process::exit(1);
    });
    debug!("config is parsed");

    // register dbus in daemon mode
    // TODO: parse with clap
    register_dbus(&application_id, application_name, &cfg).unwrap_or_else(|e| {
        error!("failed to register dbus: {}", e);
        std::process::exit(1);
    });

    start_ui(&application_id, application_name, &cfg)
        .map_err(|e| {
            error!("ui failed: {}", e);
            std::process::exit(1);
        })
        .unwrap();
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
