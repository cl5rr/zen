pub use platform::*;

#[cfg(not(target_os = "linux"))]
mod platform {
    use std::io;

    pub fn listen(_handle: &calloop::LoopHandle<crate::state::State>) {}

    pub fn block_early() -> io::Result<()> {
        Ok(())
    }
    pub fn unblock_all() -> io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{io, mem};

    pub fn listen(handle: &calloop::LoopHandle<crate::state::State>) {
        use calloop::signals::{Signal, Signals};

        handle
            .insert_source(
                Signals::new(&[Signal::SIGINT, Signal::SIGTERM, Signal::SIGHUP]).unwrap(),
                |event, _, state| {
                    info!("quitting due to receiving signal {:?}", event.signal());
                    state.zen.stop_signal.stop();
                },
            )
            .unwrap();
    }

    pub fn block_early() -> io::Result<()> {
        set_sigmask(&preferred_sigset()?)
    }

    pub fn unblock_all() -> io::Result<()> {
        set_sigmask(&empty_sigset()?)
    }

    fn empty_sigset() -> io::Result<libc::sigset_t> {
        let mut sigset = mem::MaybeUninit::uninit();
        if unsafe { libc::sigemptyset(sigset.as_mut_ptr()) } == 0 {
            Ok(unsafe { sigset.assume_init() })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn preferred_sigset() -> io::Result<libc::sigset_t> {
        let mut set = empty_sigset()?;
        unsafe {
            add_signal(&mut set, libc::SIGINT)?;
            add_signal(&mut set, libc::SIGTERM)?;
            add_signal(&mut set, libc::SIGHUP)?;
        }
        Ok(set)
    }

    unsafe fn add_signal(set: &mut libc::sigset_t, signum: libc::c_int) -> io::Result<()> {
        if unsafe { libc::sigaddset(set, signum) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn set_sigmask(set: &libc::sigset_t) -> io::Result<()> {
        let oldset = std::ptr::null_mut();
        if unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, set, oldset) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
