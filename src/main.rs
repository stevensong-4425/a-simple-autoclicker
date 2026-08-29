#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod app;
mod backend;
mod clicker;
#[cfg(target_os = "linux")]
mod hotkey;
mod icon;
mod model;
mod presets;
#[cfg(target_os = "linux")]
mod tray;
#[cfg(target_os = "windows")]
mod windows_app;

#[cfg(target_os = "linux")]
use adw::prelude::*;

#[cfg(target_os = "linux")]
const APP_ID: &str = "com.asimpleautoclicker.App";

#[cfg(target_os = "linux")]
fn main() {
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_activate(app::build_ui);
    application.run();
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = windows_app::run() {
        windows_app::show_fatal_error(&error);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!("A Simple Autoclicker currently supports Linux and Windows.");
}
