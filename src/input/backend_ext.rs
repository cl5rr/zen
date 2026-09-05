use ::input as libinput;
use smithay::backend::input;
use smithay::backend::winit::WinitVirtualDevice;
use smithay::output::Output;

use crate::state::State;
use crate::protocols::virtual_pointer::VirtualPointer;

pub trait ZenInputBackend: input::InputBackend<Device = Self::ZenDevice> {
    type ZenDevice: ZenInputDevice;
}
impl<T: input::InputBackend> ZenInputBackend for T
where
    Self::Device: ZenInputDevice,
{
    type ZenDevice = Self::Device;
}

pub trait ZenInputDevice: input::Device {
    fn output(&self, state: &State) -> Option<Output>;
}

impl ZenInputDevice for libinput::Device {
    fn output(&self, _state: &State) -> Option<Output> {
        None
    }
}

impl ZenInputDevice for WinitVirtualDevice {
    fn output(&self, _state: &State) -> Option<Output> {
        None
    }
}

impl ZenInputDevice for VirtualPointer {
    fn output(&self, _: &State) -> Option<Output> {
        self.output().cloned()
    }
}
