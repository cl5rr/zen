use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::Context;
use futures_util::StreamExt;
use smithay::backend::input::{KeyState, Keycode};
use smithay::input::keyboard::{xkb, Keysym};
use zbus::blocking::object_server::InterfaceRef;
use zbus::fdo::{self, RequestNameFlags};
use zbus::message::Header;
use zbus::names::{BusName, OwnedUniqueName, UniqueName};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{NoneValue, OwnedObjectPath, SerializeDict, Type, Value};
use zbus::{interface, DBusError};

use crate::state::{PointContents, State};
use crate::utils::get_credentials_for_surface;

pub struct Manager {
    pub keyboard_monitor: KeyboardMonitor,
    pub pointer_locator: PointerLocator,
}

pub enum A11yManagerToZen {
    QueryPointer,
}

pub enum ZenToA11yManager {
    PointerContents(Option<(PointerAppData, f64, f64)>),
}

#[derive(Debug, Default)]
struct KeyboardData {
    clients: HashMap<OwnedUniqueName, KeyboardClient>,

    grabbed_mods: HashSet<Keysym>,
    grabbed_mod_last_press_time: HashMap<Keysym, Duration>,
    suppressed_keys: HashSet<Keysym>,
}

#[derive(Debug, Default)]
struct KeyboardClient {
    watched: bool,
    grabbed: bool,
    modifiers: HashSet<Keysym>,
    keystrokes: Vec<(Keysym, u32)>,
}

#[derive(Clone, Default)]
pub struct KeyboardMonitor {
    data: Arc<Mutex<KeyboardData>>,
    iface: Arc<OnceLock<InterfaceRef<Self>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KbMonBlock {
    Pass,
    ModifierFirstPress,
    Block,
}

#[derive(Debug, Default)]
struct PointerData {
    clients: HashSet<OwnedUniqueName>,
}

#[derive(Debug, Default, SerializeDict, Type, Value)]
#[zvariant(signature = "dict")]
pub struct PointerAppData {
    pid: Option<i32>,
    app_dbus_name: Option<String>,
    toplevel_object_path: Option<OwnedObjectPath>,
}

#[derive(Clone)]
pub struct PointerLocator {
    data: Arc<Mutex<PointerData>>,
    iface: Arc<OnceLock<InterfaceRef<Self>>>,
    to_zen: calloop::channel::Sender<A11yManagerToZen>,
    from_zen: async_channel::Receiver<ZenToA11yManager>,
}

#[derive(DBusError, Debug)]
#[zbus(prefix = "org.freedesktop.a11y")]
enum A11yError {
    #[zbus(error)]
    ZBus(zbus::Error),
    UnknownToplevel,
    Failed(String),
}

#[interface(name = "org.freedesktop.a11y.KeyboardMonitor")]
impl KeyboardMonitor {
    async fn grab_keyboard(&self, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        let Some(sender) = hdr.sender() else {
            return Err(fdo::Error::Failed("no sender".to_owned()));
        };
        let sender = OwnedUniqueName::from(sender.to_owned());
        trace!("enabling keyboard grab for {sender}");

        let mut data = self.data.lock().unwrap();
        let client = data.clients.entry(sender).or_default();
        client.grabbed = true;

        Ok(())
    }

    async fn ungrab_keyboard(&self, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        let Some(sender) = hdr.sender() else {
            return Err(fdo::Error::Failed("no sender".to_owned()));
        };
        let sender = OwnedUniqueName::from(sender.to_owned());

        let mut data = self.data.lock().unwrap();
        if let Some(client) = data.clients.get_mut(&sender) {
            trace!("disabling keyboard grab for {sender}");
            client.grabbed = false;
        }

        Ok(())
    }

    async fn watch_keyboard(&self, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        let Some(sender) = hdr.sender() else {
            return Err(fdo::Error::Failed("no sender".to_owned()));
        };
        let sender = OwnedUniqueName::from(sender.to_owned());
        trace!("enabling keyboard watch for {sender}");

        let mut data = self.data.lock().unwrap();
        let client = data.clients.entry(sender).or_default();
        client.watched = true;

        Ok(())
    }

    async fn unwatch_keyboard(&self, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        let Some(sender) = hdr.sender() else {
            return Err(fdo::Error::Failed("no sender".to_owned()));
        };
        let sender = OwnedUniqueName::from(sender.to_owned());

        let mut data = self.data.lock().unwrap();
        if let Some(client) = data.clients.get_mut(&sender) {
            trace!("disabling keyboard watch for {sender}");
            client.watched = false;
        }

        Ok(())
    }

    async fn set_key_grabs(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        modifiers: Vec<u32>,
        keystrokes: Vec<(u32, u32)>,
    ) -> fdo::Result<()> {
        let Some(sender) = hdr.sender() else {
            return Err(fdo::Error::Failed("no sender".to_owned()));
        };
        let sender = OwnedUniqueName::from(sender.to_owned());
        trace!("updating key grabs for {sender}");

        let mut data = self.data.lock().unwrap();
        let client = data.clients.entry(sender).or_default();
        client.modifiers = HashSet::from_iter(modifiers.into_iter().map(Keysym::new));
        client.keystrokes =
            Vec::from_iter(keystrokes.into_iter().map(|(k, v)| (Keysym::new(k), v)));

        data.rebuild_grabbed_mods();

        Ok(())
    }

    #[zbus(signal)]
    pub async fn key_event(
        ctxt: &SignalEmitter<'_>,
        released: bool,
        state: u32,
        keysym: u32,
        unichar: u32,
        keycode: u16,
    ) -> zbus::Result<()>;
}

impl KeyboardMonitor {
    #[allow(clippy::too_many_arguments)]
    pub fn process_key(
        &self,
        repeat_delay: Duration,
        time: Duration,
        keycode: Keycode,
        released: bool,
        mods: u32,
        keysym: Keysym,
        unichar: u32,
    ) -> KbMonBlock {
        let _span = tracy_client::span!("KeyboardMonitor::process_key");

        let mut ctxt = self.iface.get().unwrap().signal_emitter().clone();

        let mut data = self.data.lock().unwrap();

        for (name, client) in &data.clients {
            if client.should_watch_keypress(&data.suppressed_keys, mods, keysym) {
                let _span = tracy_client::span!("emitting key event");

                ctxt = ctxt.set_destination(BusName::Unique(name.as_ref()));
                let ctxt = &ctxt;
                async_io::block_on(async move {
                    if let Err(err) = KeyboardMonitor::key_event(
                        ctxt,
                        released,
                        mods,
                        keysym.raw(),
                        unichar,
                        keycode.raw() as u16,
                    )
                    .await
                    {
                        warn!("error emitting key_event: {err:?}");
                    }
                });
            }
        }

        if data.grabbed_mods.contains(&keysym) {
            if released {
                if !data.suppressed_keys.contains(&keysym) {
                    trace!("handling release for second press of grabbed modifier: {keysym:?}");
                    return KbMonBlock::Pass;
                }
            } else {
                let last_press_entry = data
                    .grabbed_mod_last_press_time
                    .entry(keysym)
                    .or_insert(Duration::ZERO);
                let last_press = *last_press_entry;
                *last_press_entry = time;

                if time <= last_press.saturating_add(repeat_delay) {
                    trace!("handling second press of grabbed modifier: {keysym:?}");
                    return KbMonBlock::Pass;
                }
            }
        }

        let mut block = false;

        if released {
            if data.suppressed_keys.remove(&keysym) {
                trace!("blocking release for previously suppressed key: {keysym:?}");
                block = true;
            }
        } else if data.suppressed_keys.contains(&keysym) {
            trace!("blocking press for already-pressed key: {keysym:?}");
            block = true;
        } else {
            if data
                .clients
                .values()
                .any(|client| client.should_grab_keypress(&data.suppressed_keys, mods, keysym))
            {
                trace!("blocking press for grabbed key: {keysym:?}");
                data.suppressed_keys.insert(keysym);
                block = true;
            }
        }

        if !block {
            KbMonBlock::Pass
        } else if data.grabbed_mods.contains(&keysym) {
            KbMonBlock::ModifierFirstPress
        } else {
            KbMonBlock::Block
        }
    }
}

impl KeyboardData {
    fn rebuild_grabbed_mods(&mut self) {
        self.grabbed_mods.clear();
        for client in self.clients.values() {
            self.grabbed_mods.extend(&client.modifiers);
        }
    }
}

impl KeyboardClient {
    fn should_grab_keypress(
        &self,
        suppressed_keys: &HashSet<Keysym>,
        mods: u32,
        keysym: Keysym,
    ) -> bool {
        if self.grabbed {
            return true;
        }

        for modifier in &self.modifiers {
            if *modifier == keysym || suppressed_keys.contains(modifier) {
                return true;
            }
        }

        for (grabbed_keysym, grabbed_mods) in &self.keystrokes {
            if *grabbed_keysym == keysym && *grabbed_mods == mods {
                return true;
            }
        }

        false
    }

    fn should_watch_keypress(
        &self,
        suppressed_keys: &HashSet<Keysym>,
        mods: u32,
        keysym: Keysym,
    ) -> bool {
        if self.watched {
            return true;
        }

        self.should_grab_keypress(suppressed_keys, mods, keysym)
    }
}

#[interface(name = "org.freedesktop.a11y.PointerLocator")]
impl PointerLocator {
    async fn query_pointer(
        &self,
        #[zbus(header)] hdr: Header<'_>,
    ) -> Result<(PointerAppData, f64, f64), A11yError> {
        let Some(sender) = hdr.sender() else {
            return Err(A11yError::Failed("no sender".to_owned()));
        };
        let sender = OwnedUniqueName::from(sender.to_owned());

        if let Err(err) = self.to_zen.send(A11yManagerToZen::QueryPointer) {
            warn!("error sending message to ZEN: {err:?}");
            return Err(A11yError::Failed("internal error".to_owned()));
        }

        let rv = match self.from_zen.recv().await {
            Ok(ZenToA11yManager::PointerContents(Some((data, x, y)))) => Ok((data, x, y)),
            Ok(ZenToA11yManager::PointerContents(None)) => Err(A11yError::UnknownToplevel),
            Err(err) => {
                warn!("error receiving message from ZEN: {err:?}");
                Err(A11yError::Failed("internal error".to_owned()))
            }
        };

        let mut data = self.data.lock().unwrap();
        if !data.clients.contains(&sender) {
            trace!("enabling pointer position notifications for {sender}");
            data.clients.insert(sender);
        }

        rv
    }

    #[zbus(signal)]
    pub async fn pointer_position_changed(ctxt: &SignalEmitter<'_>) -> zbus::Result<()>;
}

impl PointerLocator {
    fn notify_pointer_position_changed(&self) {
        let mut ctxt = self.iface.get().unwrap().signal_emitter().clone();

        let mut data = self.data.lock().unwrap();
        for name in &data.clients {
            let _span = tracy_client::span!("emitting pointer_position_changed");

            ctxt = ctxt.set_destination(BusName::Unique(name.as_ref()));
            let ctxt = &ctxt;
            async_io::block_on(async move {
                if let Err(err) = PointerLocator::pointer_position_changed(ctxt).await {
                    warn!("error emitting pointer_position_changed: {err:?}");
                }
            });
        }

        data.clients.clear();
    }
}

async fn monitor_disappeared_clients(
    conn: &zbus::Connection,
    kb_data: Arc<Mutex<KeyboardData>>,
    pointer_data: Arc<Mutex<PointerData>>,
) -> anyhow::Result<()> {
    let proxy = fdo::DBusProxy::new(conn)
        .await
        .context("error creating a DBusProxy")?;

    let mut stream = proxy
        .receive_name_owner_changed_with_args(&[(2, UniqueName::null_value())])
        .await
        .context("error creating a NameOwnerChanged stream")?;

    while let Some(signal) = stream.next().await {
        let args = signal
            .args()
            .context("error retrieving NameOwnerChanged args")?;

        let Some(name) = &**args.old_owner() else {
            continue;
        };

        if args.new_owner().is_none() {
            trace!("keyboard monitor client disconnected: {name}");

            let name = OwnedUniqueName::from(name.to_owned());
            {
                let mut data = kb_data.lock().unwrap();
                data.clients.remove(&name);
                data.rebuild_grabbed_mods();
            }
            {
                let mut data = pointer_data.lock().unwrap();
                data.clients.remove(&name);
            }
        } else {
            error!("non-null new_owner should've been filtered out");
        }
    }

    Ok(())
}

impl Manager {
    pub fn new(
        to_zen: calloop::channel::Sender<A11yManagerToZen>,
        from_zen: async_channel::Receiver<ZenToA11yManager>,
    ) -> Self {
        Self {
            keyboard_monitor: KeyboardMonitor::default(),
            pointer_locator: PointerLocator {
                data: Default::default(),
                iface: Default::default(),
                to_zen,
                from_zen,
            },
        }
    }

    pub fn start(&self) -> anyhow::Result<zbus::blocking::Connection> {
        let conn = zbus::blocking::Connection::session()?;
        let flags = RequestNameFlags::AllowReplacement
            | RequestNameFlags::ReplaceExisting
            | RequestNameFlags::DoNotQueue;

        conn.object_server().at(
            "/org/freedesktop/a11y/Manager",
            self.keyboard_monitor.clone(),
        )?;
        conn.object_server().at(
            "/org/freedesktop/a11y/Manager",
            self.pointer_locator.clone(),
        )?;
        conn.request_name_with_flags("org.freedesktop.a11y.Manager", flags)?;

        let iface = conn
            .object_server()
            .interface("/org/freedesktop/a11y/Manager")?;
        let _ = self.keyboard_monitor.iface.set(iface);

        let iface = conn
            .object_server()
            .interface("/org/freedesktop/a11y/Manager")?;
        let _ = self.pointer_locator.iface.set(iface);

        let kb_data = self.keyboard_monitor.data.clone();
        let pointer_data = self.pointer_locator.data.clone();

        let async_conn = conn.inner().clone();
        let future = async move {
            if let Err(err) =
                monitor_disappeared_clients(&async_conn, kb_data.clone(), pointer_data.clone())
                    .await
            {
                warn!("error monitoring keyboard monitor clients: {err:?}");

                if let Err(err) = async_conn.close().await {
                    warn!("error closing connection: {err:?}");
                }

                {
                    let mut data = kb_data.lock().unwrap();
                    data.clients.clear();
                    data.rebuild_grabbed_mods();
                }
                {
                    let mut data = pointer_data.lock().unwrap();
                    data.clients.clear();
                }
            }
        };
        let task = conn
            .inner()
            .executor()
            .spawn(future, "monitor disappearing keyboard clients");
        task.detach();

        Ok(conn)
    }
}

impl State {
    pub fn a11y_process_key(
        &mut self,
        time: Duration,
        keycode: Keycode,
        state: KeyState,
    ) -> KbMonBlock {
        if self.zen.a11y_manager.is_none() {
            return KbMonBlock::Pass;
        }

        let keyboard = self.zen.seat.get_keyboard().unwrap();

        let (mods, keysym, unichar) = keyboard.with_xkb_state(self, |context| {
            let xkb = context.xkb().lock().unwrap();
            let state = unsafe { xkb.state() };

            let keysym = state.key_get_one_sym(keycode);
            let mods = state.serialize_mods(xkb::STATE_MODS_EFFECTIVE);
            let unichar = state.key_get_utf32(keycode);

            (mods, keysym, unichar)
        });

        let config = self.zen.config.borrow();
        let repeat_delay = Duration::from_millis(u64::from(config.input.keyboard.repeat_delay));
        let released = state == KeyState::Released;

        let Some(manager) = &self.zen.a11y_manager else {
            return KbMonBlock::Pass;
        };
        let monitor = &manager.keyboard_monitor;
        monitor.process_key(repeat_delay, time, keycode, released, mods, keysym, unichar)
    }

    pub fn a11y_notify_pointer_motion(&mut self) {
        let Some(manager) = &self.zen.a11y_manager else {
            return;
        };

        manager.pointer_locator.notify_pointer_position_changed();
    }

    pub fn on_a11y_manager_msg(
        &mut self,
        to_a11y: &async_channel::Sender<ZenToA11yManager>,
        msg: A11yManagerToZen,
    ) {
        let A11yManagerToZen::QueryPointer = msg;
        let _span = tracy_client::span!("QueryPointer");

        let pointer = &self.zen.seat.get_pointer().unwrap();
        let pointer_pos = pointer.current_location();

        let contents = match &self.zen.pointer_contents {
            PointContents {
                surface: Some((surface, surface_pos)),
                ..
            } => {
                if let Some(credentials) = get_credentials_for_surface(surface) {
                    let pos_within_surface = pointer_pos - *surface_pos;

                    let data = PointerAppData {
                        pid: Some(credentials.pid),
                        app_dbus_name: None,
                        toplevel_object_path: None,
                    };

                    Some((data, pos_within_surface.x, pos_within_surface.y))
                } else {
                    None
                }
            }
            _ => {
                Some((PointerAppData::default(), pointer_pos.x, pointer_pos.y))
            }
        };

        let msg = ZenToA11yManager::PointerContents(contents);
        if let Err(err) = to_a11y.send_blocking(msg) {
            warn!("error sending pointer contents to a11y manager: {err:?}");
        }
    }
}
