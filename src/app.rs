use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{mpsc, Arc},
    time::Duration,
};

use adw::prelude::*;
use gtk::glib::translate::IntoGlib;
use gtk::{Align, StringList};

use crate::{
    backend,
    clicker::ClickEngine,
    hotkey::HotkeyManager,
    model::{Action, ClickPosition, Hotkey, KeyModifiers},
    presets::{Preset, PresetStore},
    tray::{self, TrayCommand},
};

pub fn build_ui(application: &adw::Application) {
    // GTK forwards a second launch to this application's activation handler.
    // Reuse and reveal the existing window instead of constructing another one.
    if let Some(window) = application.windows().into_iter().next() {
        window.present();
        return;
    }

    let engine = ClickEngine::start();
    let hotkeys = Rc::new(HotkeyManager::start(Arc::clone(&engine), Hotkey::F8));
    let (tray_sender, tray_receiver) = mpsc::channel();
    let tray_handle = tray::start(Arc::clone(&engine), tray_sender);
    let tray_mode = Rc::new(Cell::new(true));
    let preset_store = Rc::new(RefCell::new(PresetStore::load()));
    let recording = Rc::new(Cell::new(false));
    let selected_hotkey = Rc::new(Cell::new(Hotkey::F8));
    let captured_position = Rc::new(Cell::new(None::<ClickPosition>));

    let left_action = gtk::ToggleButton::builder()
        .label("Left")
        .active(true)
        .build();
    let middle_action = gtk::ToggleButton::builder().label("Middle").build();
    middle_action.set_group(Some(&left_action));
    let right_action = gtk::ToggleButton::builder().label("Right").build();
    right_action.set_group(Some(&left_action));
    let mouse_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mouse_actions.add_css_class("linked");
    mouse_actions.append(&left_action);
    mouse_actions.append(&middle_action);
    mouse_actions.append(&right_action);

    let record_key_button = gtk::Button::builder()
        .label("Record a key…")
        .valign(Align::Center)
        .build();

    let fixed_position_switch = gtk::Switch::builder().valign(Align::Center).build();
    let capture_position_button = gtk::Button::builder()
        .label("Capture position…")
        .valign(Align::Center)
        .build();

    let interval = gtk::SpinButton::with_range(10.0, 60_000.0, 10.0);
    interval.set_value(100.0);
    interval.set_valign(Align::Center);
    let interval_control = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    interval_control.append(&interval);
    interval_control.append(&gtk::Label::new(Some("ms")));

    let timed_run_switch = gtk::Switch::builder().valign(Align::Center).build();
    let duration_value = gtk::SpinButton::with_range(1.0, 9_999.0, 1.0);
    duration_value.set_value(10.0);
    let duration_unit_model = StringList::new(&["Seconds", "Minutes", "Hours"]);
    let duration_unit = gtk::DropDown::builder()
        .model(&duration_unit_model)
        .selected(1)
        .build();
    let duration_control = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    duration_control.set_sensitive(false);
    duration_control.append(&duration_value);
    duration_control.append(&duration_unit);

    let count_limit_switch = gtk::Switch::builder().valign(Align::Center).build();
    let count_limit = gtk::SpinButton::with_range(1.0, 10_000_000.0, 1.0);
    count_limit.set_value(100.0);
    count_limit.set_sensitive(false);

    let tray_mode_switch = gtk::Switch::builder()
        .active(true)
        .valign(Align::Center)
        .build();

    let hotkey_model = StringList::new(&Hotkey::LABELS);
    let hotkey_dropdown = gtk::DropDown::builder()
        .model(&hotkey_model)
        .selected(2)
        .valign(Align::Center)
        .build();

    let action_row = adw::ActionRow::builder()
        .title("Repeated action")
        .subtitle("Left mouse click")
        .build();
    action_row.add_suffix(&mouse_actions);

    let record_key_row = adw::ActionRow::builder()
        .title("Keyboard action")
        .subtitle("Record any keyboard key or key combination")
        .build();
    record_key_row.add_suffix(&record_key_button);

    let fixed_position_row = adw::ActionRow::builder()
        .title("Fixed mouse position")
        .subtitle("Off — mouse clicks use the current pointer position")
        .build();
    let fixed_position_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    fixed_position_controls.append(&capture_position_button);
    fixed_position_controls.append(&fixed_position_switch);
    fixed_position_row.add_suffix(&fixed_position_controls);

    let interval_row = adw::ActionRow::builder()
        .title("Interval")
        .subtitle("Delay between repeated actions")
        .build();
    interval_row.add_suffix(&interval_control);

    let timed_run_row = adw::ActionRow::builder()
        .title("Stop automatically")
        .subtitle("End the run after a set amount of time")
        .build();
    timed_run_row.add_suffix(&timed_run_switch);

    let duration_row = adw::ActionRow::builder()
        .title("Run duration")
        .subtitle("The timer starts when clicking begins")
        .build();
    duration_row.add_suffix(&duration_control);

    let count_limit_row = adw::ActionRow::builder()
        .title("Stop after actions")
        .subtitle("End after an exact number of clicks or key presses")
        .build();
    let count_limit_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    count_limit_controls.append(&count_limit);
    count_limit_controls.append(&count_limit_switch);
    count_limit_row.add_suffix(&count_limit_controls);

    let hotkey_row = adw::ActionRow::builder()
        .title("Global toggle hotkey")
        .subtitle("Works while this window is in the background")
        .build();
    hotkey_row.add_suffix(&hotkey_dropdown);

    let tray_mode_row = adw::ActionRow::builder()
        .title("Keep in system tray")
        .subtitle("Closing the window hides it; use the tray icon to reopen or quit")
        .build();
    tray_mode_row.add_suffix(&tray_mode_switch);

    let settings_group = adw::PreferencesGroup::builder()
        .title("Click settings")
        .build();
    settings_group.add(&action_row);
    settings_group.add(&record_key_row);
    settings_group.add(&fixed_position_row);
    settings_group.add(&interval_row);
    settings_group.add(&timed_run_row);
    settings_group.add(&duration_row);
    settings_group.add(&count_limit_row);
    settings_group.add(&hotkey_row);
    settings_group.add(&tray_mode_row);

    let preset_names = preset_store.borrow().names().join("\n");
    let preset_name_refs: Vec<&str> = if preset_names.is_empty() {
        Vec::new()
    } else {
        preset_names.lines().collect()
    };
    let preset_model = StringList::new(&preset_name_refs);
    let preset_dropdown = gtk::DropDown::builder()
        .model(&preset_model)
        .hexpand(true)
        .build();
    let preset_name = gtk::Entry::builder()
        .placeholder_text("Preset name")
        .hexpand(true)
        .build();
    let load_preset_button = gtk::Button::with_label("Load");
    let save_preset_button = gtk::Button::with_label("Save current");
    let preset_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    preset_controls.append(&preset_dropdown);
    preset_controls.append(&load_preset_button);
    let preset_save_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    preset_save_controls.append(&preset_name);
    preset_save_controls.append(&save_preset_button);
    let preset_load_row = adw::ActionRow::builder()
        .title("Saved preset")
        .subtitle("Load a previously saved configuration")
        .build();
    preset_load_row.add_suffix(&preset_controls);
    let preset_save_row = adw::ActionRow::builder()
        .title("Save configuration")
        .subtitle("Presets are stored in your user configuration folder")
        .build();
    preset_save_row.add_suffix(&preset_save_controls);
    let preset_group = adw::PreferencesGroup::builder().title("Presets").build();
    preset_group.add(&preset_load_row);
    preset_group.add(&preset_save_row);

    let start_button = gtk::Button::builder()
        .label("Start clicking")
        .halign(Align::Fill)
        .height_request(48)
        .css_classes(["suggested-action", "pill"])
        .build();

    let stop_button = gtk::Button::builder()
        .label("Stop")
        .valign(Align::Center)
        .css_classes(["destructive-action"])
        .visible(false)
        .build();

    let status_row = adw::ActionRow::builder()
        .title("Ready")
        .subtitle("Press F8 or use the button below to start")
        .build();
    status_row.add_suffix(&stop_button);
    let status_group = adw::PreferencesGroup::builder().title("Status").build();
    status_group.add(&status_row);
    status_group.add(&start_button);

    let page = adw::PreferencesPage::new();
    page.add(&settings_group);
    page.add(&preset_group);
    page.add(&status_group);

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));

    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("A Simple Autoclicker")
        .default_width(760)
        .default_height(1040)
        .content(&toolbar)
        .build();

    {
        let engine = Arc::clone(&engine);
        let clicked_start_button = start_button.clone();
        let clicked_stop_button = stop_button.clone();
        start_button.connect_clicked(move |_| {
            // Remove the control beneath the pointer before the worker emits its
            // first synthetic click. Otherwise that click toggles us off again.
            clicked_start_button.set_visible(false);
            clicked_stop_button.set_visible(true);
            engine.set_active(true);
        });
    }
    {
        let engine = Arc::clone(&engine);
        stop_button.connect_clicked(move |_| engine.set_active(false));
    }
    {
        let engine = Arc::clone(&engine);
        let action_row = action_row.clone();
        let recording = Rc::clone(&recording);
        let hotkeys = Rc::clone(&hotkeys);
        let record_key_button = record_key_button.clone();
        left_action.connect_toggled(move |button| {
            if button.is_active() {
                recording.set(false);
                hotkeys.set_enabled(true);
                record_key_button.set_label("Record a key…");
                engine.set_action(Action::LeftClick);
                action_row.set_subtitle("Left mouse click");
            }
        });
    }
    {
        let engine = Arc::clone(&engine);
        let action_row = action_row.clone();
        let recording = Rc::clone(&recording);
        let hotkeys = Rc::clone(&hotkeys);
        let record_key_button = record_key_button.clone();
        middle_action.connect_toggled(move |button| {
            if button.is_active() {
                recording.set(false);
                hotkeys.set_enabled(true);
                record_key_button.set_label("Record a key…");
                engine.set_action(Action::MiddleClick);
                action_row.set_subtitle("Middle mouse click");
            }
        });
    }
    {
        let engine = Arc::clone(&engine);
        let action_row = action_row.clone();
        let recording = Rc::clone(&recording);
        let hotkeys = Rc::clone(&hotkeys);
        let record_key_button = record_key_button.clone();
        right_action.connect_toggled(move |button| {
            if button.is_active() {
                recording.set(false);
                hotkeys.set_enabled(true);
                record_key_button.set_label("Record a key…");
                engine.set_action(Action::RightClick);
                action_row.set_subtitle("Right mouse click");
            }
        });
    }
    {
        let engine = Arc::clone(&engine);
        let recording = Rc::clone(&recording);
        let hotkeys = Rc::clone(&hotkeys);
        let record_key_row = record_key_row.clone();
        record_key_button.connect_clicked(move |button| {
            engine.set_active(false);
            recording.set(true);
            hotkeys.set_enabled(false);
            record_key_row.set_subtitle("Press the key or key combination to repeat now");
            button.set_label("Listening…");
        });
    }
    {
        let engine = Arc::clone(&engine);
        let captured_position = Rc::clone(&captured_position);
        let fixed_position_row = fixed_position_row.clone();
        fixed_position_switch.connect_active_notify(move |switch| {
            if switch.is_active() {
                if let Some(position) = captured_position.get() {
                    engine.set_position(Some(position));
                    fixed_position_row.set_subtitle(&format!(
                        "Clicks will be sent to ({}, {})",
                        position.x, position.y
                    ));
                } else {
                    fixed_position_row.set_subtitle("Capture a position before enabling this mode");
                    switch.set_active(false);
                }
            } else {
                engine.set_position(None);
                if captured_position.get().is_none() {
                    fixed_position_row
                        .set_subtitle("Off — mouse clicks use the current pointer position");
                }
            }
        });
    }
    {
        let engine = Arc::clone(&engine);
        let captured_position = Rc::clone(&captured_position);
        let fixed_position_switch = fixed_position_switch.clone();
        let fixed_position_row = fixed_position_row.clone();
        capture_position_button.connect_clicked(move |button| {
            engine.set_active(false);
            button.set_sensitive(false);
            button.set_label("Move pointer — capturing in 2s");
            let engine = Arc::clone(&engine);
            let captured_position = Rc::clone(&captured_position);
            let fixed_position_switch = fixed_position_switch.clone();
            let fixed_position_row = fixed_position_row.clone();
            let button = button.clone();
            gtk::glib::timeout_add_local_once(Duration::from_secs(2), move || {
                match backend::pointer_position() {
                    Ok(position) => {
                        captured_position.set(Some(position));
                        engine.set_position(Some(position));
                        fixed_position_switch.set_active(true);
                        fixed_position_row.set_subtitle(&format!(
                            "Clicks will be sent to ({}, {})",
                            position.x, position.y
                        ));
                    }
                    Err(error) => fixed_position_row.set_subtitle(&error),
                }
                button.set_label("Capture position…");
                button.set_sensitive(true);
            });
        });
    }
    {
        let engine = Arc::clone(&engine);
        interval.connect_value_changed(move |spin| engine.set_interval_ms(spin.value() as u64));
    }
    {
        let engine = Arc::clone(&engine);
        let duration_control = duration_control.clone();
        let duration_value = duration_value.clone();
        let duration_unit = duration_unit.clone();
        timed_run_switch.connect_active_notify(move |switch| {
            duration_control.set_sensitive(switch.is_active());
            engine.set_duration_ms(duration_from_controls(
                switch.is_active(),
                duration_value.value(),
                duration_unit.selected(),
            ));
        });
    }
    {
        let engine = Arc::clone(&engine);
        let count_limit = count_limit.clone();
        count_limit_switch.connect_active_notify(move |switch| {
            count_limit.set_sensitive(switch.is_active());
            engine.set_max_actions(if switch.is_active() {
                count_limit.value() as u64
            } else {
                0
            });
        });
    }
    {
        let engine = Arc::clone(&engine);
        let count_limit_switch = count_limit_switch.clone();
        count_limit.connect_value_changed(move |spin| {
            if count_limit_switch.is_active() {
                engine.set_max_actions(spin.value() as u64);
            }
        });
    }
    {
        let tray_mode = Rc::clone(&tray_mode);
        tray_mode_switch.connect_active_notify(move |switch| {
            tray_mode.set(switch.is_active());
        });
    }
    {
        let engine = Arc::clone(&engine);
        let preset_store = Rc::clone(&preset_store);
        let selected_hotkey = Rc::clone(&selected_hotkey);
        let preset_model = preset_model.clone();
        let preset_dropdown = preset_dropdown.clone();
        let preset_name = preset_name.clone();
        let preset_save_row = preset_save_row.clone();
        let action_row = action_row.clone();
        save_preset_button.connect_clicked(move |_| {
            let name = preset_name.text().trim().to_string();
            if name.is_empty() {
                preset_save_row.set_subtitle("Enter a name before saving");
                return;
            }
            let preset = Preset {
                name: name.clone(),
                action: engine.action(),
                action_label: action_row
                    .subtitle()
                    .map(|label| label.to_string())
                    .unwrap_or_else(|| "Left mouse click".into()),
                interval_ms: engine.interval_ms(),
                duration_ms: engine.duration_ms(),
                max_actions: engine.max_actions(),
                position: engine.position(),
                hotkey: selected_hotkey.get(),
            };
            let mut store = preset_store.borrow_mut();
            match store.save(preset) {
                Ok(()) => {
                    refresh_preset_model(&preset_model, &store);
                    if let Some(index) = store.names().iter().position(|saved| *saved == name) {
                        preset_dropdown.set_selected(index as u32);
                    }
                    preset_save_row.set_subtitle(&format!("Saved preset: {name}"));
                }
                Err(error) => preset_save_row.set_subtitle(&format!("Could not save: {error}")),
            }
        });
    }
    {
        let engine = Arc::clone(&engine);
        let preset_store = Rc::clone(&preset_store);
        let preset_dropdown = preset_dropdown.clone();
        let preset_name = preset_name.clone();
        let preset_load_row = preset_load_row.clone();
        let action_row = action_row.clone();
        let left_action = left_action.clone();
        let middle_action = middle_action.clone();
        let right_action = right_action.clone();
        let interval = interval.clone();
        let timed_run_switch = timed_run_switch.clone();
        let duration_value = duration_value.clone();
        let duration_unit = duration_unit.clone();
        let count_limit_switch = count_limit_switch.clone();
        let count_limit = count_limit.clone();
        let captured_position = Rc::clone(&captured_position);
        let fixed_position_switch = fixed_position_switch.clone();
        let fixed_position_row = fixed_position_row.clone();
        let hotkey_dropdown = hotkey_dropdown.clone();
        load_preset_button.connect_clicked(move |_| {
            let Some(preset) = preset_store
                .borrow()
                .get(preset_dropdown.selected() as usize)
            else {
                preset_load_row.set_subtitle("Choose a saved preset first");
                return;
            };

            engine.set_active(false);
            match preset.action {
                Action::LeftClick => left_action.set_active(true),
                Action::MiddleClick => middle_action.set_active(true),
                Action::RightClick => right_action.set_active(true),
                Action::Key { .. } => {
                    left_action.set_active(false);
                    middle_action.set_active(false);
                    right_action.set_active(false);
                }
            }
            engine.set_action(preset.action);
            action_row.set_subtitle(&preset.action_label);
            interval.set_value(preset.interval_ms as f64);
            set_duration_widgets(
                preset.duration_ms,
                &timed_run_switch,
                &duration_value,
                &duration_unit,
            );
            engine.set_duration_ms(preset.duration_ms);
            count_limit.set_value(preset.max_actions.max(1) as f64);
            count_limit_switch.set_active(preset.max_actions > 0);
            engine.set_max_actions(preset.max_actions);

            fixed_position_switch.set_active(false);
            captured_position.set(preset.position);
            engine.set_position(preset.position);
            fixed_position_switch.set_active(preset.position.is_some());
            if let Some(position) = preset.position {
                fixed_position_row.set_subtitle(&format!(
                    "Clicks will be sent to ({}, {})",
                    position.x, position.y
                ));
            }

            if let Some(index) = Hotkey::ALL
                .iter()
                .position(|hotkey| *hotkey == preset.hotkey)
            {
                hotkey_dropdown.set_selected(index as u32);
            }
            preset_name.set_text(&preset.name);
            preset_load_row.set_subtitle(&format!("Loaded preset: {}", preset.name));
        });
    }
    {
        let engine = Arc::clone(&engine);
        let timed_run_switch = timed_run_switch.clone();
        let duration_unit = duration_unit.clone();
        duration_value.connect_value_changed(move |spin| {
            engine.set_duration_ms(duration_from_controls(
                timed_run_switch.is_active(),
                spin.value(),
                duration_unit.selected(),
            ));
        });
    }
    {
        let engine = Arc::clone(&engine);
        let timed_run_switch = timed_run_switch.clone();
        let duration_value = duration_value.clone();
        duration_unit.connect_selected_notify(move |dropdown| {
            engine.set_duration_ms(duration_from_controls(
                timed_run_switch.is_active(),
                duration_value.value(),
                dropdown.selected(),
            ));
        });
    }
    {
        let hotkeys = Rc::clone(&hotkeys);
        let selected_hotkey = Rc::clone(&selected_hotkey);
        hotkey_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(hotkey) = Hotkey::ALL.get(dropdown.selected() as usize) {
                selected_hotkey.set(*hotkey);
                hotkeys.set_hotkey(*hotkey);
            }
        });
    }

    let key_controller = gtk::EventControllerKey::new();
    {
        let engine = Arc::clone(&engine);
        let recording = Rc::clone(&recording);
        let hotkeys = Rc::clone(&hotkeys);
        let selected_hotkey = Rc::clone(&selected_hotkey);
        let action_row = action_row.clone();
        let record_key_row = record_key_row.clone();
        let record_key_button = record_key_button.clone();
        key_controller.connect_key_pressed(move |_, keyval, _, state| {
            if !recording.get() {
                return gtk::glib::Propagation::Proceed;
            }

            let keysym = keyval.into_glib() as u64;
            let key_name = keyval
                .name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| format!("keysym {keysym:#x}"));

            // A modifier may either be the requested key itself or the beginning
            // of a combination. Wait for release to distinguish those cases.
            if is_modifier_keysym(keysym) {
                return gtk::glib::Propagation::Stop;
            }

            recording.set(false);
            hotkeys.set_enabled(true);
            record_key_button.set_label("Record another key…");

            if keysym == selected_hotkey.get().keysym() {
                record_key_row.set_subtitle(
                    "That key controls the global toggle; choose another toggle key first",
                );
                return gtk::glib::Propagation::Stop;
            }

            let modifiers = KeyModifiers {
                shift: state.contains(gtk::gdk::ModifierType::SHIFT_MASK),
                control: state.contains(gtk::gdk::ModifierType::CONTROL_MASK),
                alt: state.contains(gtk::gdk::ModifierType::ALT_MASK),
                super_key: state.contains(gtk::gdk::ModifierType::SUPER_MASK),
            };
            let label = recorded_key_label(&key_name, modifiers);
            engine.set_action(Action::Key { keysym, modifiers });
            action_row.set_subtitle(&format!("{label} key"));
            record_key_row.set_subtitle(&format!("Recorded: {label}"));
            gtk::glib::Propagation::Stop
        });
    }
    {
        let engine = Arc::clone(&engine);
        let recording = Rc::clone(&recording);
        let hotkeys = Rc::clone(&hotkeys);
        let action_row = action_row.clone();
        let record_key_row = record_key_row.clone();
        let record_key_button = record_key_button.clone();
        key_controller.connect_key_released(move |_, keyval, _, _| {
            if !recording.get() {
                return;
            }

            let keysym = keyval.into_glib() as u64;
            if !is_modifier_keysym(keysym) {
                return;
            }

            recording.set(false);
            hotkeys.set_enabled(true);
            record_key_button.set_label("Record another key…");
            let key_name = keyval
                .name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| format!("keysym {keysym:#x}"));
            engine.set_action(Action::Key {
                keysym,
                modifiers: KeyModifiers::default(),
            });
            action_row.set_subtitle(&format!("{key_name} key"));
            record_key_row.set_subtitle(&format!("Recorded: {key_name}"));
        });
    }
    window.add_controller(key_controller);

    {
        let tray_mode = Rc::clone(&tray_mode);
        window.connect_close_request(move |window| {
            if tray_mode.get() {
                window.set_visible(false);
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
    }

    let previous_active = Rc::new(Cell::new(false));
    let hotkeys_keepalive = Rc::clone(&hotkeys);
    let tray_window = window.clone();
    let tray_application = application.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
        // Capturing this handle keeps the global hotkey thread configurable while
        // the window is alive.
        let _ = &hotkeys_keepalive;
        while let Ok(command) = tray_receiver.try_recv() {
            match command {
                TrayCommand::ShowWindow => tray_window.present(),
                TrayCommand::Quit => tray_application.quit(),
            }
        }
        let active = engine.is_active();
        if active != previous_active.get() {
            previous_active.set(active);
            tray_handle.refresh();
            if active {
                status_row.set_title("Clicking");
                start_button.set_visible(false);
                stop_button.set_visible(true);
            } else {
                if engine.take_completed_run() {
                    status_row.set_title("Finished");
                    status_row.set_subtitle("The selected run limit has been reached");
                } else {
                    status_row.set_title("Ready");
                    status_row.set_subtitle("Use the global hotkey or button to start");
                }
                start_button.set_visible(true);
                stop_button.set_visible(false);
            }
        }

        if active {
            let mut progress = Vec::new();
            if let Some(remaining_ms) = engine.remaining_ms() {
                progress.push(format!("{} left", format_remaining_time(remaining_ms)));
            }
            if let Some(remaining_actions) = engine.remaining_actions() {
                progress.push(format!("{remaining_actions} actions left"));
            }
            if progress.is_empty() {
                status_row.set_subtitle("Use the global hotkey or Stop to finish");
            } else {
                status_row.set_subtitle(&format!(
                    "{} • use the global hotkey or Stop to finish",
                    progress.join(" • ")
                ));
            }
        }

        if let Some(error) = engine.backend_error() {
            status_row.set_title("Input backend unavailable");
            status_row.set_subtitle(&error);
            start_button.set_sensitive(false);
            stop_button.set_sensitive(false);
        }
        gtk::glib::ControlFlow::Continue
    });

    window.present();
}

fn recorded_key_label(key_name: &str, modifiers: KeyModifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.control {
        parts.push("Ctrl".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    if modifiers.super_key {
        parts.push("Super".to_string());
    }
    parts.push(key_name.to_string());
    parts.join("+")
}

fn is_modifier_keysym(keysym: u64) -> bool {
    matches!(
        keysym,
        0xffe1
            ..=0xffee // Shift, Control, Caps, Meta, Alt, Super, and Hyper
            | 0xfe03 // ISO_Level3_Shift (commonly AltGr)
    )
}

fn duration_from_controls(enabled: bool, value: f64, unit: u32) -> u64 {
    if !enabled {
        return 0;
    }
    let seconds_per_unit = match unit {
        0 => 1,
        1 => 60,
        2 => 60 * 60,
        _ => 1,
    };
    (value as u64)
        .saturating_mul(seconds_per_unit)
        .saturating_mul(1_000)
}

fn format_remaining_time(remaining_ms: u64) -> String {
    let total_seconds = remaining_ms.saturating_add(999) / 1_000;
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

fn refresh_preset_model(model: &StringList, store: &PresetStore) {
    let names = store.names();
    model.splice(0, model.n_items(), &names);
}

fn set_duration_widgets(
    duration_ms: u64,
    enabled: &gtk::Switch,
    value: &gtk::SpinButton,
    unit: &gtk::DropDown,
) {
    if duration_ms == 0 {
        enabled.set_active(false);
        return;
    }
    let total_seconds = duration_ms / 1_000;
    let (display_value, selected_unit) = if total_seconds % 3_600 == 0 {
        (total_seconds / 3_600, 2)
    } else if total_seconds % 60 == 0 {
        (total_seconds / 60, 1)
    } else {
        (total_seconds.max(1), 0)
    };
    value.set_value(display_value as f64);
    unit.set_selected(selected_unit);
    enabled.set_active(true);
}

#[cfg(test)]
mod tests {
    use super::{duration_from_controls, format_remaining_time};

    #[test]
    fn converts_minutes_to_milliseconds() {
        assert_eq!(duration_from_controls(true, 10.0, 1), 600_000);
        assert_eq!(duration_from_controls(false, 10.0, 1), 0);
    }

    #[test]
    fn formats_remaining_time() {
        assert_eq!(format_remaining_time(600_000), "10m 00s");
        assert_eq!(format_remaining_time(3_661_000), "1h 01m 01s");
    }
}
