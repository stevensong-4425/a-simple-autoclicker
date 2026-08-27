use std::sync::{mpsc::Sender, Arc};

use crate::clicker::ClickEngine;

#[derive(Clone, Copy, Debug)]
pub enum TrayCommand {
    ShowWindow,
    Quit,
}

struct AutoclickerTray {
    engine: Arc<ClickEngine>,
    sender: Sender<TrayCommand>,
}

pub struct TrayHandle(ksni::Handle<AutoclickerTray>);

impl TrayHandle {
    pub fn refresh(&self) {
        self.0.update(|_| {});
    }
}

impl ksni::Tray for AutoclickerTray {
    fn id(&self) -> String {
        "a-simple-autoclicker".into()
    }

    fn title(&self) -> String {
        if self.engine.is_active() {
            "A Simple Autoclicker — Clicking"
        } else {
            "A Simple Autoclicker"
        }
        .into()
    }

    fn icon_name(&self) -> String {
        "input-mouse".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.send(TrayCommand::ShowWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let toggle_label = if self.engine.is_active() {
            "Stop clicking"
        } else {
            "Start clicking"
        };
        vec![
            StandardItem {
                label: "Show window".into(),
                icon_name: "window-new".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send(TrayCommand::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: toggle_label.into(),
                activate: Box::new(|tray: &mut Self| tray.engine.toggle()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn start(engine: Arc<ClickEngine>, sender: Sender<TrayCommand>) -> TrayHandle {
    let service = ksni::TrayService::new(AutoclickerTray { engine, sender });
    let handle = TrayHandle(service.handle());
    service.spawn();
    handle
}
