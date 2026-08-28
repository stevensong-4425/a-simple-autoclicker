use std::{
    mem::zeroed,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Frame, Layout, Margin, RichText, Rounding, Stroke,
    Vec2,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use windows_sys::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
    },
    WindowsAndMessaging::{
        FindWindowW, MessageBoxW, PeekMessageW, PostQuitMessage, SetForegroundWindow, ShowWindow,
        MB_ICONERROR, MB_OK, MSG, PM_REMOVE, SW_RESTORE, WM_HOTKEY,
    },
};

use crate::{
    backend,
    clicker::ClickEngine,
    model::{Action, Hotkey, KeyModifiers},
    presets::{Preset, PresetStore},
};

const APP_TITLE: &str = "A Simple Autoclicker";
const BLUE: Color32 = Color32::from_rgb(28, 126, 224);
const TEXT: Color32 = Color32::from_rgb(48, 48, 48);
const MUTED: Color32 = Color32::from_rgb(145, 145, 145);
const TRAY_SHOW: u8 = 1;
const TRAY_TOGGLE: u8 = 1 << 1;
const TRAY_QUIT: u8 = 1 << 2;

pub fn run() -> Result<(), String> {
    let icon = app_icon(false);
    let viewport_icon = egui::IconData {
        rgba: icon.0.clone(),
        width: icon.1,
        height: icon.2,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 820.0])
            .with_min_inner_size([620.0, 560.0])
            .with_icon(viewport_icon),
        follow_system_theme: false,
        default_theme: eframe::Theme::Light,
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|creation_context| Box::new(WindowsApp::new(creation_context))),
    )
    .map_err(|error| error.to_string())
}

pub fn show_fatal_error(error: &str) {
    let title = to_wide(APP_TITLE);
    let message = to_wide(error);
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

struct WindowsApp {
    engine: Arc<ClickEngine>,
    hotkey_thread: WindowsHotkeyThread,
    hotkey_toggle: Arc<AtomicBool>,
    presets: PresetStore,
    action_index: usize,
    recorded_key: Option<(u32, KeyModifiers)>,
    recording: bool,
    interval_ms: u64,
    timed_run: bool,
    duration_value: u64,
    duration_unit: usize,
    count_limited: bool,
    max_actions: u64,
    fixed_position: bool,
    capture_at: Option<Instant>,
    hotkey_index: usize,
    tray_mode: bool,
    preset_index: Option<usize>,
    preset_name: String,
    status_message: Option<String>,
    tray_icon: Option<TrayIcon>,
    pending_tray_commands: Arc<AtomicU8>,
    tray_toggle: MenuItem,
    tray_active: bool,
    allow_close: bool,
    was_minimized: bool,
}

impl WindowsApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        configure_style(&context.egui_ctx);
        let engine = ClickEngine::start();
        let hotkey_toggle = Arc::new(AtomicBool::new(false));
        let hotkey_thread = WindowsHotkeyThread::start(Arc::clone(&hotkey_toggle), Hotkey::F8);

        let tray_show = MenuItem::new("Show window", true, None);
        let tray_toggle = MenuItem::new("Start clicking", true, None);
        let tray_quit = MenuItem::new("Quit", true, None);
        let separator = PredefinedMenuItem::separator();
        let tray_menu = Menu::with_items(&[&tray_show, &tray_toggle, &separator, &tray_quit]);
        let (idle_rgba, width, height) = app_icon(false);
        let tray_icon = tray_menu
            .ok()
            .and_then(|menu| {
                tray_icon::Icon::from_rgba(idle_rgba, width, height)
                    .ok()
                    .map(|icon| (menu, icon))
            })
            .and_then(|(menu, icon)| {
                TrayIconBuilder::new()
                    .with_tooltip(APP_TITLE)
                    .with_menu(Box::new(menu))
                    .with_menu_on_left_click(false)
                    .with_icon(icon)
                    .build()
                    .ok()
            });
        let pending_tray_commands = Arc::new(AtomicU8::new(0));
        install_tray_event_handlers(
            &context.egui_ctx,
            Arc::clone(&pending_tray_commands),
            &tray_show,
            &tray_toggle,
            &tray_quit,
        );
        let status_message = tray_icon
            .is_none()
            .then(|| "The system tray icon could not be created.".to_string());

        Self {
            engine,
            hotkey_thread,
            hotkey_toggle,
            presets: PresetStore::load(),
            action_index: 0,
            recorded_key: None,
            recording: false,
            interval_ms: 100,
            timed_run: false,
            duration_value: 10,
            duration_unit: 1,
            count_limited: false,
            max_actions: 100,
            fixed_position: false,
            capture_at: None,
            hotkey_index: 2,
            tray_mode: true,
            preset_index: None,
            preset_name: String::new(),
            status_message,
            tray_icon,
            pending_tray_commands,
            tray_toggle,
            tray_active: false,
            allow_close: false,
            was_minimized: false,
        }
    }

    fn process_window_state(&mut self, ctx: &egui::Context) {
        let (close_requested, minimized) = ctx.input(|input| {
            (
                input.viewport().close_requested(),
                input.viewport().minimized.unwrap_or(false),
            )
        });
        if close_requested && self.tray_mode && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.status_message = Some("Hidden in the system tray.".into());
        }
        if minimized && !self.was_minimized && self.tray_mode {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.status_message = Some("Minimized to the system tray.".into());
        }
        self.was_minimized = minimized;
    }

    fn process_tray_events(&mut self, ctx: &egui::Context) {
        let commands = self.pending_tray_commands.swap(0, Ordering::AcqRel);
        if commands & TRAY_QUIT != 0 {
            self.allow_close = true;
            self.engine.set_active(false);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if commands & TRAY_SHOW != 0 {
            self.show_window(ctx);
        }
        if commands & TRAY_TOGGLE != 0 {
            self.toggle_clicking();
        }
    }

    fn process_keyboard_recording(&mut self, ctx: &egui::Context) {
        if !self.recording {
            return;
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let Some(virtual_key) = key_to_virtual_key(key) else {
                self.status_message = Some("That key is not supported by Windows input.".into());
                continue;
            };
            let modifiers = KeyModifiers {
                shift: modifiers.shift || key_is_down(VK_SHIFT as i32),
                control: modifiers.ctrl || key_is_down(VK_CONTROL as i32),
                alt: modifiers.alt || key_is_down(VK_MENU as i32),
                super_key: key_is_down(VK_LWIN as i32),
            };
            let hotkey = Hotkey::ALL[self.hotkey_index];
            if virtual_key == hotkey.virtual_key() && modifiers == KeyModifiers::default() {
                self.status_message = Some(format!(
                    "{} controls start/stop. Choose another global hotkey first.",
                    Hotkey::LABELS[self.hotkey_index]
                ));
                self.recording = false;
                return;
            }
            self.recorded_key = Some((virtual_key, modifiers));
            self.action_index = 3;
            self.recording = false;
            self.status_message = Some(format!("Recorded {}.", key_label(virtual_key, modifiers)));
            return;
        }
    }

    fn process_capture(&mut self) {
        let Some(deadline) = self.capture_at else {
            return;
        };
        if Instant::now() < deadline {
            return;
        }
        self.capture_at = None;
        match backend::pointer_position() {
            Ok(position) => {
                self.engine.set_position(Some(position));
                self.fixed_position = true;
                self.status_message = Some(format!(
                    "Captured pointer position ({}, {}).",
                    position.x, position.y
                ));
            }
            Err(error) => self.status_message = Some(error),
        }
    }

    fn process_engine_state(&mut self) {
        if self.hotkey_toggle.swap(false, Ordering::AcqRel) {
            self.toggle_clicking();
        }
        let active = self.engine.is_active();
        if active != self.tray_active {
            self.tray_active = active;
            self.tray_toggle.set_text(if active {
                "Stop clicking"
            } else {
                "Start clicking"
            });
            if let Some(tray_icon) = &self.tray_icon {
                let (rgba, width, height) = app_icon(active);
                if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, width, height) {
                    let _ = tray_icon.set_icon(Some(icon));
                }
                let _ = tray_icon.set_tooltip(Some(if active {
                    "A Simple Autoclicker — Clicking"
                } else {
                    APP_TITLE
                }));
            }
            if !active && self.engine.take_completed_run() {
                self.status_message = Some("The selected run limit was reached.".into());
            }
        }
    }

    fn toggle_clicking(&mut self) {
        if self.engine.is_active() {
            self.engine.set_active(false);
            return;
        }
        match self.apply_settings() {
            Ok(()) => {
                self.status_message = None;
                self.engine.set_active(true);
            }
            Err(error) => self.status_message = Some(error),
        }
    }

    fn apply_settings(&self) -> Result<(), String> {
        if self.interval_ms < 10 {
            return Err("The interval must be at least 10 milliseconds.".into());
        }
        let action = self.selected_action()?;
        if self.fixed_position && self.engine.position().is_none() {
            return Err("Capture a pointer position before enabling fixed position.".into());
        }
        self.engine.set_action(action);
        self.engine.set_interval_ms(self.interval_ms);
        self.engine.set_duration_ms(if self.timed_run {
            duration_ms(self.duration_value, self.duration_unit)
        } else {
            0
        });
        self.engine.set_max_actions(if self.count_limited {
            self.max_actions.max(1)
        } else {
            0
        });
        if !self.fixed_position {
            self.engine.set_position(None);
        }
        Ok(())
    }

    fn selected_action(&self) -> Result<Action, String> {
        let action = match self.action_index {
            0 => Action::LeftClick,
            1 => Action::MiddleClick,
            2 => Action::RightClick,
            3 => {
                let (keysym, modifiers) = self
                    .recorded_key
                    .ok_or_else(|| "Record a keyboard key before starting.".to_string())?;
                Action::Key {
                    keysym: keysym as u64,
                    modifiers,
                }
            }
            _ => return Err("Choose a valid repeated action.".into()),
        };
        if let Action::Key { keysym, modifiers } = action {
            if keysym == Hotkey::ALL[self.hotkey_index].virtual_key() as u64
                && modifiers == KeyModifiers::default()
            {
                return Err("The repeated key and global toggle hotkey must be different.".into());
            }
        }
        Ok(action)
    }

    fn save_preset(&mut self) {
        let name = self.preset_name.trim().to_string();
        if name.is_empty() {
            self.status_message = Some("Enter a name before saving the preset.".into());
            return;
        }
        if let Err(error) = self.apply_settings() {
            self.status_message = Some(error);
            return;
        }
        let action = self.engine.action();
        let preset = Preset {
            name: name.clone(),
            action,
            action_label: action_label(action),
            interval_ms: self.engine.interval_ms(),
            duration_ms: self.engine.duration_ms(),
            max_actions: self.engine.max_actions(),
            position: if self.fixed_position {
                self.engine.position()
            } else {
                None
            },
            hotkey: Hotkey::ALL[self.hotkey_index],
        };
        match self.presets.save(preset) {
            Ok(()) => {
                self.preset_index = self.presets.names().iter().position(|saved| *saved == name);
                self.status_message = Some("Preset saved.".into());
            }
            Err(error) => self.status_message = Some(format!("Could not save preset: {error}")),
        }
    }

    fn load_preset(&mut self) {
        let Some(index) = self.preset_index else {
            self.status_message = Some("Choose a saved preset first.".into());
            return;
        };
        let Some(preset) = self.presets.get(index) else {
            return;
        };
        self.preset_name = preset.name.clone();
        self.interval_ms = preset.interval_ms;
        let (duration_value, duration_unit) = split_duration(preset.duration_ms);
        self.timed_run = preset.duration_ms > 0;
        self.duration_value = duration_value;
        self.duration_unit = duration_unit;
        self.count_limited = preset.max_actions > 0;
        self.max_actions = preset.max_actions.max(1);
        self.fixed_position = preset.position.is_some();
        self.engine.set_position(preset.position);
        self.hotkey_index = Hotkey::ALL
            .iter()
            .position(|hotkey| *hotkey == preset.hotkey)
            .unwrap_or(2);
        self.hotkey_thread.set(preset.hotkey);
        match preset.action {
            Action::LeftClick => self.action_index = 0,
            Action::MiddleClick => self.action_index = 1,
            Action::RightClick => self.action_index = 2,
            Action::Key { keysym, modifiers } => {
                self.action_index = 3;
                self.recorded_key = Some((keysym as u32, modifiers));
            }
        }
        self.status_message = Some(format!("Loaded preset “{}”.", preset.name));
    }

    fn show_window(&mut self, ctx: &egui::Context) {
        if matches!(
            self.status_message.as_deref(),
            Some("Hidden in the system tray." | "Minimized to the system tray.")
        ) {
            self.status_message = None;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn click_settings_ui(&mut self, ui: &mut egui::Ui) {
        group_heading(ui, "Click settings");
        card(ui, |ui| {
            settings_row(ui, "Repeated action", self.action_subtitle(), |ui| {
                ui.horizontal(|ui| {
                    for (index, label) in ["Left", "Middle", "Right"].iter().enumerate() {
                        if segment_button(ui, label, self.action_index == index).clicked() {
                            self.action_index = index;
                        }
                    }
                });
            });
            row_separator(ui);
            settings_row(
                ui,
                "Keyboard action",
                self.recorded_key
                    .map(|(key, modifiers)| format!("Recorded: {}", key_label(key, modifiers)))
                    .unwrap_or_else(|| "Record any keyboard key or key combination".into()),
                |ui| {
                    if ui
                        .add_sized(
                            [164.0, 34.0],
                            egui::Button::new(if self.recording {
                                "Press a key…"
                            } else {
                                "Record a key…"
                            }),
                        )
                        .clicked()
                    {
                        self.recording = true;
                        self.status_message =
                            Some("Press the key or key combination to repeat.".into());
                    }
                },
            );
            row_separator(ui);
            let position_text = self
                .engine
                .position()
                .map(|position| format!("Clicks will be sent to ({}, {})", position.x, position.y))
                .unwrap_or_else(|| "Off — mouse clicks use the current pointer position".into());
            settings_row(ui, "Fixed mouse position", position_text, |ui| {
                ui.horizontal(|ui| {
                    let capture_label = self
                        .capture_at
                        .map(|deadline| {
                            format!(
                                "Capturing in {}…",
                                deadline.saturating_duration_since(Instant::now()).as_secs() + 1
                            )
                        })
                        .unwrap_or_else(|| "Capture position…".into());
                    if ui
                        .add_enabled(
                            self.capture_at.is_none(),
                            egui::Button::new(capture_label).min_size(Vec2::new(160.0, 34.0)),
                        )
                        .clicked()
                    {
                        self.capture_at = Some(Instant::now() + Duration::from_secs(2));
                        self.status_message =
                            Some("Move the pointer to the target position.".into());
                    }
                    toggle(ui, &mut self.fixed_position);
                });
            });
            row_separator(ui);
            settings_row(ui, "Interval", "Delay between repeated actions", |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.interval_ms)
                            .clamp_range(10..=60_000)
                            .speed(10),
                    );
                    ui.label("ms");
                });
            });
            row_separator(ui);
            settings_row(
                ui,
                "Stop automatically",
                "End the run after a set amount of time",
                |ui| {
                    toggle(ui, &mut self.timed_run);
                },
            );
            row_separator(ui);
            settings_row(
                ui,
                "Run duration",
                "The timer starts when clicking begins",
                |ui| {
                    ui.add_enabled_ui(self.timed_run, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut self.duration_value)
                                    .clamp_range(1..=9_999),
                            );
                            egui::ComboBox::from_id_source("duration-unit")
                                .selected_text(["Seconds", "Minutes", "Hours"][self.duration_unit])
                                .show_ui(ui, |ui| {
                                    for (index, name) in
                                        ["Seconds", "Minutes", "Hours"].iter().enumerate()
                                    {
                                        ui.selectable_value(&mut self.duration_unit, index, *name);
                                    }
                                });
                        });
                    });
                },
            );
            row_separator(ui);
            settings_row(
                ui,
                "Stop after actions",
                "End after an exact number of clicks or key presses",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            self.count_limited,
                            egui::DragValue::new(&mut self.max_actions).clamp_range(1..=10_000_000),
                        );
                        toggle(ui, &mut self.count_limited);
                    });
                },
            );
            row_separator(ui);
            settings_row(ui, "Global toggle hotkey", self.hotkey_subtitle(), |ui| {
                let old_index = self.hotkey_index;
                egui::ComboBox::from_id_source("global-hotkey")
                    .selected_text(Hotkey::LABELS[self.hotkey_index])
                    .show_ui(ui, |ui| {
                        for (index, label) in Hotkey::LABELS.iter().enumerate() {
                            ui.selectable_value(&mut self.hotkey_index, index, *label);
                        }
                    });
                if self.hotkey_index != old_index {
                    self.hotkey_thread.set(Hotkey::ALL[self.hotkey_index]);
                }
            });
            row_separator(ui);
            settings_row(
                ui,
                "Keep in system tray",
                "Closing or minimizing hides this window; use the tray icon to reopen or quit",
                |ui| {
                    ui.add_enabled_ui(self.tray_icon.is_some(), |ui| {
                        toggle(ui, &mut self.tray_mode);
                    });
                },
            );
        });
    }

    fn presets_ui(&mut self, ui: &mut egui::Ui) {
        group_heading(ui, "Presets");
        card(ui, |ui| {
            settings_row(
                ui,
                "Saved preset",
                "Load a previously saved configuration",
                |ui| {
                    ui.horizontal(|ui| {
                        let selected = self
                            .preset_index
                            .and_then(|index| self.presets.get(index))
                            .map(|preset| preset.name)
                            .unwrap_or_else(|| "Choose a preset".into());
                        egui::ComboBox::from_id_source("saved-preset")
                            .selected_text(selected)
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for (index, name) in self.presets.names().iter().enumerate() {
                                    ui.selectable_value(&mut self.preset_index, Some(index), *name);
                                }
                            });
                        if ui
                            .add_sized([76.0, 34.0], egui::Button::new("Load"))
                            .clicked()
                        {
                            self.load_preset();
                        }
                    });
                },
            );
            row_separator(ui);
            settings_row(
                ui,
                "Save configuration",
                "Presets are stored in your user configuration folder",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [175.0, 34.0],
                            egui::TextEdit::singleline(&mut self.preset_name)
                                .hint_text("Preset name"),
                        );
                        if ui
                            .add_sized([112.0, 34.0], egui::Button::new("Save current"))
                            .clicked()
                        {
                            self.save_preset();
                        }
                    });
                },
            );
        });
    }

    fn status_ui(&mut self, ui: &mut egui::Ui) {
        group_heading(ui, "Status");
        card(ui, |ui| {
            let active = self.engine.is_active();
            ui.label(
                RichText::new(if active { "Clicking" } else { "Ready" })
                    .size(17.0)
                    .color(TEXT),
            );
            ui.add_space(3.0);
            let subtitle = if let Some(error) = self.engine.backend_error() {
                format!("Input backend error: {error}")
            } else if let Some(message) = &self.status_message {
                message.clone()
            } else if active {
                self.progress_text()
            } else {
                format!(
                    "Press {} or use the button below to start",
                    Hotkey::LABELS[self.hotkey_index]
                )
            };
            ui.label(RichText::new(subtitle).size(14.0).color(MUTED));
        });
        ui.add_space(2.0);
        let active = self.engine.is_active();
        let button = egui::Button::new(
            RichText::new(if active {
                "Stop clicking"
            } else {
                "Start clicking"
            })
            .size(17.0)
            .strong()
            .color(Color32::WHITE),
        )
        .fill(if active {
            Color32::from_rgb(210, 48, 48)
        } else {
            BLUE
        })
        .rounding(22.0);
        if ui.add_sized([ui.available_width(), 48.0], button).clicked() {
            self.toggle_clicking();
        }
    }

    fn action_subtitle(&self) -> String {
        match self.action_index {
            0 => "Left mouse click".into(),
            1 => "Middle mouse click".into(),
            2 => "Right mouse click".into(),
            3 => self
                .recorded_key
                .map(|(key, modifiers)| format!("{} key", key_label(key, modifiers)))
                .unwrap_or_else(|| "Record a keyboard key below".into()),
            _ => "Choose an action".into(),
        }
    }

    fn hotkey_subtitle(&self) -> String {
        if self.hotkey_thread.available() {
            "Works while this window is in the background".into()
        } else {
            format!(
                "{} is already in use by another application",
                Hotkey::LABELS[self.hotkey_index]
            )
        }
    }

    fn progress_text(&self) -> String {
        let mut parts = Vec::new();
        if let Some(remaining) = self.engine.remaining_ms() {
            parts.push(format!("{} remaining", format_duration(remaining)));
        }
        if let Some(remaining) = self.engine.remaining_actions() {
            parts.push(format!("{remaining} actions remaining"));
        }
        if parts.is_empty() {
            format!(
                "Press {} or Stop to finish",
                Hotkey::LABELS[self.hotkey_index]
            )
        } else {
            parts.join(" • ")
        }
    }
}

impl eframe::App for WindowsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_window_state(ctx);
        self.process_tray_events(ctx);
        self.process_keyboard_recording(ctx);
        self.process_capture();
        self.process_engine_state();

        egui::CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(248, 248, 248)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.set_max_width(700.0);
                            ui.add_space(12.0);
                            ui.label(RichText::new(APP_TITLE).size(19.0).strong().color(TEXT));
                            ui.add_space(28.0);
                            self.click_settings_ui(ui);
                            ui.add_space(26.0);
                            self.presets_ui(ui);
                            ui.add_space(26.0);
                            self.status_ui(ui);
                            ui.add_space(20.0);
                        });
                    });
            });

        ctx.request_repaint_after(Duration::from_millis(
            if self.engine.is_active() || self.capture_at.is_some() {
                50
            } else {
                150
            },
        ));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.engine.set_active(false);
    }
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(15.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(15.0, FontFamily::Proportional),
    );
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = Color32::from_rgb(248, 248, 248);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(235, 235, 235);
    style.visuals.widgets.inactive.rounding = Rounding::same(7.0);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(224, 224, 224);
    style.visuals.widgets.hovered.rounding = Rounding::same(7.0);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(205, 205, 205);
    style.visuals.widgets.active.rounding = Rounding::same(7.0);
    style.visuals.selection.bg_fill = BLUE;
    ctx.set_style(style);
}

fn install_tray_event_handlers(
    ctx: &egui::Context,
    pending_commands: Arc<AtomicU8>,
    show: &MenuItem,
    toggle: &MenuItem,
    quit: &MenuItem,
) {
    let show_id = show.id().clone();
    let toggle_id = toggle.id().clone();
    let quit_id = quit.id().clone();
    let menu_commands = Arc::clone(&pending_commands);
    let menu_context = ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let command = if event.id() == &show_id {
            TRAY_SHOW
        } else if event.id() == &toggle_id {
            TRAY_TOGGLE
        } else if event.id() == &quit_id {
            TRAY_QUIT
        } else {
            return;
        };
        if command == TRAY_SHOW {
            restore_native_window();
        } else if command == TRAY_QUIT {
            // Menu callbacks run on the Windows event-loop thread. Posting
            // WM_QUIT exits cleanly even while the eframe window is hidden.
            unsafe { PostQuitMessage(0) };
            return;
        }
        menu_commands.fetch_or(command, Ordering::Release);
        // A hidden eframe window does not receive periodic redraws on Windows.
        // Explicitly wake its event loop so the queued command is processed.
        menu_context.request_repaint();
    }));

    let tray_context = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            restore_native_window();
            pending_commands.fetch_or(TRAY_SHOW, Ordering::Release);
            tray_context.request_repaint();
        }
    }));
}

fn restore_native_window() {
    let title = to_wide(APP_TITLE);
    unsafe {
        let window = FindWindowW(ptr::null(), title.as_ptr());
        if !window.is_null() {
            ShowWindow(window, SW_RESTORE);
            SetForegroundWindow(window);
        }
    }
}

fn group_heading(ui: &mut egui::Ui, title: &str) {
    // `with_layout` consumes all remaining vertical space. A horizontal row
    // stays content-height, which keeps the settings card directly below its
    // heading on short or display-scaled Windows screens.
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(20.0).strong().color(TEXT));
    });
    ui.add_space(7.0);
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    Frame::none()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(222, 222, 222)))
        .rounding(11.0)
        .inner_margin(Margin::symmetric(14.0, 5.0))
        .shadow(egui::epaint::Shadow {
            offset: Vec2::new(0.0, 1.0),
            blur: 4.0,
            spread: 0.0,
            color: Color32::from_black_alpha(22),
        })
        .show(ui, contents);
}

fn settings_row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: impl Into<String>,
    controls: impl FnOnce(&mut egui::Ui),
) {
    let subtitle = subtitle.into();
    ui.horizontal(|ui| {
        ui.set_min_height(52.0);
        let left_width = (ui.available_width() * 0.53).clamp(260.0, 370.0);
        ui.allocate_ui_with_layout(
            Vec2::new(left_width, 50.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.add_space(2.0);
                ui.label(RichText::new(title).size(16.0).color(TEXT));
                ui.label(RichText::new(subtitle).size(13.5).color(MUTED));
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), controls);
    });
}

fn row_separator(ui: &mut egui::Ui) {
    ui.separator();
}

fn segment_button(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    ui.add_sized(
        [76.0, 36.0],
        egui::Button::new(RichText::new(text).strong().color(TEXT))
            .fill(if selected {
                Color32::from_rgb(198, 198, 198)
            } else {
                Color32::from_rgb(235, 235, 235)
            })
            .rounding(6.0),
    )
}

fn toggle(ui: &mut egui::Ui, enabled: &mut bool) -> egui::Response {
    let desired_size = Vec2::new(46.0, 26.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *enabled = !*enabled;
        response.mark_changed();
    }
    let amount = ui.ctx().animate_bool(response.id, *enabled);
    let background = if *enabled {
        BLUE
    } else {
        Color32::from_rgb(215, 215, 215)
    };
    ui.painter()
        .rect_filled(rect, rect.height() / 2.0, background);
    let radius = rect.height() * 0.38;
    let x = egui::lerp(
        (rect.left() + rect.height() / 2.0)..=(rect.right() - rect.height() / 2.0),
        amount,
    );
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), radius, Color32::WHITE);
    response
}

fn duration_ms(value: u64, unit: usize) -> u64 {
    value.saturating_mul(match unit {
        0 => 1_000,
        1 => 60_000,
        _ => 3_600_000,
    })
}

fn split_duration(milliseconds: u64) -> (u64, usize) {
    if milliseconds >= 3_600_000 && milliseconds % 3_600_000 == 0 {
        (milliseconds / 3_600_000, 2)
    } else if milliseconds >= 60_000 && milliseconds % 60_000 == 0 {
        (milliseconds / 60_000, 1)
    } else {
        ((milliseconds / 1_000).max(1), 0)
    }
}

fn format_duration(milliseconds: u64) -> String {
    let seconds = milliseconds.div_ceil(1_000);
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn action_label(action: Action) -> String {
    match action {
        Action::LeftClick => "Left mouse click".into(),
        Action::MiddleClick => "Middle mouse click".into(),
        Action::RightClick => "Right mouse click".into(),
        Action::Key { keysym, modifiers } => key_label(keysym as u32, modifiers),
    }
}

fn key_is_down(virtual_key: i32) -> bool {
    unsafe { GetAsyncKeyState(virtual_key) < 0 }
}

fn key_to_virtual_key(key: egui::Key) -> Option<u32> {
    use egui::Key;
    Some(match key {
        Key::Backspace => 0x08,
        Key::Tab => 0x09,
        Key::Enter => 0x0d,
        Key::Escape => 0x1b,
        Key::Space => 0x20,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::End => 0x23,
        Key::Home => 0x24,
        Key::ArrowLeft => 0x25,
        Key::ArrowUp => 0x26,
        Key::ArrowRight => 0x27,
        Key::ArrowDown => 0x28,
        Key::Insert => 0x2d,
        Key::Delete => 0x2e,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4a,
        Key::K => 0x4b,
        Key::L => 0x4c,
        Key::M => 0x4d,
        Key::N => 0x4e,
        Key::O => 0x4f,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5a,
        Key::F1 => 0x70,
        Key::F2 => 0x71,
        Key::F3 => 0x72,
        Key::F4 => 0x73,
        Key::F5 => 0x74,
        Key::F6 => 0x75,
        Key::F7 => 0x76,
        Key::F8 => 0x77,
        Key::F9 => 0x78,
        Key::F10 => 0x79,
        Key::F11 => 0x7a,
        Key::F12 => 0x7b,
        Key::F13 => 0x7c,
        Key::F14 => 0x7d,
        Key::F15 => 0x7e,
        Key::F16 => 0x7f,
        Key::F17 => 0x80,
        Key::F18 => 0x81,
        Key::F19 => 0x82,
        Key::F20 => 0x83,
        Key::F21 => 0x84,
        Key::F22 => 0x85,
        Key::F23 => 0x86,
        Key::F24 => 0x87,
        Key::Semicolon | Key::Colon => 0xba,
        Key::Plus | Key::Equals => 0xbb,
        Key::Comma => 0xbc,
        Key::Minus => 0xbd,
        Key::Period => 0xbe,
        Key::Slash | Key::Questionmark => 0xbf,
        Key::Backtick => 0xc0,
        Key::OpenBracket => 0xdb,
        Key::Backslash | Key::Pipe => 0xdc,
        Key::CloseBracket => 0xdd,
        Key::Copy
        | Key::Cut
        | Key::Paste
        | Key::F25
        | Key::F26
        | Key::F27
        | Key::F28
        | Key::F29
        | Key::F30
        | Key::F31
        | Key::F32
        | Key::F33
        | Key::F34
        | Key::F35 => return None,
    })
}

fn key_label(key: u32, modifiers: KeyModifiers) -> String {
    let mut pieces = Vec::new();
    if modifiers.control {
        pieces.push("Ctrl".to_string());
    }
    if modifiers.shift {
        pieces.push("Shift".to_string());
    }
    if modifiers.alt {
        pieces.push("Alt".to_string());
    }
    if modifiers.super_key {
        pieces.push("Windows".to_string());
    }
    let key_name = match key {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0d => "Enter".into(),
        0x1b => "Escape".into(),
        0x20 => "Space".into(),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2d => "Insert".into(),
        0x2e => "Delete".into(),
        0x30..=0x39 | 0x41..=0x5a => char::from_u32(key).unwrap_or('?').to_string(),
        0x70..=0x87 => format!("F{}", key - 0x6f),
        0xba => ";".into(),
        0xbb => "=".into(),
        0xbc => ",".into(),
        0xbd => "-".into(),
        0xbe => ".".into(),
        0xbf => "/".into(),
        0xc0 => "`".into(),
        0xdb => "[".into(),
        0xdc => "\\".into(),
        0xdd => "]".into(),
        _ => format!("Key 0x{key:02X}"),
    };
    pieces.push(key_name);
    pieces.join("+")
}

fn app_icon(active: bool) -> (Vec<u8>, u32, u32) {
    let size = 32u32;
    let center = (size - 1) as f32 / 2.0;
    let radius = center - 1.0;
    let color = if active {
        (224, 42, 42)
    } else {
        (28, 126, 224)
    };
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            if dx * dx + dy * dy <= radius * radius {
                let white_mark = ((14..=17).contains(&x) && (7..=23).contains(&y))
                    || ((10..=21).contains(&x) && (14..=17).contains(&y));
                let (red, green, blue) = if white_mark { (255, 255, 255) } else { color };
                rgba.extend_from_slice(&[red, green, blue, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (rgba, size, size)
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

struct WindowsHotkeyThread {
    sender: mpsc::Sender<Hotkey>,
    stop: Arc<AtomicBool>,
    available: Arc<AtomicBool>,
}

impl WindowsHotkeyThread {
    fn start(toggle_requested: Arc<AtomicBool>, initial: Hotkey) -> Self {
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let available = Arc::new(AtomicBool::new(true));
        let thread_stop = Arc::clone(&stop);
        let thread_available = Arc::clone(&available);
        thread::spawn(move || unsafe {
            let mut message: MSG = zeroed();
            PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE);
            let mut current = initial;
            thread_available.store(
                RegisterHotKey(ptr::null_mut(), 1, 0x4000, current.virtual_key()) != 0,
                Ordering::Release,
            );
            while !thread_stop.load(Ordering::Acquire) {
                while let Ok(next) = receiver.try_recv() {
                    UnregisterHotKey(ptr::null_mut(), 1);
                    current = next;
                    thread_available.store(
                        RegisterHotKey(ptr::null_mut(), 1, 0x4000, current.virtual_key()) != 0,
                        Ordering::Release,
                    );
                }
                while PeekMessageW(
                    &mut message,
                    ptr::null_mut(),
                    WM_HOTKEY,
                    WM_HOTKEY,
                    PM_REMOVE,
                ) != 0
                {
                    if message.message == WM_HOTKEY && message.wParam == 1 {
                        toggle_requested.store(true, Ordering::Release);
                    }
                }
                thread::sleep(Duration::from_millis(15));
            }
            UnregisterHotKey(ptr::null_mut(), 1);
        });
        Self {
            sender,
            stop,
            available,
        }
    }

    fn set(&self, hotkey: Hotkey) {
        let _ = self.sender.send(hotkey);
    }

    fn available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }
}

impl Drop for WindowsHotkeyThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::{duration_ms, format_duration, group_heading, split_duration};

    #[test]
    fn duration_units_round_trip() {
        assert_eq!(duration_ms(10, 1), 600_000);
        assert_eq!(split_duration(600_000), (10, 1));
    }

    #[test]
    fn remaining_time_is_readable() {
        assert_eq!(format_duration(3_661_000), "1:01:01");
        assert_eq!(format_duration(9_001), "0:10");
    }

    #[test]
    fn section_heading_does_not_consume_the_viewport_height() {
        let context = eframe::egui::Context::default();
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                eframe::egui::Pos2::ZERO,
                eframe::egui::vec2(700.0, 700.0),
            )),
            ..Default::default()
        };
        let mut used_height = 0.0;

        let _ = context.run(input, |context| {
            eframe::egui::CentralPanel::default().show(context, |ui| {
                let top = ui.cursor().top();
                group_heading(ui, "Click settings");
                used_height = ui.cursor().top() - top;
            });
        });

        assert!(
            used_height < 80.0,
            "section heading unexpectedly used {used_height}px"
        );
    }
}
