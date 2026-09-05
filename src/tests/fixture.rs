use std::os::fd::AsFd as _;
use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;
use std::time::Duration;

use calloop::generic::Generic;
use calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use zen_config::Config;
use smithay::output::Output;

use super::client::{Client, ClientId};
use super::server::Server;
use crate::state::{NewClient, Zen};

pub struct Fixture {
    pub event_loop: EventLoop<'static, State>,
    pub handle: LoopHandle<'static, State>,
    pub state: State,
}

pub struct State {
    pub server: Server,
    pub clients: Vec<Client>,
}

impl Fixture {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_config(mut config: Config) -> Self {
        config.camera.open_on_canvas = false;
        let event_loop = EventLoop::try_new().unwrap();
        let handle = event_loop.handle();

        let server = Server::new(config);
        let fd = server.event_loop.as_fd().try_clone_to_owned().unwrap();
        let source = Generic::new(fd, Interest::READ, Mode::Level);
        handle
            .insert_source(source, |_, _, state: &mut State| {
                state.server.dispatch();
                Ok(PostAction::Continue)
            })
            .unwrap();

        let state = State {
            server,
            clients: Vec::new(),
        };

        Self {
            event_loop,
            handle,
            state,
        }
    }

    pub fn dispatch(&mut self) {
        self.event_loop
            .dispatch(Duration::ZERO, &mut self.state)
            .unwrap();
    }

    pub fn zen_state(&mut self) -> &mut crate::state::State {
        &mut self.state.server.state
    }

    pub fn zen(&mut self) -> &mut Zen {
        &mut self.zen_state().zen
    }

    pub fn zen_output(&self, n: u8) -> Output {
        let zen = &self.state.server.state.zen;
        let idx = usize::from(n - 1);
        let output = zen.global_space.outputs().nth(idx).unwrap();
        output.clone()
    }

    pub fn zen_focus_output(&mut self, n: u8) {
        let zen = &mut self.state.server.state.zen;
        let idx = usize::from(n - 1);
        let output = zen.global_space.outputs().nth(idx).unwrap();
        zen.layout.focus_output(output);
    }

    pub fn zen_complete_animations(&mut self) {
        let zen = self.zen();
        zen.clock.set_complete_instantly(true);
        zen.advance_animations();
        zen.clock.set_complete_instantly(false);
    }

    pub fn add_output(&mut self, n: u8, size: (u16, u16)) {
        let state = self.zen_state();
        let zen = &mut state.zen;
        state.backend.headless().add_output(zen, n, size);
    }

    pub fn add_client(&mut self) -> ClientId {
        let (sock1, sock2) = UnixStream::pair().unwrap();
        self.zen().insert_client(NewClient {
            client: sock1,
            restricted: false,
            credentials_unknown: false,
        });

        let client = Client::new(sock2);
        let id = client.id;

        let fd = client.event_loop.as_fd().try_clone_to_owned().unwrap();
        let source = Generic::new(fd, Interest::READ, Mode::Level);
        self.handle
            .insert_source(source, move |_, _, state: &mut State| {
                state.client(id).dispatch();
                Ok(PostAction::Continue)
            })
            .unwrap();

        self.state.clients.push(client);
        self.roundtrip(id);
        id
    }

    pub fn client(&mut self, id: ClientId) -> &mut Client {
        self.state.client(id)
    }

    pub fn roundtrip(&mut self, id: ClientId) {
        let client = self.state.client(id);
        let data = client.send_sync();
        while !data.done.load(Ordering::Relaxed) {
            self.dispatch();
        }
    }

    pub fn double_roundtrip(&mut self, id: ClientId) {
        self.roundtrip(id);
        self.roundtrip(id);
    }
}

impl State {
    pub fn client(&mut self, id: ClientId) -> &mut Client {
        self.clients.iter_mut().find(|c| c.id == id).unwrap()
    }
}
