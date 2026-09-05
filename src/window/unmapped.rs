use zen_config::PresetSize;
use smithay::desktop::Window;
use smithay::output::Output;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::xdg_activation::XdgActivationTokenData;

use super::ResolvedWindowRules;

#[derive(Debug)]
pub struct Unmapped {
    pub window: Window,
    pub state: InitialConfigureState,
    pub activation_token_data: Option<XdgActivationTokenData>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum InitialConfigureState {
    NotConfigured {
        wants_fullscreen: Option<Option<Output>>,

        wants_maximized: bool,
    },
    Configured {
        rules: ResolvedWindowRules,

        width: Option<PresetSize>,

        height: Option<PresetSize>,

        floating_width: Option<PresetSize>,

        floating_height: Option<PresetSize>,

        is_full_width: bool,

        output: Option<Output>,

        workspace_name: Option<String>,

        is_pending_maximized: bool,
    },
}

impl Unmapped {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            state: InitialConfigureState::NotConfigured {
                wants_fullscreen: None,
                wants_maximized: false,
            },
            activation_token_data: None,
        }
    }

    pub fn needs_initial_configure(&self) -> bool {
        matches!(self.state, InitialConfigureState::NotConfigured { .. })
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.window.toplevel().expect("no X11 support")
    }
}
