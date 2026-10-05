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
pub fn create_ibus_text(text: &str, underline: bool) -> Value<'static> {
    let mut attr_properties = Vec::<Value<'static>>::new();

    if underline && !text.is_empty() {
        // IBus attribute indices are unsigned Unicode-character offsets, not UTF-8 bytes.
        let attr_attachments = HashMap::<String, Value<'static>>::new();
        let attr_struct = (
            "IBusAttribute",
            attr_attachments,
            1u32, // IBUS_ATTR_TYPE_UNDERLINE
            1u32, // IBUS_ATTR_UNDERLINE_SINGLE
            0u32,
            text.chars().count() as u32,
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
    let mut key = match keyval {
        0xFF1B => VirtualKey::Escape,
        0xFF09 => VirtualKey::Tab,
        0xFF51 | 0xFF96 => VirtualKey::Left,
        0xFF53 | 0xFF98 => VirtualKey::Right,
        0xFF50 | 0xFF95 => VirtualKey::Home,
        0xFF57 | 0xFF9C => VirtualKey::End,
        0xFF08 => VirtualKey::Backspace,
        0xFFFF | 0xFF9F => VirtualKey::Delete,
        0x20 => VirtualKey::Space,
        0xFF0D | 0xFF8D => VirtualKey::Enter,
        0x21..=0x7E => VirtualKey::Char(keyval as u8 as char),
        _ => VirtualKey::Other,
    };
    // KeyEvent has no Mod3/Mod4/Mod5 or Super/Hyper/Meta fields; preserve these shortcuts.
    if key.is_composition_control()
        && state & ((1 << 5) | (1 << 6) | (1 << 7) | (1 << 26) | (1 << 27) | (1 << 28)) != 0
    {
        key = VirtualKey::Other;
    }

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
            engine: Mutex::new(PhoneticEngine::default()),
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
            let empty_text = create_ibus_text("", false);
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
                let empty_text = create_ibus_text("", false);
                let _ = Self::update_preedit_text(&ctxt, empty_text, 0, false, 0).await;
                true
            }
            bornika_phonetic::KeyAction::Commit { text, bypass_key } => {
                let empty_text = create_ibus_text("", false);
                let _ = Self::update_preedit_text(&ctxt, empty_text, 0, false, 0).await;
                let val_text = create_ibus_text(&text, false);
                let _ = Self::commit_text(&ctxt, val_text).await;
                !bypass_key
            }
            bornika_phonetic::KeyAction::UpdatePreedit {
                text,
                cursor_pos,
                visible,
            } => {
                let styled_text = create_ibus_text(&text, true);
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

    fn assert_ibus_text(value: &Value<'_>, expected: &str, underline: bool) {
        let Value::Structure(text) = value else {
            panic!("expected IBusText structure: {value:?}");
        };
        assert_eq!(text.signature().as_str(), "(sa{sv}sv)");
        assert_eq!(text.fields()[0].downcast_ref::<str>(), Some("IBusText"));
        assert_eq!(text.fields()[2].downcast_ref::<str>(), Some(expected));
        let Value::Value(attrs) = &text.fields()[3] else {
            panic!("expected attribute-list variant");
        };
        let Value::Structure(attrs) = attrs.as_ref() else {
            panic!("expected IBusAttrList structure");
        };
        assert_eq!(attrs.signature().as_str(), "(sa{sv}av)");
        assert_eq!(
            attrs.fields()[0].downcast_ref::<str>(),
            Some("IBusAttrList")
        );
        let Value::Array(properties) = &attrs.fields()[2] else {
            panic!("expected attribute array");
        };
        if !underline || expected.is_empty() {
            assert!(properties.is_empty());
            return;
        }
        assert_eq!(properties.len(), 1);
        let Value::Value(attribute) = &properties[0] else {
            panic!("expected attribute variant");
        };
        let Value::Structure(attribute) = attribute.as_ref() else {
            panic!("expected IBusAttribute structure");
        };
        assert_eq!(attribute.signature().as_str(), "(sa{sv}uuuu)");
        assert_eq!(
            attribute.fields()[0].downcast_ref::<str>(),
            Some("IBusAttribute")
        );
        for (field, expected) in
            attribute.fields()[2..]
                .iter()
                .zip([1, 1, 0, expected.chars().count() as u32])
        {
            assert_eq!(field.downcast_ref::<u32>(), Some(&expected));
        }
    }

    #[test]
    fn test_ibus_text_attributes_use_character_offsets_and_unsigned_indices() {
        for text in ["", "ami", "আমি", "ক🙂খ"] {
            for underline in [false, true] {
                assert_ibus_text(&create_ibus_text(text, underline), text, underline);
            }
        }
    }

    #[tokio::test]
    async fn test_engine_emits_preedit_commit_and_cancel_signals() {
        use std::time::Duration;
        use tokio::net::UnixStream;
        use zbus::export::futures_util::TryStreamExt;
        use zbus::zvariant::OwnedValue;
        use zbus::{ConnectionBuilder, Guid, MatchRule, MessageStream, MessageType};

        tokio::time::timeout(Duration::from_secs(5), async {
            let path = "/org/freedesktop/IBus/Engine/bornika";
            let interface = "org.freedesktop.IBus.Engine";
            let guid = Guid::generate();
            let (server_socket, client_socket) = UnixStream::pair().unwrap();
            let server = ConnectionBuilder::unix_stream(server_socket)
                .server(&guid)
                .p2p()
                .serve_at(path, IBusEngine::new())
                .unwrap()
                .build();
            let client = ConnectionBuilder::unix_stream(client_socket).p2p().build();
            let (server, client) = tokio::try_join!(server, client).unwrap();
            let rule = MatchRule::builder()
                .msg_type(MessageType::Signal)
                .interface(interface)
                .unwrap()
                .path(path)
                .unwrap()
                .build();
            let mut signals = MessageStream::for_match_rule(rule, &client, None)
                .await
                .unwrap();

            let cases = [
                (
                    b'k' as u32,
                    0u32,
                    true,
                    vec![("UpdatePreeditText", "ক", 1, true)],
                ),
                (
                    b'h' as u32,
                    0,
                    true,
                    vec![("UpdatePreeditText", "খ", 1, true)],
                ),
                (0xFF51, 0, true, vec![("UpdatePreeditText", "খ", 0, true)]),
                (0xFF51, 1 << 30, false, vec![]),
                (0xFF53, 0, true, vec![("UpdatePreeditText", "খ", 1, true)]),
                (
                    0xFF09,
                    0,
                    true,
                    vec![
                        ("UpdatePreeditText", "", 0, false),
                        ("CommitText", "খ", 0, false),
                    ],
                ),
                (0xFF09, 1 << 30, false, vec![]),
                (
                    b'a' as u32,
                    0,
                    true,
                    vec![("UpdatePreeditText", "আ", 1, true)],
                ),
                (
                    b'm' as u32,
                    0,
                    true,
                    vec![("UpdatePreeditText", "আম", 2, true)],
                ),
                (
                    b'i' as u32,
                    0,
                    true,
                    vec![("UpdatePreeditText", "আমি", 3, true)],
                ),
                (0xFF50, 0, true, vec![("UpdatePreeditText", "আমি", 0, true)]),
                (0xFFFF, 0, true, vec![("UpdatePreeditText", "মি", 0, true)]),
                (
                    b'a' as u32,
                    0,
                    true,
                    vec![("UpdatePreeditText", "আমি", 1, true)],
                ),
                (0xFF53, 0, true, vec![("UpdatePreeditText", "আমি", 2, true)]),
                (0xFF08, 0, true, vec![("UpdatePreeditText", "আই", 1, true)]),
                (
                    b'm' as u32,
                    0,
                    true,
                    vec![("UpdatePreeditText", "আমি", 2, true)],
                ),
                (0xFF9F, 0, true, vec![("UpdatePreeditText", "আম", 2, true)]),
                (0xFF1B, 0, true, vec![("UpdatePreeditText", "", 0, false)]),
                (0xFF1B, 1 << 30, false, vec![]),
                (0xFF09, 0, false, vec![]),
            ];

            for (keyval, state, handled, expected) in cases {
                let reply = client
                    .call_method(
                        None::<&str>,
                        path,
                        Some(interface),
                        "ProcessKeyEvent",
                        &(keyval, 0u32, state),
                    )
                    .await
                    .unwrap();
                assert_eq!(reply.body::<bool>().unwrap(), handled);
                // A same-connection marker proves no extra signals escaped each event.
                server
                    .emit_signal(None::<&str>, path, interface, "TestBarrier", &())
                    .await
                    .unwrap();

                for (member, text, cursor, visible) in expected {
                    let message = signals.try_next().await.unwrap().unwrap();
                    assert_eq!(message.member().unwrap().as_str(), member);
                    if member == "CommitText" {
                        let value = message.body::<OwnedValue>().unwrap();
                        assert_ibus_text(&value, text, false);
                    } else {
                        let (value, actual_cursor, actual_visible, mode) =
                            message.body::<(OwnedValue, u32, bool, u32)>().unwrap();
                        assert_ibus_text(&value, text, visible);
                        assert_eq!((actual_cursor, actual_visible, mode), (cursor, visible, 0));
                    }
                }
                let barrier = signals.try_next().await.unwrap().unwrap();
                assert_eq!(barrier.member().unwrap().as_str(), "TestBarrier");
            }
        })
        .await
        .expect("local D-Bus signal test timed out");
    }

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
                let mut engine = PhoneticEngine::default();
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
        for keyval in [0xFF08, 0xFFFF, 0xFF9F, 0xFF1B, 0xFF09, 0xFE20] {
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
                let mut engine = PhoneticEngine::default();
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
            let mut engine = PhoneticEngine::default();
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

    #[test]
    fn test_ibus_navigation_and_keypad_aliases() {
        for (keyvals, key, start, cursor_pos) in [
            ([0xFF51, 0xFF96], VirtualKey::Left, VirtualKey::End, 2),
            ([0xFF53, 0xFF98], VirtualKey::Right, VirtualKey::Home, 1),
            ([0xFF50, 0xFF95], VirtualKey::Home, VirtualKey::End, 0),
            ([0xFF57, 0xFF9C], VirtualKey::End, VirtualKey::Home, 3),
        ] {
            for keyval in keyvals {
                for state in [0, 2, 16, 18] {
                    let mut engine = PhoneticEngine::default();
                    engine.set_buffer("ami".into());
                    let event = decode_key_event(keyval, state);
                    engine.process_key_event(KeyEvent {
                        key: start,
                        ..event
                    });
                    assert_eq!(event.key, key);
                    assert_eq!(
                        engine.process_key_event(event),
                        KeyAction::UpdatePreedit {
                            text: "আমি".into(),
                            cursor_pos,
                            visible: true,
                        },
                        "keyval={keyval:#x}, state={state:#x}"
                    );
                    assert_eq!(engine.get_buffer(), "ami");
                }
            }
        }
    }

    #[test]
    fn test_ibus_modified_navigation_and_releases_bypass() {
        for keyval in [
            0xFF51, 0xFF96, 0xFF53, 0xFF98, 0xFF50, 0xFF95, 0xFF57, 0xFF9C,
        ] {
            for state in [
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
                let mut engine = PhoneticEngine::default();
                engine.set_buffer("ami".into());
                assert_eq!(
                    engine.process_key_event(decode_key_event(keyval, state)),
                    KeyAction::Bypass,
                    "keyval={keyval:#x}, state={state:#x}"
                );
                assert_eq!(engine.get_buffer(), "ami");
                assert!(engine.bangla_mode);
            }
        }
    }

    #[test]
    fn test_ibus_delete_and_keypad_delete_edit_at_cursor() {
        for keyval in [0xFFFF, 0xFF9F] {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer("ami".into());
            engine.process_key_event(decode_key_event(0xFF50, 0));
            let event = decode_key_event(keyval, 0);
            assert_eq!(event.key, VirtualKey::Delete);
            assert_eq!(
                engine.process_key_event(event),
                KeyAction::UpdatePreedit {
                    text: "মি".into(),
                    cursor_pos: 0,
                    visible: true,
                }
            );
            assert_eq!(engine.get_buffer(), "mi");
        }
    }
}
