pub mod macro_record {
    use rdev::{simulate, Button, Event, EventType, Key};
    use serde::{Deserialize, Serialize};
    use std::collections::HashSet;
    use std::fs::{self, File};
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    pub struct AppState {
        pressed_keys: HashSet<Key>,
        is_recording: bool,
        // is_executing_macro = start the loop that execute the macro
        is_executing_macro: Arc<AtomicBool>,
        // macro_is_already_running = prevent execute macro callback to execute multiple times,
        // because callback is in rdev listen() loop
        macro_is_already_running: Arc<AtomicBool>,
        force_stop_macro: Arc<AtomicBool>,
        event_list: Vec<Event>,
    }

    impl AppState {
        pub fn new() -> Self {
            Self {
                pressed_keys: HashSet::new(),
                is_recording: false,
                is_executing_macro: Arc::new(AtomicBool::new(false)),
                force_stop_macro: Arc::new(AtomicBool::new(false)),
                macro_is_already_running: Arc::new(AtomicBool::new(false)),
                event_list: Vec::new(),
            }
        }

        pub fn handle_key_combinaison(
            &mut self,
            event: &Event,
            app_path: &str,
            selected_macro: &str,
            refresh_app: impl Fn(),
        ) {
            // Create a list of all key pressed simultaneously
            match event.event_type {
                EventType::KeyPress(key) => {
                    self.pressed_keys.insert(key);
                }
                EventType::KeyRelease(key) => {
                    self.pressed_keys.remove(&key);
                }
                _ => {}
            }
            fn stop_macro_execution(appstate: &mut AppState) {
                println!("Stop execution");
                appstate.is_executing_macro.store(false, Ordering::Relaxed);
                appstate
                    .macro_is_already_running
                    .store(false, Ordering::Relaxed);
                appstate.force_stop_macro.store(true, Ordering::Relaxed);
                appstate.event_list.clear();
            }
            // Handle key combinaison
            if self.pressed_keys.contains(&Key::ControlLeft)
                && self.pressed_keys.contains(&Key::ShiftLeft)
            {
                if let Some(name) = &event.name {
                    // ------------------------------------- Ctrl + Shift + C ------------------------------------------------

                    if name.eq_ignore_ascii_case("c") {
                        stop_macro_execution(self);

                        // --------------------------------- Ctrl + Shift + R ------------------------------------------------
                    } else if name.eq_ignore_ascii_case("r") {
                        println!("start/stop record");
                        // If is recording, stop the record and save the macro
                        if self.is_recording {
                            self.is_recording = false;
                            // Remove the last 3 items since it is the keyboard combinaison to stop the record
                            self.event_list.pop();
                            self.event_list.pop();
                            self.event_list.pop();
                            save_macro(app_path, &self.event_list, refresh_app);
                        } else {
                            // If not recording, clear event list and start recording
                            self.event_list.clear();
                            self.is_recording = true;
                        }

                    // -------------------------------------- Ctrl + Shift + X ------------------------------------------------
                    } else if name.eq_ignore_ascii_case("x") {
                        // If user press Ctrl + shift + x again during execution, it stop the macro
                        if self.is_executing_macro.load(Ordering::Relaxed) {
                            stop_macro_execution(self);
                            return;
                        }
                        // load macro and replace self.event_list with loaded file
                        println!("start execute macro");
                        let macro_event_list = load_macro(app_path, selected_macro);
                        self.event_list.clear();
                        self.event_list.extend_from_slice(&macro_event_list);

                        self.force_stop_macro.store(false, Ordering::Relaxed);
                        self.is_executing_macro.store(true, Ordering::Relaxed);
                    }
                }
            }
        }

        pub fn record_macro_handler(&mut self, event: &Event) {
            if !self.is_recording {
                return;
            }
            self.event_list.push(event.clone());
        }

        pub fn execute_macro_handler_async(&mut self) {
            if !self.is_executing_macro.load(Ordering::Relaxed) {
                //
                return;
            }
            if self.macro_is_already_running.load(Ordering::Relaxed) {
                return;
            }
            self.macro_is_already_running.store(true, Ordering::Relaxed);

            // clone self object to use it in another thread
            let events = self.event_list.clone();
            let stop_flag = self.force_stop_macro.clone();
            let is_executing_macro = self.is_executing_macro.clone();
            let macro_is_already_running = self.macro_is_already_running.clone();

            thread::spawn(move || {
                if events.is_empty() {
                    return;
                }

                let start_time = std::time::Instant::now();
                let first_event_time = events[0].time;

                for event in events.iter() {
                    if stop_flag.load(Ordering::Relaxed) {
                        println!("Macro stopped!");
                        break;
                    }

                    // Compute absolute target time
                    let target = start_time
                        + event
                            .time
                            .duration_since(first_event_time)
                            .unwrap_or(std::time::Duration::ZERO);

                    // Hybrid wait
                    loop {
                        let now = std::time::Instant::now();
                        if now >= target || stop_flag.load(Ordering::Relaxed) {
                            break;
                        }

                        let remaining = target - now;
                        if remaining > std::time::Duration::from_millis(2) {
                            std::thread::sleep(remaining - std::time::Duration::from_millis(1));
                        } else {
                            std::hint::spin_loop();
                        }
                    }

                    if stop_flag.load(Ordering::Relaxed) {
                        println!("Macro stopped!");
                        break;
                    }

                    simulate(&event.event_type).unwrap();
                }

                simulate(&EventType::KeyRelease(Key::Alt)).unwrap();
                simulate(&EventType::KeyRelease(Key::ShiftLeft)).unwrap();
                simulate(&EventType::KeyRelease(Key::ControlLeft)).unwrap();
                is_executing_macro.store(false, Ordering::Relaxed); // reset flag
                macro_is_already_running.store(false, Ordering::Relaxed);
                stop_flag.store(false, Ordering::Relaxed); // reset flag
            });
        }
    }

    pub fn get_config_path() -> PathBuf {
        // This will return the user-specific config directory:
        // - Linux: $XDG_CONFIG_HOME or ~/.config
        // - macOS: ~/Library/Application Support
        // - Windows: {FOLDERID_RoamingAppData} (usually C:\Users\username\AppData\Roaming)
        let mut config_dir = dirs_next::config_dir().expect("Could not find config directory");

        // Append your app-specific folder
        config_dir.push("rust_macro_recorder");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        }

        config_dir
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SerializableEvent {
        pub event_type: String, // "KeyPress", "MouseMove", "ButtonPress", etc.

        pub key: Option<String>,    // for key events (A, B, Ctrl, ...)
        pub button: Option<String>, // for mouse buttons (Left, Right, Middle)

        pub x: Option<f64>, // MouseMove X
        pub y: Option<f64>, // MouseMove Y

        pub delta_x: Option<i64>, // MouseWheel deltahttps://open.spotify.com/playlist/37i9dQZEVXcT7t1wegwehI
        pub delta_y: Option<i64>,

        pub event_name: Option<String>, // comes from event.name (may be None)
        pub time: u64,
    }

    impl SerializableEvent {
        pub fn from_rdev(event: &Event) -> Self {
            let mut event_type = "Other".to_string();
            let mut key = None;
            let mut button = None;
            let mut x = None;
            let mut y = None;
            let mut delta_x = None;
            let mut delta_y = None;

            match event.event_type {
                EventType::KeyPress(k) => {
                    event_type = "KeyPress".into();
                    key = Some(format!("{:?}", k));
                }
                EventType::KeyRelease(k) => {
                    event_type = "KeyRelease".into();
                    key = Some(format!("{:?}", k));
                }

                EventType::MouseMove { x: mx, y: my } => {
                    event_type = "MouseMove".into();
                    x = Some(mx);
                    y = Some(my);
                }

                EventType::ButtonPress(b) => {
                    event_type = "ButtonPress".into();
                    button = Some(format!("{:?}", b));
                }
                EventType::ButtonRelease(b) => {
                    event_type = "ButtonRelease".into();
                    button = Some(format!("{:?}", b));
                }

                EventType::Wheel {
                    delta_x: dx,
                    delta_y: dy,
                } => {
                    event_type = "Wheel".into();
                    delta_x = Some(dx);
                    delta_y = Some(dy);
                }
            }

            let time = event
                .time
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            Self {
                event_type,
                key,
                button,
                x,
                y,
                delta_x,
                delta_y,
                event_name: event.name.clone(),
                time: time,
            }
        }

        pub fn to_rdev(&self) -> Event {
            pub fn parse_key(name: &str) -> Key {
                match name {
                    "Alt" => Key::Alt,
                    "AltGr" => Key::AltGr,
                    "Backspace" => Key::Backspace,
                    "CapsLock" => Key::CapsLock,
                    "ControlLeft" => Key::ControlLeft,
                    "ControlRight" => Key::ControlRight,
                    "Delete" => Key::Delete,
                    "DownArrow" => Key::DownArrow,
                    "End" => Key::End,
                    "Escape" => Key::Escape,
                    "F1" => Key::F1,
                    "F2" => Key::F2,
                    "F3" => Key::F3,
                    "F4" => Key::F4,
                    "F5" => Key::F5,
                    "F6" => Key::F6,
                    "F7" => Key::F7,
                    "F8" => Key::F8,
                    "F9" => Key::F9,
                    "F10" => Key::F10,
                    "F11" => Key::F11,
                    "F12" => Key::F12,
                    "Home" => Key::Home,
                    "LeftArrow" => Key::LeftArrow,
                    "MetaLeft" => Key::MetaLeft,
                    "MetaRight" => Key::MetaRight,
                    "PageDown" => Key::PageDown,
                    "PageUp" => Key::PageUp,
                    "Return" => Key::Return,
                    "RightArrow" => Key::RightArrow,
                    "ShiftLeft" => Key::ShiftLeft,
                    "ShiftRight" => Key::ShiftRight,
                    "Space" => Key::Space,
                    "Tab" => Key::Tab,
                    "UpArrow" => Key::UpArrow,
                    "PrintScreen" => Key::PrintScreen,
                    "ScrollLock" => Key::ScrollLock,
                    "Pause" => Key::Pause,
                    "NumLock" => Key::NumLock,
                    "BackQuote" => Key::BackQuote,
                    "Num1" => Key::Num1,
                    "Num2" => Key::Num2,
                    "Num3" => Key::Num3,
                    "Num4" => Key::Num4,
                    "Num5" => Key::Num5,
                    "Num6" => Key::Num6,
                    "Num7" => Key::Num7,
                    "Num8" => Key::Num8,
                    "Num9" => Key::Num9,
                    "Num0" => Key::Num0,
                    "Minus" => Key::Minus,
                    "Equal" => Key::Equal,
                    "KeyQ" => Key::KeyQ,
                    "KeyW" => Key::KeyW,
                    "KeyE" => Key::KeyE,
                    "KeyR" => Key::KeyR,
                    "KeyT" => Key::KeyT,
                    "KeyY" => Key::KeyY,
                    "KeyU" => Key::KeyU,
                    "KeyI" => Key::KeyI,
                    "KeyO" => Key::KeyO,
                    "KeyP" => Key::KeyP,
                    "LeftBracket" => Key::LeftBracket,
                    "RightBracket" => Key::RightBracket,
                    "KeyA" => Key::KeyA,
                    "KeyS" => Key::KeyS,
                    "KeyD" => Key::KeyD,
                    "KeyF" => Key::KeyF,
                    "KeyG" => Key::KeyG,
                    "KeyH" => Key::KeyH,
                    "KeyJ" => Key::KeyJ,
                    "KeyK" => Key::KeyK,
                    "KeyL" => Key::KeyL,
                    "SemiColon" => Key::SemiColon,
                    "Quote" => Key::Quote,
                    "BackSlash" => Key::BackSlash,
                    "IntlBackslash" => Key::IntlBackslash,
                    "KeyZ" => Key::KeyZ,
                    "KeyX" => Key::KeyX,
                    "KeyC" => Key::KeyC,
                    "KeyV" => Key::KeyV,
                    "KeyB" => Key::KeyB,
                    "KeyN" => Key::KeyN,
                    "KeyM" => Key::KeyM,
                    "Comma" => Key::Comma,
                    "Dot" => Key::Dot,
                    "Slash" => Key::Slash,
                    "Insert" => Key::Insert,
                    "KpReturn" => Key::KpReturn,
                    "KpMinus" => Key::KpMinus,
                    "KpPlus" => Key::KpPlus,
                    "KpMultiply" => Key::KpMultiply,
                    "KpDivide" => Key::KpDivide,
                    "Kp0" => Key::Kp0,
                    "Kp1" => Key::Kp1,
                    "Kp2" => Key::Kp2,
                    "Kp3" => Key::Kp3,
                    "Kp4" => Key::Kp4,
                    "Kp5" => Key::Kp5,
                    "Kp6" => Key::Kp6,
                    "Kp7" => Key::Kp7,
                    "Kp8" => Key::Kp8,
                    "Kp9" => Key::Kp9,
                    "KpDelete" => Key::KpDelete,
                    "Function" => Key::Function,
                    other => {
                        // Handle Unknown(n) format like "Unknown(123)"
                        if let Some(inner) = other
                            .strip_prefix("Unknown(")
                            .and_then(|s| s.strip_suffix(")"))
                        {
                            if let Ok(num) = inner.parse::<u32>() {
                                return Key::Unknown(num);
                            }
                        }
                        // fallback: Unknown(0)
                        return Key::Unknown(0);
                    }
                }
            }
            pub fn parse_key_opt(name: Option<&str>) -> Key {
                match name {
                    Some(s) => parse_key(s),
                    None => Key::Unknown(0),
                }
            }
            fn parse_button(name: Option<&str>) -> Button {
                match name.unwrap_or("") {
                    "Left" => Button::Left,
                    "Right" => Button::Right,
                    "Middle" => Button::Middle,
                    _ => Button::Unknown(0),
                }
            }

            let event_type = match self.event_type.as_str() {
                "KeyPress" => {
                    let key = parse_key_opt(self.key.as_deref());
                    EventType::KeyPress(key)
                }
                "KeyRelease" => {
                    let key = parse_key_opt(self.key.as_deref());
                    EventType::KeyRelease(key)
                }
                "MouseMove" => EventType::MouseMove {
                    x: self.x.unwrap_or(0.0),
                    y: self.y.unwrap_or(0.0),
                },
                "ButtonPress" => {
                    let btn = parse_button(self.button.as_deref());
                    EventType::ButtonPress(btn)
                }
                "ButtonRelease" => {
                    let btn = parse_button(self.button.as_deref());
                    EventType::ButtonRelease(btn)
                }
                "Wheel" => EventType::Wheel {
                    delta_x: self.delta_x.unwrap_or(0) as i64,
                    delta_y: self.delta_y.unwrap_or(0) as i64,
                },
                // error : ensure that all possible cases are being handled by adding a match arm with a wildcard pattern or an explicit pattern as shown: `,
                // &_ => todo!()`
                // But all case are covered and rust doesn't know it , so Catch-all arm is replaced by a harmless action :
                // It release the pause key (a key not much used, if the key was already release it will release it again which does nothing)
                _ => EventType::KeyRelease(Key::Pause),
            };

            let time = UNIX_EPOCH + std::time::Duration::from_millis(self.time);

            Event {
                event_type,
                name: self.event_name.clone(),
                time,
            }
        }
    }

    pub fn convert_event_list(events: &[Event]) -> Vec<SerializableEvent> {
        events
            .iter()
            .map(|e| SerializableEvent::from_rdev(e))
            .collect()
    }

    pub fn convert_serializable_list(events: &[SerializableEvent]) -> Vec<Event> {
        events.iter().map(|e| e.to_rdev()).collect()
    }

    pub fn save_macro(path: &str, events: &[Event], refresh_app: impl Fn()) -> bool {
        // Générer un nom unique basé sur le timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp_ms = now.as_millis();
        let macro_name = format!("macro_{}.json", timestamp_ms);

        // Construire le chemin complet
        let save_path = Path::new(path).join(macro_name);

        // Convertir en SerializableEvent
        let serializable_events: Vec<SerializableEvent> = convert_event_list(events);

        // Sérialiser en JSON
        let json = match serde_json::to_string(&serializable_events) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("Erreur lors de la sérialisation des événements : {}", e);
                return false;
            }
        };

        // Créer / ouvrir le fichier
        let mut file = match File::create(&save_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "Impossible de créer le fichier '{}': {}",
                    save_path.display(),
                    e
                );
                return false;
            }
        };

        // Écrire le JSON dans le fichier
        if let Err(e) = file.write_all(json.as_bytes()) {
            eprintln!(
                "Impossible d'écrire dans le fichier '{}': {}",
                save_path.display(),
                e
            );
            return false;
        }

        println!("Macro sauvegardée avec succès : {}", save_path.display());
        refresh_app();
        true
    }

    pub fn load_macro(path: &str, macro_name: &str) -> Vec<Event> {
        // Construire le chemin complet
        let save_path = Path::new(path).join(macro_name);
        // Ouvrir le fichier
        let mut file = match File::open(&save_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Impossible d'ouvrir le fichier '{:?}': {}", &save_path, e);
                return vec![]; // renvoie un vecteur vide si le fichier est manquant
            }
        };

        // Lire le contenu
        let mut content = String::new();
        if let Err(e) = file.read_to_string(&mut content) {
            eprintln!("Impossible de lire le fichier '{}': {}", path, e);
            return vec![];
        }

        // Désérialiser les événements
        let serializable_events: Vec<SerializableEvent> = match serde_json::from_str(&content) {
            Ok(events) => events,
            Err(e) => {
                eprintln!("Erreur de désérialisation JSON dans '{}': {}", path, e);
                return vec![];
            }
        };

        // Convertir en rdev::Event
        let events: Vec<Event> = convert_serializable_list(&serializable_events);

        events
    }

    /// Estimates macro duration in seconds from a Vec<Event>
    /// Returns 0 if there are fewer than 2 events
    pub fn estimate_macro_time(events: &[Event]) -> u64 {
        if events.is_empty() {
            return 0;
        }

        // Replace `event.time` with the actual timestamp field in Event
        // Here we assume each Event has a `time: SystemTime`
        let first_time: SystemTime = events.first().unwrap().time;
        let last_time: SystemTime = events.last().unwrap().time;

        // duration_since returns Result<Duration, SystemTimeError>
        match last_time.duration_since(first_time) {
            Ok(dur) => dur.as_secs(),
            Err(_) => 0, // in case last_time < first_time
        }
    }
}
