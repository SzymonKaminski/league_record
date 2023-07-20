mod data;

use std::{
    path::Path,
    sync::{
        mpsc::{channel, RecvTimeoutError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use libobs_recorder::{
    settings::{RateControl, Resolution, Size, Window},
    Recorder, RecorderSettings,
};
use shaco::{
    ingame::{EventStream, IngameClient},
    model::{
        ingame::{ChampionKill, DragonType, GameEvent, GameResult, Killer},
        ws::LcuSubscriptionType::JsonApiEvent,
    },
    ws::LcuWebsocketClient,
};
use tauri::{async_runtime, AppHandle, Manager, Runtime};
use tokio_util::sync::CancellationToken;
#[cfg(target_os = "windows")]
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{FindWindowA, GetClientRect},
    },
};

use crate::state::Settings;

const WINDOW_TITLE: &str = "League of Legends (TM) Client";
const WINDOW_CLASS: &str = "RiotWindowClass";
const WINDOW_PROCESS: &str = "League of Legends.exe";

fn set_recording_tray_item<R: Runtime>(app_handle: &AppHandle<R>, recording: bool) {
    let item = app_handle.tray_handle().get_item("rec");
    // set selected only updates the tray menu when open if the menu item is enabled
    _ = item.set_enabled(true);
    _ = item.set_selected(recording);
    _ = item.set_enabled(false);
}

#[cfg(target_os = "windows")]
fn get_lol_window() -> Option<HWND> {
    let mut window_title = WINDOW_TITLE.to_owned();
    window_title.push('\0'); // null terminate
    let mut window_class = WINDOW_CLASS.to_owned();
    window_class.push('\0'); // null terminate

    let title = PCSTR(window_title.as_ptr());
    let class = PCSTR(window_class.as_ptr());

    let hwnd = unsafe { FindWindowA(class, title) };
    if hwnd.is_invalid() {
        return None;
    }
    Some(hwnd)
}

#[cfg(target_os = "windows")]
fn get_window_size(hwnd: HWND) -> Result<Size, ()> {
    let mut rect = RECT::default();
    let ok = unsafe { GetClientRect(hwnd, &mut rect as _).as_bool() };
    if ok && rect.right > 0 && rect.bottom > 0 {
        Ok(Size::new(rect.right as u32, rect.bottom as u32))
    } else {
        Err(())
    }
}

pub fn start<R: Runtime>(app_handle: AppHandle<R>) {
    thread::spawn(move || {
        // send stop to channel on "shutdown" event
        let (tx, rx) = channel::<_>();
        app_handle.once_global("shutdown_recorder", move |_| _ = tx.send(()));

        // get owned copy of settings so we can change window_size
        let settings_state = app_handle.state::<Settings>();
        let debug_log = settings_state.debug_log();

        // use Options to 'store' values between loops
        let recorder: Arc<Mutex<Option<Recorder>>> = Arc::new(Mutex::new(None));
        let mut filename = None;
        let mut game_data_thread: Option<(async_runtime::JoinHandle<()>, CancellationToken)> = None;

        loop {
            match get_lol_window() {
                // initialize recorder
                Some(window_handle) if recorder.lock().unwrap().is_none() => {
                    if debug_log {
                        println!("LoL Window found");
                    }

                    let Ok(mut rec) =
                        Recorder::new_with_paths(Some(Path::new("./libobs/extprocess_recorder.exe")), None, None, None)
                    else {
                        continue;
                    };

                    let mut settings = RecorderSettings::new();

                    settings.set_window(Window::new(
                        WINDOW_TITLE,
                        Some(WINDOW_CLASS.into()),
                        Some(WINDOW_PROCESS.into()),
                    ));

                    settings.set_input_size(
                        get_window_size(window_handle).unwrap_or_else(|_| Resolution::_1080p.get_size()),
                    );

                    settings.set_output_resolution(settings_state.get_output_resolution());
                    settings.set_framerate(settings_state.get_framerate());
                    settings.set_rate_control(RateControl::CQP(settings_state.get_encoding_quality()));
                    settings.record_audio(settings_state.get_audio_source());

                    let mut filename_path = settings_state.get_recordings_path();
                    filename_path.push(format!(
                        "{}",
                        chrono::Local::now().format(&settings_state.get_filename_format())
                    ));
                    settings.set_output_path(filename_path.to_str().expect("error converting filename path to &str"));
                    filename.replace(filename_path);

                    rec.configure(&settings);

                    *recorder.lock().unwrap() = Some(rec)

                    // wait 2 seconds before continuing to make sure the Recorder is ready / initialized
                    // else we get a completely black screen at the start of the recording
                    // std::thread::sleep(Duration::from_secs(2));
                }
                // initialize game_data, previous match branch garuantees recorder to be Some
                Some(_) if game_data_thread.is_none() => {
                    let cancel_token = CancellationToken::new();
                    let cancel_subtoken = cancel_token.child_token();

                    let app_handle = app_handle.clone();
                    let recorder = Arc::clone(&recorder);
                    let mut outfile = settings_state.get_recordings_path().join(filename.as_ref().unwrap());
                    outfile.set_extension("json");

                    let handle = async_runtime::spawn(async move {
                        // IngameClient::new() never actually returns Err()
                        let ingame_client = IngameClient::new().unwrap();

                        let mut timer = tokio::time::interval(Duration::from_millis(500));
                        while !ingame_client.active_game().await {
                            // busy wait for API
                            // "sleep" by selecting either the next timer tick or the token cancel
                            tokio::select! {
                                _ = cancel_subtoken.cancelled() => return,
                                _ = timer.tick() => {}
                            }
                        }

                        // don't record spectator games
                        if let Ok(true) = ingame_client.is_spectator_mode().await {
                            println!("spectator game detected");
                            return;
                        }

                        let mut game_data = data::GameData::default();
                        if let Ok(data) = ingame_client.all_game_data(None).await {
                            game_data.game_info.game_mode = data.game_data.game_mode.to_string();
                            // unwrap because active player always exists in livegame which we check for above
                            game_data.game_info.summoner_name = data.active_player.unwrap().summoner_name;
                            game_data.game_info.champion_name = data
                                .all_players
                                .into_iter()
                                .find_map(|p| {
                                    if p.summoner_name == game_data.game_info.summoner_name {
                                        Some(p.champion_name)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap();
                        }

                        // if initial game_data is successfull => start recording
                        if let Some(rec) = recorder.lock().unwrap().as_mut() {
                            if !rec.start_recording() {
                                // if recording start failed stop recording just in case and retry next loop
                                rec.stop_recording();
                                return;
                            }
                        } else {
                            return;
                        }

                        // recording started
                        let recording_start = Some(Instant::now());
                        set_recording_tray_item(&app_handle, true);

                        // get values from Options that are always Some
                        let mut ingame_events = EventStream::from_ingame_client(ingame_client, None);
                        let recording_start = recording_start.as_ref().unwrap();

                        while let Some(event) = tokio::select! { event = ingame_events.next() => event, _ = cancel_subtoken.cancelled() => None }
                        {
                            let time = recording_start.elapsed().as_secs_f64();
                            println!("[{}]: {:?}", time, event);

                            let event_name = match event {
                                GameEvent::BaronKill(_) => Some("Baron"),
                                GameEvent::ChampionKill(e) => {
                                    let summoner_name = &game_data.game_info.summoner_name;
                                    match e {
                                        ChampionKill {
                                            killer_name: Killer::Summoner(ref killer_name),
                                            ..
                                        } if killer_name == summoner_name => Some("Kill"),
                                        ChampionKill { ref victim_name, .. } if victim_name == summoner_name => {
                                            Some("Death")
                                        }
                                        ChampionKill { assisters, .. } if assisters.contains(summoner_name) => {
                                            Some("Assist")
                                        }
                                        _ => None,
                                    }
                                }
                                GameEvent::DragonKill(e) => {
                                    let dragon = match e.dragon_type {
                                        DragonType::Infernal => "Infernal-Dragon",
                                        DragonType::Ocean => "Ocean-Dragon",
                                        DragonType::Mountain => "Mountain-Dragon",
                                        DragonType::Cloud => "Cloud-Dragon",
                                        DragonType::Hextech => "Hextech-Dragon",
                                        DragonType::Chemtech => "Chemtech-Dragon",
                                        DragonType::Elder => "Elder-Dragon",
                                    };
                                    Some(dragon)
                                }
                                GameEvent::GameEnd(e) => {
                                    game_data.win = match e.result {
                                        GameResult::Win => Some(true),
                                        GameResult::Lose => Some(false),
                                    };
                                    None
                                }
                                GameEvent::HeraldKill(_) => Some("Herald"),
                                GameEvent::InhibKilled(_) => Some("Inhibitor"),
                                GameEvent::TurretKilled(_) => Some("Turret"),
                                _ => None,
                            };

                            if let Some(name) = event_name {
                                game_data.events.push(data::GameEvent { name, time })
                            }
                        }

                        // after the game client closes wait for LCU websocket End Of Game event
                        let Ok(mut ws_client) = LcuWebsocketClient::connect().await else {
                            return;
                        };
                        let subscription = ws_client
                            .subscribe(JsonApiEvent("lol-end-of-game/v1/eog-stats-block".to_string()))
                            .await;
                        if subscription.is_err() {
                            return;
                        }

                        tokio::select! {
                            _ = cancel_subtoken.cancelled() => (),
                            event = tokio::time::timeout(Duration::from_secs(30), ws_client.next()) => {
                                if let Ok(Some(mut event)) = event {
                                    println!("EOG stats: {:?}", event.data);

                                    let json_stats = event.data["localPlayer"]["stats"].take();

                                    if game_data.win.is_none() {
                                        // on win the data contains a "WIN" key with a value of '1'
                                        // on lose the data contains a "LOSE" key with a value of '1'
                                        // So if json_stats["WIN"] is not null => WIN
                                        // and if json_stats["LOSE"] is not null => LOSE
                                        if !json_stats["WIN"].is_null() {
                                            game_data.win = Some(true);
                                        } else if !json_stats["LOSE"].is_null() {
                                            game_data.win = Some(false);
                                        }
                                    }

                                    match serde_json::from_value(json_stats) {
                                        Ok(stats) => game_data.stats = stats,
                                        Err(e) => println!("Error deserializing end of game stats: {:?}", e),
                                    }
                                } else {
                                    println!("LCU event listener timed out");
                                }
                            }
                        }

                        async_runtime::spawn_blocking(move || {
                            // serde_json requires a std::fs::File
                            if let Ok(file) = std::fs::File::create(&outfile) {
                                _ = serde_json::to_writer(file, &game_data);
                                println!("metadata saved");
                            }
                        });
                    });

                    game_data_thread.replace((handle, cancel_token));
                }
                Some(_) => { /* do nothing while recording is active */ }
                None => {
                    // stop recorder
                    if let Some(mut rec) = recorder.lock().unwrap().take() {
                        rec.stop_recording();
                        _ = rec.shutdown();
                        set_recording_tray_item(&app_handle, false);
                    };

                    // spawn async thread to cleanup the game_data_thread if it doesn't exit by itself
                    if let Some((mut handle, cancel_token)) = game_data_thread.take() {
                        if handle.inner().is_finished() {
                            continue;
                        }

                        async_runtime::spawn(async move {
                            // wait for 30s for EOG lobby before cancelling the task
                            match tokio::time::timeout(Duration::from_secs(30), &mut handle).await {
                                Ok(_) => return,
                                Err(_) => cancel_token.cancel(),
                            }
                            // abort task if it still hasn't stopped after 15s
                            if let Err(_) = tokio::time::timeout(Duration::from_secs(15), &mut handle).await {
                                handle.abort();
                            }
                        });
                    }
                }
            }

            // break if stop event received or sender disconnected
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(_) | Err(RecvTimeoutError::Disconnected) => {
                    // stop recorder if running
                    if let Some(mut rec) = recorder.lock().unwrap().take() {
                        rec.stop_recording();
                        _ = rec.shutdown();
                        set_recording_tray_item(&app_handle, false);
                    };

                    // spawn async thread to cleanup the game_data_thread if it doesn't exit by itself
                    if let Some((handle, cancel_token)) = game_data_thread.take() {
                        cancel_token.cancel();
                        // give the task a little bit of time to complete a fs::write or sth
                        std::thread::sleep(Duration::from_millis(250));
                        handle.abort();
                    }
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }

        app_handle.trigger_global("recorder_shutdown", None);
    });
}