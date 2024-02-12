use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::{env, thread};
use fastnbt::Value;
use flate2::read::GzDecoder;
use std::io::Read;
use serde::{Deserialize, Serialize};
use config::Config as Settings;
use std::collections::HashMap;
use enigo::*;
use inputbot::KeybdKey;


const ONE_MINUTE_IN_TICKS: u64 = 1200;
static mut AUTO_RESET: bool = false;

#[derive(Serialize, Deserialize, Debug)]
struct LevelDat {
    #[serde(rename = "Data")]
    data: Data,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Data {
    thunder_time: u64,
    rain_time: u64,

    #[serde(flatten)]
    other: HashMap<String, Value>,
}

fn main() {
    match load_config() {
        None => {
            println!("ERROR: Couldn't load the config file.");
        },
        Some(config) => {
            let path: String = config.get("instance_path").unwrap();
            let reset_hotkey: char = config.get("reset_hotkey").unwrap();

            println!("Watching {path}");

            KeybdKey::bind_all(move |event| {
                match inputbot::from_keybd_key(event) {
                    Some(key) => {
                        unsafe {
                            if key == reset_hotkey && AUTO_RESET == false {
                                AUTO_RESET = true;
                                println!("Auto resetting is now enabled.");
                            }
                            
                            if key != reset_hotkey && AUTO_RESET == true {
                                AUTO_RESET = false;
                                println!("Auto resetting is now disabled.")
                            }
                        }
                    },
                    None => {},
                };
            });

            thread::spawn(|| {
                inputbot::handle_input_events();
            });
        
            if let Err(error) = watch(config) {
                println!("Error: {error:?}");
            }
        }
    }
}

fn load_config() -> Option<Settings> {
    match env::current_dir() {
        Err(err) => {
            println!("ERROR: {}", err);
        },
        Ok(path) => {
            match path.join("config.toml").to_str() {
                None => {
                    println!("ERROR: Couldn't convert config file path to string.");
                },
                Some(path) => {
                    match Settings::builder().add_source(config::File::with_name(path)).build() {
                        Err(err) => {
                            println!("ERROR: {}", err);
                        },
                        Ok(config) => {
                            return Some(config)
                        }
                    }
                }
            }
        }
    }
    
    None
}

fn watch(config: Settings) -> notify::Result<()> {
    let path: String = config.get("instance_path").unwrap();
    let reset_hotkey: char = config.get("reset_hotkey").unwrap();
    let min_thunder_start_time: u64 = config.get("min_thunder_start_time").unwrap();
    let max_thunder_start_time: u64 = config.get("max_thunder_start_time").unwrap();
    let min_thunder_duration: u64 = config.get("min_thunder_duration").unwrap();
    let debug_mode: bool = config.get("debug_mode").unwrap();

    let mut enigo = Enigo::new();
    
    let mut last_world: String = String::new();
    let (tx, rx) = std::sync::mpsc::channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(&path.as_ref(), RecursiveMode::NonRecursive).unwrap();

    for res in rx {
        match res {
            Ok(_event) => {
                unsafe {
                    if !AUTO_RESET { continue; }
                }

                let current_world = std::fs::read_dir(&path)
                    .expect("ERROR: Couldn't access local directory.")
                    .flatten() // Remove failed
                    .max_by_key(|x| x.metadata().unwrap().modified().unwrap()); // Get the most recently modified file

                let path = current_world.expect("ERROR: Couldn't access current world.").path();
                let string_path = path.clone().to_str().expect("ERROR: Couldn't convert current world path to string.").to_owned();
                match std::fs::File::open(path.join("level.dat")) {
                    Err(_) => {},
                    Ok(file) => {
                        let mut decoder = GzDecoder::new(file);
                        let mut bytes = vec![];
                        decoder.read_to_end(&mut bytes).expect("ERROR: Couldn't read the level.dat file.");
                    
                        let val: LevelDat = fastnbt::from_bytes(&bytes).expect("ERROR: Couldn't read the level.dat file.");
                    
                        let thunder_time: u64 = val.data.thunder_time;
                        let rain_time: u64 = val.data.rain_time;

                        if thunder_time != 0 && rain_time != 0 && string_path != last_world {
                            if debug_mode {
                                println!("DEBUG: Rain cycle start: {}; Thunder cycle start: {};", format_time(rain_time), format_time(thunder_time));
                            }

                            if thunder_time > max_thunder_start_time || 
                                rain_time > max_thunder_start_time || 
                                thunder_time < min_thunder_start_time || 
                                rain_time < min_thunder_start_time 
                            {
                                enigo.key_click(Key::Layout(reset_hotkey));
                            }
                            
                            if thunder_time > rain_time {
                                if thunder_time - rain_time > ONE_MINUTE_IN_TICKS * 10 - min_thunder_duration {
                                    enigo.key_click(Key::Layout(reset_hotkey));
                                }
                            } else if rain_time > thunder_time {
                                if rain_time - thunder_time > ONE_MINUTE_IN_TICKS * 3 - min_thunder_duration {
                                    enigo.key_click(Key::Layout(reset_hotkey));
                                }
                            }
                        }

                        if thunder_time != 0 && rain_time != 0 {
                            last_world = string_path;
                        }
                    }
                }
            },
            Err(error) => println!("Error: {error:?}"),
        }
    }

    Ok(())
}

pub fn format_time(time: u64) -> String {
    let hours: u64 = time / (ONE_MINUTE_IN_TICKS * 60);
    let minutes: u64 = (time - hours * ONE_MINUTE_IN_TICKS * 60) / ONE_MINUTE_IN_TICKS;
    let seconds: u64 = (time - hours * ONE_MINUTE_IN_TICKS * 60 - minutes * ONE_MINUTE_IN_TICKS) / 20;
    let milliseconds: u64 = (time - hours * ONE_MINUTE_IN_TICKS * 60 - minutes * ONE_MINUTE_IN_TICKS - seconds * 20) * 5;

    let hours: String = if hours.to_string().len() == 1 { format!("0{}", hours) } else { hours.to_string() };
    let minutes: String = if minutes.to_string().len() == 1 { format!("0{}", minutes) } else { minutes.to_string() };
    let seconds: String = if seconds.to_string().len() == 1 { format!("0{}", seconds) } else { seconds.to_string() };
    let milliseconds: u64 = match milliseconds.to_string().len() {
        1 => { milliseconds * 100 },
        2 => { milliseconds * 10 },
        _ => { milliseconds }
    };

    format!("{}:{}:{}.{}", hours, minutes, seconds, milliseconds)
}