#[cfg(target_os = "linux")]
mod x11;

use crate::model::{Action, ClickPosition};

pub trait InputBackend: Send {
    fn perform(&mut self, action: Action, position: Option<ClickPosition>) -> Result<(), String>;
}

pub fn pointer_position() -> Result<ClickPosition, String> {
    #[cfg(target_os = "linux")]
    {
        return x11::pointer_position();
    }

    #[allow(unreachable_code)]
    Err("Pointer capture is not available on this platform yet".into())
}

pub fn create() -> Result<Box<dyn InputBackend>, String> {
    #[cfg(target_os = "linux")]
    {
        return x11::X11Backend::new().map(|backend| Box::new(backend) as Box<dyn InputBackend>);
    }

    #[allow(unreachable_code)]
    Err("This platform does not have an input backend yet".into())
}
