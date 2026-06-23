use bornika_phonetic::PhoneticEngine;
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
        let is_release = (state & (1 << 30)) != 0;
        let bangla_mode = {
            let engine = self.engine.lock().unwrap();
            engine.bangla_mode
        };

        let key = if keyval == 0xFF08 {
            bornika_phonetic::VirtualKey::Backspace
        } else if keyval == 0x20 {
            bornika_phonetic::VirtualKey::Space
        } else if keyval == 0xFF0D || keyval == 0xFF8D {
            bornika_phonetic::VirtualKey::Enter
        } else if (0x20..=0x7E).contains(&keyval) {
            bornika_phonetic::VirtualKey::Char(keyval as u8 as char)
        } else {
            bornika_phonetic::VirtualKey::Other
        };

        let event = bornika_phonetic::KeyEvent {
            key,
            ctrl: (state & 4) != 0,
            alt: (state & 8) != 0,
            shift: (state & 1) != 0,
            is_release,
        };

        log_info(&format!(
            "Engine: ProcessKeyEvent (keyval: 0x{:X}, keycode: {}, state: 0x{:X}, is_release: {}, bangla_mode: {})",
            keyval, keycode, state, is_release, bangla_mode
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
