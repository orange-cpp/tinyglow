#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod scene;
#[cfg(windows)]
mod window;

#[cfg(windows)]
fn main() {
    if let Err(error) = window::run() {
        window::report_error(&error.to_string());
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The Direct3D 9 preview requires Windows.");
}
