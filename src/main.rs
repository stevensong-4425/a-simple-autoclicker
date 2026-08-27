mod app;
mod backend;
mod clicker;
mod hotkey;
mod model;
mod presets;
mod tray;

use adw::prelude::*;

const APP_ID: &str = "com.asimpleautoclicker.App";

fn main() {
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_activate(app::build_ui);
    application.run();
}
