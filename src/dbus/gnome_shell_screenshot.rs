use std::collections::HashMap;
use std::path::PathBuf;

use zen_ipc::PickedColor;
use zbus::fdo::{self, RequestNameFlags};
use zbus::zvariant::OwnedValue;
use zbus::{interface, zvariant};

use super::Start;

pub struct Screenshot {
    to_zen: calloop::channel::Sender<ScreenshotToZen>,
    from_zen: async_channel::Receiver<ZenToScreenshot>,
}

pub enum ScreenshotToZen {
    TakeScreenshot { include_cursor: bool },
    PickColor(async_channel::Sender<Option<PickedColor>>),
}

pub enum ZenToScreenshot {
    ScreenshotResult(Option<PathBuf>),
}

#[interface(name = "org.gnome.Shell.Screenshot")]
impl Screenshot {
    async fn screenshot(
        &self,
        include_cursor: bool,
        _flash: bool,
        _filename: PathBuf,
    ) -> fdo::Result<(bool, PathBuf)> {
        if let Err(err) = self
            .to_zen
            .send(ScreenshotToZen::TakeScreenshot { include_cursor })
        {
            warn!("error sending message to ZEN: {err:?}");
            return Err(fdo::Error::Failed("internal error".to_owned()));
        }

        let filename = match self.from_zen.recv().await {
            Ok(ZenToScreenshot::ScreenshotResult(Some(filename))) => filename,
            Ok(ZenToScreenshot::ScreenshotResult(None)) => {
                return Err(fdo::Error::Failed("internal error".to_owned()));
            }
            Err(err) => {
                warn!("error receiving message from ZEN: {err:?}");
                return Err(fdo::Error::Failed("internal error".to_owned()));
            }
        };

        Ok((true, filename))
    }

    async fn pick_color(&self) -> fdo::Result<HashMap<String, OwnedValue>> {
        let (tx, rx) = async_channel::bounded(1);
        if let Err(err) = self.to_zen.send(ScreenshotToZen::PickColor(tx)) {
            warn!("error sending pick color message to ZEN: {err:?}");
            return Err(fdo::Error::Failed("internal error".to_owned()));
        }

        let color = match rx.recv().await {
            Ok(Some(color)) => color,
            Ok(None) => {
                return Err(fdo::Error::Failed("no color picked".to_owned()));
            }
            Err(err) => {
                warn!("error receiving message from ZEN: {err:?}");
                return Err(fdo::Error::Failed("internal error".to_owned()));
            }
        };

        let mut result = HashMap::new();
        let [r, g, b] = color.rgb;
        result.insert(
            "color".to_string(),
            zvariant::OwnedValue::try_from(zvariant::Value::from((r, g, b))).unwrap(),
        );

        Ok(result)
    }
}

impl Screenshot {
    pub fn new(
        to_zen: calloop::channel::Sender<ScreenshotToZen>,
        from_zen: async_channel::Receiver<ZenToScreenshot>,
    ) -> Self {
        Self { to_zen, from_zen }
    }
}

impl Start for Screenshot {
    fn start(self) -> anyhow::Result<zbus::blocking::Connection> {
        let conn = zbus::blocking::Connection::session()?;
        let flags = RequestNameFlags::AllowReplacement
            | RequestNameFlags::ReplaceExisting
            | RequestNameFlags::DoNotQueue;

        conn.object_server()
            .at("/org/gnome/Shell/Screenshot", self)?;
        conn.request_name_with_flags("org.gnome.Shell.Screenshot", flags)?;

        Ok(conn)
    }
}
