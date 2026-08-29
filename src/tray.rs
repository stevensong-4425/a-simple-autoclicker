use std::sync::{mpsc::Sender, Arc};

use crate::{clicker::ClickEngine, icon::mouse_rgba};

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
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        [22, 32]
            .into_iter()
            .map(|size| tray_icon(size, self.engine.is_active()))
            .collect()
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

fn tray_icon(size: i32, active: bool) -> ksni::Icon {
    let rgba = mouse_rgba(size as u32, active);
    let mut data = Vec::with_capacity(rgba.len());
    // StatusNotifierItem pixmaps use ARGB rather than the RGBA order used by
    // eframe and tray-icon on Windows.
    for pixel in rgba.as_chunks::<4>().0 {
        data.extend_from_slice(&[pixel[3], pixel[0], pixel[1], pixel[2]]);
    }

    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

pub fn start(engine: Arc<ClickEngine>, sender: Sender<TrayCommand>) -> TrayHandle {
    let service = ksni::TrayService::new(AutoclickerTray { engine, sender });
    let handle = TrayHandle(service.handle());
    service.spawn();
    handle
}

#[cfg(test)]
mod tests {
    use super::tray_icon;

    #[test]
    fn active_tray_icon_is_red() {
        let icon = tray_icon(22, true);
        assert!(icon
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[0] == 255 && pixel[1] > 200 && pixel[2] < 80));
    }

    #[test]
    fn idle_tray_icon_is_grey() {
        let icon = tray_icon(22, false);
        assert!(icon.data.as_chunks::<4>().0.iter().any(|pixel| {
            pixel[0] == 255 && pixel[1].abs_diff(pixel[2]) < 12 && pixel[2].abs_diff(pixel[3]) < 12
        }));
    }
}
