//! The native host's JSON-lines protocol, serviced by the window thread.
use super::window::Event;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use winit::event_loop::EventLoopProxy;

pub struct Agent {
    path: Option<PathBuf>,
}
impl Agent {
    pub fn start(proxy: EventLoopProxy<Event>, ready: String) -> Result<Self, String> {
        let path = std::env::var_os("EXACT_AGENT_SOCKET").map(PathBuf::from);
        let listener = path
            .as_ref()
            .map(UnixListener::bind)
            .transpose()
            .map_err(|e| e.to_string())?;
        std::thread::Builder::new()
            .name("painted-agent".into())
            .spawn(move || {
                let result = if let Some(listener) = listener {
                    listener.accept().and_then(|(socket, _)| {
                        let writer = socket.try_clone()?;
                        serve(BufReader::new(socket), writer, &proxy, &ready)
                    })
                } else {
                    serve(
                        BufReader::new(std::io::stdin()),
                        std::io::stdout(),
                        &proxy,
                        &ready,
                    )
                };
                if let Err(error) = result {
                    eprintln!("painted agent: {error}");
                }
                let _ = proxy.send_event(Event::Closed);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { path })
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}
fn serve(
    mut reader: impl BufRead,
    mut writer: impl Write,
    proxy: &EventLoopProxy<Event>,
    ready: &str,
) -> std::io::Result<()> {
    writeln!(writer, "{ready}")?;
    writer.flush()?;
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        let (reply, receive) = std::sync::mpsc::sync_channel(1);
        if proxy
            .send_event(Event::Request(std::mem::take(&mut line), reply))
            .is_err()
        {
            break;
        }
        let Ok(answer) = receive.recv() else {
            break;
        };
        writeln!(writer, "{answer}")?;
        writer.flush()?;
    }
    Ok(())
}
