use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::{Event, Reply, Request};

pub const SOCKET_PATH_ENV: &str = "ZEN_SOCKET";

pub const COMPAT_SOCKET_PATH_ENV: &str = "NIRI_SOCKET";

pub struct Socket {
    stream: BufReader<UnixStream>,
}

impl Socket {
    pub fn connect() -> io::Result<Self> {
        let socket_path = env::var_os(SOCKET_PATH_ENV).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{SOCKET_PATH_ENV} is not set, are you running this within ZEN?"),
            )
        })?;
        Self::connect_to(socket_path)
    }

    pub fn connect_to(path: impl AsRef<Path>) -> io::Result<Self> {
        let stream = UnixStream::connect(path.as_ref())?;
        let stream = BufReader::new(stream);
        Ok(Self { stream })
    }

    pub fn send(&mut self, request: Request) -> io::Result<Reply> {
        let mut buf = serde_json::to_string(&request).unwrap();
        buf.push('\n');
        self.stream.get_mut().write_all(buf.as_bytes())?;

        buf.clear();
        self.stream.read_line(&mut buf)?;

        let reply = serde_json::from_str(&buf)?;
        Ok(reply)
    }

    pub fn read_events(self) -> impl FnMut() -> io::Result<Event> {
        let Self { mut stream } = self;
        let _ = stream.get_mut().shutdown(Shutdown::Write);

        let mut buf = String::new();
        move || {
            buf.clear();
            stream.read_line(&mut buf)?;
            let event = serde_json::from_str(&buf)?;
            Ok(event)
        }
    }
}
