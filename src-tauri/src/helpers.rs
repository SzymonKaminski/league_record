use std::{cmp::Ordering, io, path::PathBuf};

use reqwest::blocking::Client;
use tauri::{AppHandle, Manager};

pub fn create_client() -> Client {
    let pem = include_bytes!("../riotgames.pem");
    let cert = reqwest::Certificate::from_pem(pem).unwrap();
    let client = Client::builder()
        .add_root_certificate(cert)
        .build()
        .unwrap();
    return client;
}

pub fn get_recordings(rec_folder: PathBuf) -> Vec<PathBuf> {
    // get all mp4 files in ~/Videos/%folder-name%
    let mut recordings = Vec::<PathBuf>::new();
    let rd_dir = if let Ok(rd_dir) = rec_folder.read_dir() {
        rd_dir
    } else {
        return vec![];
    };
    for entry in rd_dir {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "mp4" {
                    recordings.push(path);
                }
            }
        }
    }
    return recordings;
}

pub fn compare_time(a: &PathBuf, b: &PathBuf) -> io::Result<Ordering> {
    let a_time = a.metadata()?.created()?;
    let b_time = b.metadata()?.created()?;
    Ok(a_time.cmp(&b_time).reverse())
}

pub fn show_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
