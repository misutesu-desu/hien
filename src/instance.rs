//! Keeps Hien to one window. A later launch hands its file to the running
//! window over a loopback socket and exits, so it never pays for a second
//! OpenGL start.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

const HELLO: &str = "hien-open-v1";

/// Requests from later launches. `None` asks only to bring the window forward.
pub type Inbox = Arc<Mutex<Vec<Option<PathBuf>>>>;

fn port_file() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Hien").join("instance"))
}

/// Passes `file` to a running Hien. Returns `true` when one took it.
pub fn send(file: Option<&Path>) -> bool {
    // Connecting to a closed loopback port takes about two seconds on Windows,
    // so the leftover port file of a closed Hien is only tried when one is running.
    if !already_running() {
        return false;
    }
    port_file()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse::<u16>().ok())
        .is_some_and(|port| send_to(port, file))
}

fn send_to(port: u16, file: Option<&Path>) -> bool {
    let line = match file.map(std::path::absolute) {
        None => String::new(),
        Some(Ok(p)) => match p.to_str() {
            Some(s) => s.to_owned(),
            None => return false,
        },
        Some(Err(_)) => return false,
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(200)) else {
        return false;
    };
    allow_focus_steal();
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1000)));
    if write!(stream, "{HELLO}\n{line}\n").is_err() {
        return false;
    }
    // The port may now belong to some other program, so only a proper answer counts.
    let mut answer = [0; 2];
    stream.read_exact(&mut answer).is_ok() && &answer == b"ok"
}

/// Starts answering later launches. Returns `None` if the socket could not be opened,
/// in which case every launch simply gets its own window.
/// `window` is the native handle of Hien's window, used to show it straight away.
pub fn listen(ctx: egui::Context, window: Option<isize>) -> Option<Inbox> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let path = port_file()?;
    fs::create_dir_all(path.parent()?).ok()?;
    fs::write(path, port.to_string()).ok()?;
    Some(serve(listener, ctx, window))
}

fn serve(listener: TcpListener, ctx: egui::Context, window: Option<isize>) -> Inbox {
    let inbox = Inbox::default();
    let sink = Arc::clone(&inbox);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if let Some(request) = read_request(&stream) {
                sink.lock().unwrap().push(request);
                if let Some(hwnd) = window {
                    raise(hwnd);
                }
                ctx.request_repaint();
                let _ = (&stream).write_all(b"ok");
            }
        }
    });
    inbox
}

fn read_request(stream: &TcpStream) -> Option<Option<PathBuf>> {
    stream
        .set_read_timeout(Some(Duration::from_millis(1000)))
        .ok()?;
    let mut lines = BufReader::new(stream).lines();
    if lines.next()?.ok()? != HELLO {
        return None;
    }
    let path = lines.next()?.ok()?;
    Some((!path.is_empty()).then(|| PathBuf::from(path)))
}

/// The first Hien creates a named mutex and holds it until it exits, so a later
/// launch sees it already exists. Windows drops it even if Hien crashes.
#[cfg(windows)]
pub fn already_running() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = r"Local\Hien.instance".encode_utf16().chain([0]).collect();
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        let exists = GetLastError() == ERROR_ALREADY_EXISTS;
        if exists {
            CloseHandle(handle);
        }
        exists
    }
}

#[cfg(not(windows))]
pub fn already_running() -> bool {
    true
}

// Windows lets only the foreground process raise a window. The launch that came
// from the user's double-click has that right and passes it on before exiting.
#[cfg(windows)]
fn allow_focus_steal() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
}

#[cfg(not(windows))]
fn allow_focus_steal() {}

// eframe repaints a hidden window at most every 100 ms, so asking it to show the
// window through egui leaves the user waiting. Showing it here makes Windows send
// a paint right away, and that paint opens the file.
#[cfg(windows)]
fn raise(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindowAsync,
    };
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        ShowWindowAsync(
            hwnd,
            if IsIconic(hwnd) != 0 {
                SW_RESTORE
            } else {
                SW_SHOW
            },
        );
        SetForegroundWindow(hwnd);
    }
}

#[cfg(not(windows))]
fn raise(_hwnd: isize) {}

#[cfg(windows)]
pub fn window_of(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

#[cfg(not(windows))]
pub fn window_of(_cc: &eframe::CreationContext<'_>) -> Option<isize> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_window_receives_the_file() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let inbox = serve(listener, egui::Context::default(), None);
        let file = std::env::temp_dir().join("not defteri ğ.txt");

        assert!(send_to(port, Some(&file)));
        assert!(send_to(port, None));
        assert_eq!(*inbox.lock().unwrap(), [Some(file), None]);
    }

    #[test]
    fn unrelated_listener_is_not_trusted() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let _ = s.write_all(
                    b"HTTP/1.1 400
",
                );
            }
        });
        assert!(!send_to(port, None));
    }
}
