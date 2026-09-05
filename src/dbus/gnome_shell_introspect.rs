use std::collections::HashMap;

use zbus::fdo::{self, RequestNameFlags};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{SerializeDict, Type, Value};

use super::Start;

pub struct Introspect {
    to_zen: calloop::channel::Sender<IntrospectToZen>,
    from_zen: async_channel::Receiver<ZenToIntrospect>,
}

pub enum IntrospectToZen {
    GetWindows,
}

pub enum ZenToIntrospect {
    Windows(HashMap<u64, WindowProperties>),
}

#[derive(Debug, SerializeDict, Type, Value)]
#[zvariant(signature = "dict")]
pub struct WindowProperties {
    /// Window title.
    pub title: String,
    /// Window app ID.
    ///
    /// This is actually the name of the .desktop file, and Shell does internal tracking to match
    /// Wayland app IDs to desktop files. We don't do that yet, which is the reason why
    /// xdg-desktop-portal-gnome's window list is missing icons.
    #[zvariant(rename = "app-id")]
    pub app_id: String,
}

#[interface(name = "org.gnome.Shell.Introspect")]
impl Introspect {
    async fn get_windows(&self) -> fdo::Result<HashMap<u64, WindowProperties>> {
        if let Err(err) = self.to_zen.send(IntrospectToZen::GetWindows) {
            warn!("error sending message to ZEN: {err:?}");
            return Err(fdo::Error::Failed("internal error".to_owned()));
        }

        match self.from_zen.recv().await {
            Ok(ZenToIntrospect::Windows(windows)) => Ok(windows),
            Err(err) => {
                warn!("error receiving message from ZEN: {err:?}");
                Err(fdo::Error::Failed("internal error".to_owned()))
            }
        }
    }

    // FIXME: call this upon window changes, once more of the infrastructure is there (will be
    // needed for the event stream IPC anyway).
    #[zbus(signal)]
    pub async fn windows_changed(ctxt: &SignalEmitter<'_>) -> zbus::Result<()>;
}

impl Introspect {
    pub fn new(
        to_zen: calloop::channel::Sender<IntrospectToZen>,
        from_zen: async_channel::Receiver<ZenToIntrospect>,
    ) -> Self {
        Self { to_zen, from_zen }
    }
}

impl Start for Introspect {
    fn start(self) -> anyhow::Result<zbus::blocking::Connection> {
        let conn = zbus::blocking::Connection::session()?;
        let flags = RequestNameFlags::AllowReplacement
            | RequestNameFlags::ReplaceExisting
            | RequestNameFlags::DoNotQueue;

        conn.object_server()
            .at("/org/gnome/Shell/Introspect", self)?;
        conn.request_name_with_flags("org.gnome.Shell.Introspect", flags)?;

        Ok(conn)
    }
}
