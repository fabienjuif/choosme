mod application;
mod cli;
mod config;
mod daemon;
mod dbus;
mod realm;
mod runner;
mod ui;

use anyhow::{Context, Result, format_err};
use config::DEFAULT_CONFIG_ID;
use daemon::register_dbus;
use gtk4::gio::prelude::ApplicationExtManual;
use gtk4::glib::ExitCode;
use runner::start_applications_opener;
use std::env;
use std::path::PathBuf;
use std::sync::mpsc;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, warn};
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
    let application_id = format!("juif.fabien.{application_name}.client");

    // we keep the guard around for the duration of the application
    // to ensure that all logs are flushed before the application exits.
    let _guard = init_logging(application_name).map_err(|e| format_err!("on init_logging(): {e}"));

    if let Err(e) = run(&application_id, application_name) {
        error!("{e}");
        std::process::exit(1);
    }
}

fn run(application_id: &str, application_name: &str) -> Result<()> {
    let realms = realm::Realm::load_all()?;

    // parsing arguments
    let mut daemon_mode = false;
    let cli = cli::parse();
    let realm_id = cli.id.unwrap_or_else(|| DEFAULT_CONFIG_ID.to_string());
    match cli.command {
        Some(cli::Commands::Daemon {
            set_default,
            unset_default,
            status,
            kill,
            set_default_next,
            waybar,
        }) => {
            if !status
                && !unset_default
                && set_default.is_none()
                && !kill
                && !set_default_next
                && !waybar
            {
                daemon_mode = true;
            } else {
                // in all other cases we need a dbus client
                let dbus_client = dbus::DBUSClient::new()
                    .map_err(|e| format_err!("on DBUSClient::new(): {e}"))?;

                if status {
                    let output = dbus_client
                        .status(&realm_id)
                        .map_err(|e| format_err!("on dbus_client.status(): {e}"))?;
                    serde_json::to_writer(std::io::stdout(), &output)
                        .expect("failed to write status command output");
                    return Ok(());
                } else if kill {
                    let _ = dbus_client
                        .kill()
                        .map_err(|e| format_err!("on dbus_client.kill(): {e}"))?;
                    return Ok(());
                } else if let Some(index) = set_default {
                    let _ = dbus_client
                        .set_default(&realm_id, index as i64)
                        .map_err(|e| format_err!("on dbus_client.set_default(): {e}"))?;
                    return Ok(());
                } else if unset_default {
                    let _ = dbus_client
                        .set_default(&realm_id, -1)
                        .map_err(|e| format_err!("on dbus_client.set_default(-1): {e}"))?;
                    return Ok(());
                } else if set_default_next {
                    // getting status
                    let status = dbus_client
                        .status(&realm_id)
                        .map_err(|e| format_err!("on dbus_client.status(): {e}"))?;
                    // getting default app index if set otherwise -1 (to do 1 after the increment)
                    let default_index = status
                        .applications
                        .iter()
                        .enumerate()
                        .find(|(_, app)| app.is_default)
                        .map(|(index, _)| index as i64)
                        .unwrap_or(-1);
                    let mut next_index = default_index + 1;
                    if next_index >= status.applications.len() as i64 {
                        next_index = -1; // if we are at the end, we unset the default
                    }
                    // setting the next default app index
                    let _ = dbus_client
                        .set_default(&realm_id, next_index)
                        .map_err(|e| format_err!("on dbus_client.set_default_next(): {e}"))?;
                    return Ok(());
                } else if waybar {
                    let status = dbus_client
                        .status(&realm_id)
                        .map_err(|e| format_err!("on dbus_client.status(): {e}"))?;
                    // {"text": "$text", "alt": "$alt", "tooltip": "$tooltip", "class": "$class", "percentage": $percentage }
                    #[derive(serde::Serialize)]
                    struct WaybarOutput {
                        text: String,
                        class: String,
                        alt: String,
                    }
                    let default_application = status.applications.iter().find(|app| app.is_default);
                    let application_name = default_application
                        .map_or_else(|| "Select".to_string(), |app| app.name.clone());
                    let css_class = default_application
                        .map_or_else(|| "no-default".to_string(), |app| app.name.clone())
                        .to_lowercase();
                    let waybar_output = WaybarOutput {
                        text: application_name.clone(),
                        alt: css_class.clone(),
                        class: format!("choosme-{}", remove_whitespace(&css_class)),
                    };
                    serde_json::to_writer(std::io::stdout(), &waybar_output)
                        .expect("failed to write waybar command output");
                    return Ok(());
                }

                // TODO:
                return Err(format_err!(
                    "NOT READY YET set_default: {:?}, unset_default: {}",
                    set_default,
                    unset_default
                ));
            }
        }
        None => {
            // run the UI
            warn!(
                "no command provided, running in client mode: uri={:?}",
                cli.uri
            );
        }
    }

    // if no daemon mode, we try to connect to it
    // and if we fail we fallback with local resolution (and eventually start the UI onf fallback)
    if !daemon_mode {
        if let Ok(dbus_client) = dbus::DBUSClient::new() {
            debug!("connected to dbus in client mode");
            match dbus_client.open(&realm_id, &cli.uri.clone().unwrap_or_default()) {
                Ok(outputs) => {
                    info!("open command executed successfully: {:?}", outputs);
                    std::process::exit(0);
                }
                Err(e) => {
                    // we are not exiting here, we will fallback to standalone mode
                    error!(
                        "failed to execute open command: {}, fallbacking to standalone mode",
                        e
                    );
                }
            }
        } else {
            warn!("failed to create dbus client, using standalone mode");
        }
    }

    // if we are here, it means we are either in daemon mode or we unsucessfully tried to connect to dbus
    let (jh_applications_opener, applications_opener_tx) = start_applications_opener();

    // if we have an uri maybe we can open it?
    let realm = realms
        .get(&realm_id)
        .context(format_err!("no realm found with id: {}", realm_id))?;
    let resolved = if let Some(uri) = &cli.uri {
        let mut found = false;
        for desktop_file in &realm.applications {
            if desktop_file.match_uri(uri) {
                debug!("found matching desktop file: {}", desktop_file.id);
                // we have a matching desktop file, we can open the url
                if let Err(e) = applications_opener_tx.send(runner::ApplicationOpenerCommand::Open(
                    runner::OpenParams {
                        uris: vec![uri.clone()],
                        application_id: desktop_file.id.clone(),
                        realm_id: realm_id.clone(),
                    },
                )) {
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
    let (ui_tx, ui_rx) = async_channel::bounded::<String>(1);

    // register dbus in daemon mode
    let applications_opener_clone = applications_opener_tx.clone();
    let jh_dbus = if daemon_mode && !resolved {
        Some(
            register_dbus(
                application_name,
                realms.clone(),
                applications_opener_clone,
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
        let applications_opener_clone = applications_opener_tx.clone();
        let ui_application = start_ui(
            application_id,
            application_name,
            realm,
            applications_opener_clone,
            ui_rx,
            daemon_mode,
            cli.uri,
        );

        info!("running application: {}", application_id);
        let exit_code = ui_application.run_with_args::<String>(&[]);
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
    applications_opener_tx
        .send(runner::ApplicationOpenerCommand::Quit)
        .unwrap_or_else(|e| {
            error!("failed to send quit command to desktop file opener: {}", e);
        });
    jh_applications_opener.join().unwrap_or_else(|e| {
        error!("desktop file opener thread failed: {:?}", e);
    });
    info!("desktop file opener thread closed!");

    Ok(())
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

fn remove_whitespace(input: &str) -> String {
    input.chars().filter(|c| !c.is_whitespace()).collect()
}
