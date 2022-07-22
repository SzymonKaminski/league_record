use std::{
    cmp::Ordering,
    io,
    path::{Path, PathBuf},
};

use reqwest::{blocking::Client, redirect::Policy, StatusCode};
use tauri::{
    api::version::compare, AppHandle, CustomMenuItem, Manager, SystemTrayMenu, SystemTrayMenuItem,
    Window,
};

use crate::state::WindowState;

pub fn create_tray_menu() -> SystemTrayMenu {
    SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("rec", "Recording").disabled())
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("open", "Open"))
        .add_item(CustomMenuItem::new("quit", "Quit"))
}

pub fn check_updates(app_handle: &AppHandle) {
    let config = app_handle.config();
    let version = config.package.version.as_ref().unwrap();

    let client = Client::builder()
        .redirect(Policy::none())
        .build()
        .expect("couldn't create http client");
    let result = client
        .get("https://github.com/FFFFFFFXXXXXXX/league_record/releases/latest")
        .send()
        .expect("couldn't GET http result");

    if result.status() == StatusCode::FOUND {
        let url = result.headers().get("location").unwrap();
        if let Ok(url) = url.to_str() {
            let new_version = url.rsplit_once("/v").unwrap().1;
            if let Ok(res) = compare(&version, new_version) {
                if res == 1 {
                    let tray_menu = create_tray_menu()
                        .add_native_item(SystemTrayMenuItem::Separator)
                        .add_item(CustomMenuItem::new("update", "Update Available!"));
                    let _ = app_handle.tray_handle().set_menu(tray_menu);
                }
            }
        }
    }
}

pub fn get_recordings(rec_folder: &Path) -> Vec<PathBuf> {
    // get all mp4 files in ~/Videos/%folder-name%
    let mut recordings = Vec::<PathBuf>::new();
    let rd_dir = match rec_folder.read_dir() {
        Ok(rd_dir) => rd_dir,
        Err(_) => return vec![],
    };
    for entry in rd_dir.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "mp4" {
                recordings.push(path);
            }
        }
    }
    recordings
}

pub fn compare_time(a: &Path, b: &Path) -> io::Result<Ordering> {
    let a_time = a.metadata()?.created()?;
    let b_time = b.metadata()?.created()?;
    Ok(a_time.cmp(&b_time).reverse())
}

pub fn show_window(window: &Window) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

pub fn create_window(app_handle: &AppHandle) {
    if let Some(main) = app_handle.windows().get("main") {
        show_window(main);
    } else {
        let window_state = app_handle.state::<WindowState>();

        let builder = tauri::Window::builder(
            app_handle,
            "main",
            tauri::WindowUrl::App(PathBuf::from("/")),
        );

        let size = *window_state.size.lock().unwrap();
        let position = *window_state.position.lock().unwrap();
        builder
            .title("LeagueRecord")
            .inner_size(size.0, size.1)
            .position(position.0, position.1)
            .min_inner_size(800.0, 450.0)
            .visible(false)
            .build()
            .expect("error creating window");
    }
}

pub fn set_window_state(app_handle: &AppHandle, window: &Window) {
    let scale_factor = window.scale_factor().unwrap();
    let window_state = app_handle.state::<WindowState>();

    if let Ok(size) = window.inner_size() {
        *window_state.size.lock().unwrap() = (
            (size.width as f64) / scale_factor,
            (size.height as f64) / scale_factor,
        );
    }
    if let Ok(position) = window.outer_position() {
        *window_state.position.lock().unwrap() = (
            (position.x as f64) / scale_factor,
            (position.y as f64) / scale_factor,
        );
    }
}
