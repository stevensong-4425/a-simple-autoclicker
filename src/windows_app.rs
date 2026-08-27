use std::{
    cell::{Cell, RefCell},
    mem::zeroed,
    ptr,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

use native_windows_gui as nwg;
use windows_sys::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
    },
    WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY},
};

use crate::{
    backend,
    clicker::ClickEngine,
    model::{Action, Hotkey, KeyModifiers},
    presets::{Preset, PresetStore},
};

const APP_TITLE: &str = "A Simple Autoclicker";

pub fn run() -> Result<(), String> {
    nwg::init().map_err(|error| error.to_string())?;
    nwg::Font::set_global_family("Segoe UI").map_err(|error| error.to_string())?;

    let app = App::build().map_err(|error| error.to_string())?;
    let weak = Rc::downgrade(&app);
    let handler = nwg::full_bind_event_handler(&app.window.handle, move |event, data, handle| {
        let Some(app) = weak.upgrade() else { return };
        app.handle_event(event, data, handle);
    });

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
    Ok(())
}

struct App {
    engine: Arc<ClickEngine>,
    hotkey_thread: WindowsHotkeyThread,
    hotkey_toggle: Arc<AtomicBool>,
    presets: RefCell<PresetStore>,
    recorded_key: Cell<Option<(u32, KeyModifiers)>>,
    recording: Cell<bool>,

    window: nwg::Window,
    action_combo: nwg::ComboBox<String>,
    record_button: nwg::Button,
    recorded_label: nwg::Label,
    interval_input: nwg::TextInput,
    duration_input: nwg::TextInput,
    count_input: nwg::TextInput,
    fixed_check: nwg::CheckBox,
    position_label: nwg::Label,
    capture_button: nwg::Button,
    hotkey_combo: nwg::ComboBox<String>,
    preset_combo: nwg::ComboBox<String>,
    preset_name: nwg::TextInput,
    preset_load: nwg::Button,
    preset_save: nwg::Button,
    start_button: nwg::Button,
    status_label: nwg::Label,
    timer: nwg::AnimationTimer,

    _icon: nwg::Icon,
    tray: nwg::TrayNotification,
    tray_menu: nwg::Menu,
    tray_show: nwg::MenuItem,
    tray_toggle: nwg::MenuItem,
    tray_exit: nwg::MenuItem,
}

impl App {
    fn build() -> Result<Rc<Self>, nwg::NwgError> {
        let engine = ClickEngine::start();
        let hotkey_toggle = Arc::new(AtomicBool::new(false));
        let hotkey_thread = WindowsHotkeyThread::start(Arc::clone(&hotkey_toggle), Hotkey::F8);
        let presets = PresetStore::load();

        let mut window = nwg::Window::default();
        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
            .size((640, 700))
            .position((300, 100))
            .title(APP_TITLE)
            .center(true)
            .build(&mut window)?;

        let mut action_title = nwg::Label::default();
        label(&window, &mut action_title, "Action", (24, 20), (130, 24))?;
        let mut action_combo = nwg::ComboBox::default();
        nwg::ComboBox::builder()
            .collection(vec![
                "Left mouse click".into(),
                "Middle mouse click".into(),
                "Right mouse click".into(),
                "Recorded keyboard key".into(),
            ])
            .selected_index(Some(0))
            .position((180, 18))
            .size((300, 120))
            .parent(&window)
            .build(&mut action_combo)?;

        let mut record_button = nwg::Button::default();
        button(
            &window,
            &mut record_button,
            "Record keyboard key",
            (180, 58),
            (180, 30),
        )?;
        let mut recorded_label = nwg::Label::default();
        label(
            &window,
            &mut recorded_label,
            "No key recorded",
            (370, 62),
            (230, 24),
        )?;

        let mut interval_title = nwg::Label::default();
        label(
            &window,
            &mut interval_title,
            "Interval (milliseconds)",
            (24, 108),
            (150, 24),
        )?;
        let mut interval_input = nwg::TextInput::default();
        input(&window, &mut interval_input, "100", (180, 104), (120, 28))?;

        let mut duration_title = nwg::Label::default();
        label(
            &window,
            &mut duration_title,
            "Stop after (seconds)",
            (24, 148),
            (150, 24),
        )?;
        let mut duration_input = nwg::TextInput::default();
        input(&window, &mut duration_input, "0", (180, 144), (120, 28))?;
        let mut duration_hint = nwg::Label::default();
        label(
            &window,
            &mut duration_hint,
            "0 = never",
            (315, 148),
            (110, 24),
        )?;

        let mut count_title = nwg::Label::default();
        label(
            &window,
            &mut count_title,
            "Stop after actions",
            (24, 188),
            (150, 24),
        )?;
        let mut count_input = nwg::TextInput::default();
        input(&window, &mut count_input, "0", (180, 184), (120, 28))?;
        let mut count_hint = nwg::Label::default();
        label(
            &window,
            &mut count_hint,
            "0 = unlimited",
            (315, 188),
            (110, 24),
        )?;

        let mut fixed_check = nwg::CheckBox::default();
        nwg::CheckBox::builder()
            .text("Click at a fixed position")
            .position((24, 232))
            .size((190, 28))
            .parent(&window)
            .build(&mut fixed_check)?;
        let mut position_label = nwg::Label::default();
        label(
            &window,
            &mut position_label,
            "Position: not set",
            (225, 235),
            (170, 24),
        )?;
        let mut capture_button = nwg::Button::default();
        button(
            &window,
            &mut capture_button,
            "Capture current pointer",
            (405, 230),
            (190, 30),
        )?;

        let mut hotkey_title = nwg::Label::default();
        label(
            &window,
            &mut hotkey_title,
            "Global start/stop key",
            (24, 278),
            (150, 24),
        )?;
        let mut hotkey_combo = nwg::ComboBox::default();
        nwg::ComboBox::builder()
            .collection(
                Hotkey::LABELS
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
            )
            .selected_index(Some(2))
            .position((180, 274))
            .size((120, 120))
            .parent(&window)
            .build(&mut hotkey_combo)?;

        let mut separator = nwg::Label::default();
        label(
            &window,
            &mut separator,
            "Saved presets",
            (24, 330),
            (150, 24),
        )?;
        let mut preset_combo = nwg::ComboBox::default();
        nwg::ComboBox::builder()
            .collection(
                presets
                    .names()
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect(),
            )
            .position((180, 326))
            .size((300, 150))
            .parent(&window)
            .build(&mut preset_combo)?;
        let mut preset_load = nwg::Button::default();
        button(&window, &mut preset_load, "Load", (490, 325), (105, 30))?;

        let mut preset_name_title = nwg::Label::default();
        label(
            &window,
            &mut preset_name_title,
            "Preset name",
            (24, 372),
            (150, 24),
        )?;
        let mut preset_name = nwg::TextInput::default();
        input(&window, &mut preset_name, "", (180, 368), (300, 28))?;
        let mut preset_save = nwg::Button::default();
        button(&window, &mut preset_save, "Save", (490, 367), (105, 30))?;

        let mut start_button = nwg::Button::default();
        button(&window, &mut start_button, "Start", (24, 430), (571, 48))?;
        let mut status_label = nwg::Label::default();
        label(
            &window,
            &mut status_label,
            "Ready — press F8 or Start",
            (24, 495),
            (571, 60),
        )?;

        let mut help_label = nwg::Label::default();
        label(
            &window,
            &mut help_label,
            "Closing this window keeps the app in the system tray.\nSome elevated apps only accept clicks when this app is also run as administrator.",
            (24, 570),
            (571, 54),
        )?;

        let mut timer = nwg::AnimationTimer::default();
        nwg::AnimationTimer::builder()
            .interval(Duration::from_millis(150))
            .active(true)
            .parent(&window)
            .build(&mut timer)?;

        let mut icon = nwg::Icon::default();
        nwg::Icon::builder()
            .source_system(Some(nwg::OemIcon::WinLogo))
            .build(&mut icon)?;
        window.set_icon(Some(&icon));

        let mut tray = nwg::TrayNotification::default();
        nwg::TrayNotification::builder()
            .parent(&window)
            .icon(Some(&icon))
            .tip(Some(APP_TITLE))
            .build(&mut tray)?;
        let mut tray_menu = nwg::Menu::default();
        nwg::Menu::builder()
            .popup(true)
            .parent(&window)
            .build(&mut tray_menu)?;
        let mut tray_show = nwg::MenuItem::default();
        nwg::MenuItem::builder()
            .text("Show window")
            .parent(&tray_menu)
            .build(&mut tray_show)?;
        let mut tray_toggle = nwg::MenuItem::default();
        nwg::MenuItem::builder()
            .text("Start / Stop")
            .parent(&tray_menu)
            .build(&mut tray_toggle)?;
        let mut tray_exit = nwg::MenuItem::default();
        nwg::MenuItem::builder()
            .text("Exit")
            .parent(&tray_menu)
            .build(&mut tray_exit)?;

        Ok(Rc::new(Self {
            engine,
            hotkey_thread,
            hotkey_toggle,
            presets: RefCell::new(presets),
            recorded_key: Cell::new(None),
            recording: Cell::new(false),
            window,
            action_combo,
            record_button,
            recorded_label,
            interval_input,
            duration_input,
            count_input,
            fixed_check,
            position_label,
            capture_button,
            hotkey_combo,
            preset_combo,
            preset_name,
            preset_load,
            preset_save,
            start_button,
            status_label,
            timer,
            _icon: icon,
            tray,
            tray_menu,
            tray_show,
            tray_toggle,
            tray_exit,
        }))
    }

    fn handle_event(&self, event: nwg::Event, data: nwg::EventData, handle: nwg::ControlHandle) {
        use nwg::Event as E;
        match event {
            E::OnButtonClick if handle == self.record_button => self.begin_recording(),
            E::OnButtonClick if handle == self.capture_button => self.capture_position(),
            E::OnButtonClick if handle == self.start_button => self.toggle_from_ui(),
            E::OnButtonClick if handle == self.preset_save => self.save_preset(),
            E::OnButtonClick if handle == self.preset_load => self.load_preset(),
            E::OnComboxBoxSelection if handle == self.hotkey_combo => self.update_hotkey(),
            E::OnKeyPress if self.recording.get() => self.record_key(data.on_key()),
            E::OnTimerTick if handle == self.timer => self.refresh_status(),
            E::OnWindowClose if handle == self.window => self.window.set_visible(false),
            E::OnWindowMinimize if handle == self.window => self.window.set_visible(false),
            E::OnMousePress(nwg::MousePressEvent::MousePressLeftUp) if handle == self.tray => {
                self.show_window()
            }
            E::OnContextMenu if handle == self.tray => {
                let (x, y) = nwg::GlobalCursor::position();
                self.tray_menu.popup(x, y);
            }
            E::OnMenuItemSelected if handle == self.tray_show => self.show_window(),
            E::OnMenuItemSelected if handle == self.tray_toggle => self.toggle_from_ui(),
            E::OnMenuItemSelected if handle == self.tray_exit => {
                self.engine.set_active(false);
                nwg::stop_thread_dispatch();
            }
            _ => {}
        }
    }

    fn begin_recording(&self) {
        self.recording.set(true);
        self.record_button.set_text("Press a key now…");
        self.window.set_focus();
    }

    fn record_key(&self, virtual_key: u32) {
        if is_modifier_key(virtual_key) {
            return;
        }
        let modifiers = KeyModifiers {
            shift: key_is_down(VK_SHIFT as i32),
            control: key_is_down(VK_CONTROL as i32),
            alt: key_is_down(VK_MENU as i32),
            super_key: key_is_down(VK_LWIN as i32),
        };
        self.recorded_key.set(Some((virtual_key, modifiers)));
        self.recording.set(false);
        self.record_button.set_text("Record keyboard key");
        self.recorded_label
            .set_text(&key_label(virtual_key, modifiers));
        self.action_combo.set_selection(Some(3));
    }

    fn capture_position(&self) {
        match backend::pointer_position() {
            Ok(position) => {
                self.engine.set_position(Some(position));
                self.fixed_check
                    .set_check_state(nwg::CheckBoxState::Checked);
                self.position_label
                    .set_text(&format!("Position: {}, {}", position.x, position.y));
            }
            Err(error) => {
                nwg::modal_error_message(&self.window, "Pointer capture failed", &error);
            }
        }
    }

    fn toggle_from_ui(&self) {
        if self.engine.is_active() {
            self.engine.set_active(false);
            return;
        }
        match self.apply_controls_to_engine() {
            Ok(()) => self.engine.set_active(true),
            Err(error) => {
                nwg::modal_error_message(&self.window, "Check the settings", &error);
            }
        }
    }

    fn apply_controls_to_engine(&self) -> Result<(), String> {
        let interval = parse_u64(&self.interval_input, "Interval")?.max(10);
        let duration_seconds = parse_u64(&self.duration_input, "Duration")?;
        let max_actions = parse_u64(&self.count_input, "Action limit")?;
        let action = self.selected_action()?;
        let fixed = self.fixed_check.check_state() == nwg::CheckBoxState::Checked;
        if fixed && self.engine.position().is_none() {
            return Err("Capture a pointer position first, or turn off fixed position.".into());
        }
        self.engine.set_interval_ms(interval);
        self.engine
            .set_duration_ms(duration_seconds.saturating_mul(1000));
        self.engine.set_max_actions(max_actions);
        self.engine.set_action(action);
        if !fixed {
            self.engine.set_position(None);
            self.position_label.set_text("Position: not set");
        }
        Ok(())
    }

    fn selected_action(&self) -> Result<Action, String> {
        match self.action_combo.selection().unwrap_or(0) {
            0 => Ok(Action::LeftClick),
            1 => Ok(Action::MiddleClick),
            2 => Ok(Action::RightClick),
            3 => self
                .recorded_key
                .get()
                .map(|(keysym, modifiers)| Action::Key {
                    keysym: keysym as u64,
                    modifiers,
                })
                .ok_or_else(|| "Record a keyboard key first.".to_string()),
            _ => Err("Choose a valid action.".into()),
        }
    }

    fn update_hotkey(&self) {
        let hotkey = selected_hotkey(&self.hotkey_combo);
        self.hotkey_thread.set(hotkey);
        self.refresh_status();
    }

    fn save_preset(&self) {
        let name = self.preset_name.text().trim().to_string();
        if name.is_empty() {
            nwg::modal_error_message(
                &self.window,
                "Preset name required",
                "Enter a name for this preset.",
            );
            return;
        }
        if let Err(error) = self.apply_controls_to_engine() {
            nwg::modal_error_message(&self.window, "Check the settings", &error);
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
            position: self.engine.position(),
            hotkey: selected_hotkey(&self.hotkey_combo),
        };
        if let Err(error) = self.presets.borrow_mut().save(preset) {
            nwg::modal_error_message(&self.window, "Could not save preset", &error);
            return;
        }
        let names = self
            .presets
            .borrow()
            .names()
            .iter()
            .map(|name| (*name).to_string())
            .collect();
        self.preset_combo.set_collection(names);
        self.preset_combo.set_selection_string(&name);
        self.status_label.set_text("Preset saved");
    }

    fn load_preset(&self) {
        let Some(index) = self.preset_combo.selection() else {
            nwg::modal_error_message(
                &self.window,
                "Choose a preset",
                "Select a saved preset first.",
            );
            return;
        };
        let Some(preset) = self.presets.borrow().get(index) else {
            return;
        };
        self.interval_input
            .set_text(&preset.interval_ms.to_string());
        self.duration_input
            .set_text(&(preset.duration_ms / 1000).to_string());
        self.count_input.set_text(&preset.max_actions.to_string());
        self.preset_name.set_text(&preset.name);
        match preset.action {
            Action::LeftClick => self.action_combo.set_selection(Some(0)),
            Action::MiddleClick => self.action_combo.set_selection(Some(1)),
            Action::RightClick => self.action_combo.set_selection(Some(2)),
            Action::Key { keysym, modifiers } => {
                self.recorded_key.set(Some((keysym as u32, modifiers)));
                self.recorded_label
                    .set_text(&key_label(keysym as u32, modifiers));
                self.action_combo.set_selection(Some(3));
            }
        }
        self.engine.set_position(preset.position);
        self.fixed_check
            .set_check_state(if preset.position.is_some() {
                nwg::CheckBoxState::Checked
            } else {
                nwg::CheckBoxState::Unchecked
            });
        if let Some(position) = preset.position {
            self.position_label
                .set_text(&format!("Position: {}, {}", position.x, position.y));
        } else {
            self.position_label.set_text("Position: not set");
        }
        let hotkey_index = Hotkey::ALL
            .iter()
            .position(|key| *key == preset.hotkey)
            .unwrap_or(2);
        self.hotkey_combo.set_selection(Some(hotkey_index));
        self.hotkey_thread.set(preset.hotkey);
        self.status_label.set_text("Preset loaded");
    }

    fn refresh_status(&self) {
        if self.hotkey_toggle.swap(false, Ordering::AcqRel) {
            self.toggle_from_ui();
        }
        let hotkey = selected_hotkey(&self.hotkey_combo);
        if let Some(error) = self.engine.backend_error() {
            self.status_label.set_text(&format!("Input error: {error}"));
            self.start_button.set_text("Start");
        } else if self.engine.is_active() {
            let time = self
                .engine
                .remaining_ms()
                .map(|ms| format!(" • {:.1}s left", ms as f64 / 1000.0))
                .unwrap_or_default();
            let count = self
                .engine
                .remaining_actions()
                .map(|value| format!(" • {value} actions left"))
                .unwrap_or_default();
            self.status_label.set_text(&format!("Running{time}{count}"));
            self.start_button.set_text("Stop");
        } else {
            self.start_button.set_text("Start");
            let suffix = if self.engine.take_completed_run() {
                " • limit reached"
            } else {
                ""
            };
            self.status_label.set_text(&format!(
                "Ready — press {} or Start{suffix}",
                hotkey_label(hotkey)
            ));
        }
    }

    fn show_window(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
    }
}

fn label(
    parent: &nwg::Window,
    output: &mut nwg::Label,
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
) -> Result<(), nwg::NwgError> {
    nwg::Label::builder()
        .text(text)
        .position(position)
        .size(size)
        .parent(parent)
        .build(output)
}

fn button(
    parent: &nwg::Window,
    output: &mut nwg::Button,
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
) -> Result<(), nwg::NwgError> {
    nwg::Button::builder()
        .text(text)
        .position(position)
        .size(size)
        .parent(parent)
        .build(output)
}

fn input(
    parent: &nwg::Window,
    output: &mut nwg::TextInput,
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
) -> Result<(), nwg::NwgError> {
    nwg::TextInput::builder()
        .text(text)
        .position(position)
        .size(size)
        .parent(parent)
        .build(output)
}

fn parse_u64(input: &nwg::TextInput, name: &str) -> Result<u64, String> {
    input
        .text()
        .trim()
        .parse()
        .map_err(|_| format!("{name} must be a whole number."))
}

fn selected_hotkey(combo: &nwg::ComboBox<String>) -> Hotkey {
    Hotkey::ALL
        .get(combo.selection().unwrap_or(2))
        .copied()
        .unwrap_or(Hotkey::F8)
}

fn hotkey_label(hotkey: Hotkey) -> &'static str {
    Hotkey::LABELS[Hotkey::ALL
        .iter()
        .position(|key| *key == hotkey)
        .unwrap_or(2)]
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

fn is_modifier_key(key: u32) -> bool {
    matches!(key, 0x10 | 0x11 | 0x12 | 0x5b | 0x5c | 0xa0..=0xa5)
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
        0x60..=0x69 => format!("Numpad {}", key - 0x60),
        0x70..=0x87 => format!("F{}", key - 0x6f),
        _ => format!("Key 0x{key:02X}"),
    };
    pieces.push(key_name);
    pieces.join("+")
}

struct WindowsHotkeyThread {
    sender: mpsc::Sender<Hotkey>,
    stop: Arc<AtomicBool>,
}

impl WindowsHotkeyThread {
    fn start(toggle_requested: Arc<AtomicBool>, initial: Hotkey) -> Self {
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        thread::spawn(move || unsafe {
            let mut message: MSG = zeroed();
            PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE);
            let mut current = initial;
            RegisterHotKey(ptr::null_mut(), 1, 0x4000, current.virtual_key());
            while !thread_stop.load(Ordering::Acquire) {
                while let Ok(next) = receiver.try_recv() {
                    UnregisterHotKey(ptr::null_mut(), 1);
                    current = next;
                    RegisterHotKey(ptr::null_mut(), 1, 0x4000, current.virtual_key());
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
        Self { sender, stop }
    }

    fn set(&self, hotkey: Hotkey) {
        let _ = self.sender.send(hotkey);
    }
}

impl Drop for WindowsHotkeyThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
