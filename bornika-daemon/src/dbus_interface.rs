use bornika_phonetic::{KeyEvent, PhoneticEngine, VirtualKey};
use std::collections::HashMap;
use std::sync::Mutex;
use zbus::zvariant::{ObjectPath, Value};
use zbus::{dbus_interface, SignalContext};

pub fn log_info(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/bornika.log")
    {
        let _ = writeln!(file, "{}", msg);
    }
}

/// Dynamically constructs the exact GVariant wire layout expected by the IBus daemon.
///
/// In IBus, all serializable objects must be wrapped in an IBusSerializable envelope:
/// Signature: (sa{sv}sv)
///   1. "IBusText" (class name: s)
///   2. Attachments dictionary (a{sv})
///   3. Text string (s)
///   4. AttrList variant (v containing IBusAttrList envelope: (sa{sv}av))
pub fn create_ibus_text(text: &str) -> Value<'static> {
    // 1. Build the inner IBusAttrList (empty for default style)
    let attr_attachments = HashMap::<String, Value<'static>>::new();
    let attr_properties = Vec::<Value<'static>>::new();
    let attr_list_struct = ("IBusAttrList", attr_attachments, attr_properties);
    let attr_list_variant = Value::new(attr_list_struct);

    // 2. Build the outer IBusText
    let text_attachments = HashMap::<String, Value<'static>>::new();
    let ibus_text_struct = (
        "IBusText",
        text_attachments,
        text.to_string(),
        attr_list_variant,
    );

    Value::new(ibus_text_struct)
}

/// Constructs a styled IBusText with a standard underline attribute spanning the entire text.
/// Text editors require composition attributes (like underlines) to render the preedit text inline in real-time.
pub fn create_ibus_text_styled(text: &str) -> Value<'static> {
    let mut attr_properties = Vec::<Value<'static>>::new();

    if !text.is_empty() {
        // IBusAttribute envelope: (sa{sv}uuii)
        //   1. "IBusAttribute" (class: s)
        //   2. Attachments (a{sv})
        //   3. Type = 1 (Underline)
        //   4. Value = 1 (Single Underline)
        //   5. Start Index = 0
        //   6. End Index = length in bytes
        let attr_attachments = HashMap::<String, Value<'static>>::new();
        let attr_struct = (
            "IBusAttribute",
            attr_attachments,
            1u32,              // IBUS_ATTR_TYPE_UNDERLINE
            1u32,              // IBUS_ATTR_UNDERLINE_SINGLE
            0i32,              // start_index
            text.len() as i32, // end_index in bytes
        );
        attr_properties.push(Value::new(attr_struct));
    }

    let attr_attachments = HashMap::<String, Value<'static>>::new();
    let attr_list_struct = ("IBusAttrList", attr_attachments, attr_properties);
    let attr_list_variant = Value::new(attr_list_struct);

    let text_attachments = HashMap::<String, Value<'static>>::new();
    let ibus_text_struct = (
        "IBusText",
        text_attachments,
        text.to_string(),
        attr_list_variant,
    );

    Value::new(ibus_text_struct)
}

// ----------------- IBusFactory -----------------

pub struct IBusFactory;

#[dbus_interface(name = "org.freedesktop.IBus.Factory")]
impl IBusFactory {
    #[dbus_interface(name = "CreateEngine")]
    async fn create_engine(&self, name: String) -> zbus::fdo::Result<ObjectPath<'_>> {
        log_info(&format!(
            "IBusFactory: CreateEngine called for engine: {}",
            name
        ));
        Ok(ObjectPath::from_static_str("/org/freedesktop/IBus/Engine/bornika").unwrap())
    }
}

// ----------------- IBusEngine -----------------

fn decode_key_event(keyval: u32, state: u32) -> KeyEvent {
    let key = match keyval {
        // KeyEvent has no Mod3/Mod4/Mod5 or Super/Hyper/Meta fields; preserve these shortcuts.
        0xFF1B | 0xFF09
            if state & ((1 << 5) | (1 << 6) | (1 << 7) | (1 << 26) | (1 << 27) | (1 << 28))
                != 0 =>
        {
            VirtualKey::Other
        }
        0xFF1B => VirtualKey::Escape,
        0xFF09 => VirtualKey::Tab,
        0xFF08 => VirtualKey::Backspace,
        0x20 => VirtualKey::Space,
        0xFF0D | 0xFF8D => VirtualKey::Enter,
        0x21..=0x7E => VirtualKey::Char(keyval as u8 as char),
        _ => VirtualKey::Other,
    };

    KeyEvent {
        key,
        ctrl: (state & 4) != 0,
        alt: (state & 8) != 0,
        shift: (state & 1) != 0,
        is_release: (state & (1 << 30)) != 0,
    }
}

pub struct IBusEngine {
    engine: Mutex<PhoneticEngine>,
}

impl IBusEngine {
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(PhoneticEngine::new()),
        }
    }
}

#[dbus_interface(name = "org.freedesktop.IBus.Engine")]
impl IBusEngine {
    #[dbus_interface(name = "FocusIn")]
    async fn focus_in(&self) {
        log_info("Engine: FocusIn");
    }

    #[dbus_interface(name = "FocusOut")]
    async fn focus_out(&self) {
        log_info("Engine: FocusOut");
        let mut engine = self.engine.lock().unwrap();
        engine.clear();
    }

    #[dbus_interface(name = "Reset")]
    async fn reset(&self, #[zbus(signal_context)] ctxt: SignalContext<'_>) {
        log_info("Engine: Reset");
        let was_not_empty = {
            let mut engine = self.engine.lock().unwrap();
            let is_empty = engine.is_empty();
            if !is_empty {
                engine.clear();
            }
            !is_empty
        };

        if was_not_empty {
            let empty_text = create_ibus_text("");
            let _ = Self::update_preedit_text(&ctxt, empty_text, 0, false, 0).await;
        }
    }

    #[dbus_interface(name = "SetCursorLocation")]
    async fn set_cursor_location(&self, x: i32, y: i32, w: i32, h: i32) {
        log_info(&format!(
            "Engine: SetCursorLocation (x: {}, y: {}, w: {}, h: {})",
            x, y, w, h
        ));
    }

    #[dbus_interface(name = "SetCapabilities")]
    async fn set_capabilities(&self, caps: u32) {
        log_info(&format!("Engine: SetCapabilities (caps: {})", caps));
    }

    #[dbus_interface(name = "ProcessKeyEvent")]
    async fn process_key_event(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> bool {
        let bangla_mode = {
            let engine = self.engine.lock().unwrap();
            engine.bangla_mode
        };

        let event = decode_key_event(keyval, state);

        log_info(&format!(
            "Engine: ProcessKeyEvent (keyval: 0x{:X}, keycode: {}, state: 0x{:X}, is_release: {}, bangla_mode: {})",
            keyval, keycode, state, event.is_release, bangla_mode
        ));

        let action = {
            let mut engine = self.engine.lock().unwrap();
            engine.process_key_event(event)
        };

        match action {
            bornika_phonetic::KeyAction::Bypass => false,
            bornika_phonetic::KeyAction::Swallow => true,
            bornika_phonetic::KeyAction::ToggleMode { bangla_mode } => {
                log_info(&format!(
                    "Engine: Toggled Bangla typing mode to {}",
                    bangla_mode
                ));
                let empty_text = create_ibus_text("");
                let _ = Self::update_preedit_text(&ctxt, empty_text, 0, false, 0).await;
                true
            }
            bornika_phonetic::KeyAction::Commit { text, bypass_key } => {
                let empty_text = create_ibus_text("");
                let _ = Self::update_preedit_text(&ctxt, empty_text, 0, false, 0).await;
                let val_text = create_ibus_text(&text);
                let _ = Self::commit_text(&ctxt, val_text).await;
                !bypass_key
            }
            bornika_phonetic::KeyAction::UpdatePreedit {
                text,
                cursor_pos,
                visible,
            } => {
                let styled_text = create_ibus_text_styled(&text);
                let _ = Self::update_preedit_text(&ctxt, styled_text, cursor_pos, visible, 0).await;
                true
            }
        }
    }

    // Signals
    #[dbus_interface(signal, name = "CommitText")]
    async fn commit_text(ctxt: &SignalContext<'_>, text: Value<'_>) -> zbus::Result<()>;

    #[dbus_interface(signal, name = "UpdatePreeditText")]
    async fn update_preedit_text(
        ctxt: &SignalContext<'_>,
        text: Value<'_>,
        cursor_pos: u32,
        visible: bool,
        mode: u32,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bornika_phonetic::KeyAction;

    #[test]
    fn test_ibus_escape_cancels_and_tab_commits() {
        for (keyval, key, expected) in [
            (
                0xFF1B,
                VirtualKey::Escape,
                KeyAction::UpdatePreedit {
                    text: String::new(),
                    cursor_pos: 0,
                    visible: false,
                },
            ),
            (
                0xFF09,
                VirtualKey::Tab,
                KeyAction::Commit {
                    text: "আমি".into(),
                    bypass_key: false,
                },
            ),
        ] {
            // Caps Lock and Num Lock must not turn these controls into shortcuts.
            for state in [0, 2, 16, 18] {
                let mut engine = PhoneticEngine::new();
                for c in b"ami" {
                    engine.process_key_event(decode_key_event(*c as u32, 0));
                }
                let event = decode_key_event(keyval, state);
                assert_eq!(event.key, key);
                assert_eq!(engine.process_key_event(event), expected.clone());
                assert!(engine.is_empty());
                assert_eq!(engine.process_key_event(event), KeyAction::Bypass);
            }
        }
    }

    #[test]
    fn test_ibus_modified_controls_and_releases_bypass() {
        for keyval in [0xFF1B, 0xFF09, 0xFE20] {
            for state in [
                0,
                1,
                4,
                8,
                1 << 5,
                1 << 6,
                1 << 7,
                1 << 26,
                1 << 27,
                1 << 28,
                1 << 30,
            ] {
                if state == 0 && keyval != 0xFE20 {
                    continue;
                }
                let mut engine = PhoneticEngine::new();
                engine.set_buffer("ami".into());
                let event = decode_key_event(keyval, state);
                assert_eq!(
                    engine.process_key_event(event),
                    KeyAction::Bypass,
                    "keyval={keyval:#x}, state={state:#x}"
                );
                assert_eq!(engine.get_buffer(), "ami");
                assert!(engine.bangla_mode);
            }
        }
    }

    #[test]
    fn test_ibus_existing_commit_keys_still_pass_through() {
        for keyval in [0x20, 0xFF0D, 0xFF8D, 0x21, 0x7E] {
            let mut engine = PhoneticEngine::new();
            engine.set_buffer("ami".into());
            assert_eq!(
                engine.process_key_event(decode_key_event(keyval, 0)),
                KeyAction::Commit {
                    text: "আমি".into(),
                    bypass_key: true,
                },
                "keyval={keyval:#x}"
            );
            assert!(engine.is_empty());
        }
    }
}
