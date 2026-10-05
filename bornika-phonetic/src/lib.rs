mod utils;

pub mod rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualKey {
    Char(char),
    Backspace,
    Delete,
    Escape,
    Tab,
    Left,
    Right,
    Home,
    End,
    Space,
    Enter,
    Other,
}

impl VirtualKey {
    pub fn is_composition_control(self) -> bool {
        matches!(
            self,
            Self::Backspace
                | Self::Delete
                | Self::Escape
                | Self::Tab
                | Self::Left
                | Self::Right
                | Self::Home
                | Self::End
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: VirtualKey,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub is_release: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Bypass,
    Swallow,
    Commit {
        text: String,
        bypass_key: bool,
    },
    UpdatePreedit {
        text: String,
        cursor_pos: u32,
        visible: bool,
    },
    ToggleMode {
        bangla_mode: bool,
    },
}

fn is_composition_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '`' || c == '.' || c == '^' || c == ':' || c == ','
}

fn is_commit_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() && c != '`' && c != '.' && c != '^' && c != ':' && c != ','
}

#[derive(Debug, Clone)]
pub struct PhoneticEngine {
    composition_buffer: String,
    // Source byte offset, always on a UTF-8 boundary; rendered cursor counts characters.
    source_cursor: usize,
    pub bangla_mode: bool,
}

impl Default for PhoneticEngine {
    fn default() -> Self {
        Self {
            composition_buffer: String::new(),
            source_cursor: 0,
            bangla_mode: true,
        }
    }
}

impl PhoneticEngine {
    pub fn get_buffer(&self) -> &str {
        &self.composition_buffer
    }

    pub fn set_buffer(&mut self, val: String) {
        self.source_cursor = val.len();
        self.composition_buffer = val;
    }

    pub fn clear(&mut self) {
        self.composition_buffer.clear();
        self.source_cursor = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.composition_buffer.is_empty()
    }

    pub fn translate(&self) -> String {
        translate(&self.composition_buffer)
    }

    pub fn process_key_event(&mut self, event: KeyEvent) -> KeyAction {
        if event.alt
            || (event.ctrl && event.key != VirtualKey::Space)
            || (event.shift && event.key.is_composition_control())
        {
            return KeyAction::Bypass;
        }

        if event.is_release {
            if self.bangla_mode && !self.is_empty() {
                if let VirtualKey::Char(c) = event.key {
                    if is_composition_char(c) {
                        return KeyAction::Swallow;
                    }
                }
            }
            return KeyAction::Bypass;
        }

        if event.ctrl && event.key == VirtualKey::Space {
            self.bangla_mode = !self.bangla_mode;
            let cleared = !self.is_empty();
            if cleared {
                self.clear();
            }
            return KeyAction::ToggleMode {
                bangla_mode: self.bangla_mode,
            };
        }

        if !self.bangla_mode {
            return KeyAction::Bypass;
        }

        if matches!(
            event.key,
            VirtualKey::Space | VirtualKey::Enter | VirtualKey::Tab
        ) || matches!(event.key, VirtualKey::Char(c) if is_commit_punctuation(c))
        {
            if self.is_empty() {
                return KeyAction::Bypass;
            }
            let text = self.translate();
            self.clear();
            return KeyAction::Commit {
                text,
                bypass_key: event.key != VirtualKey::Tab,
            };
        }

        match event.key {
            VirtualKey::Left | VirtualKey::Backspace if !self.is_empty() => {
                let previous = self.source_cursor;
                self.source_cursor = self.composition_buffer[..previous]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(offset, _)| offset);
                if event.key == VirtualKey::Backspace {
                    self.composition_buffer
                        .replace_range(self.source_cursor..previous, "");
                }
            }
            VirtualKey::Delete if !self.is_empty() => {
                if self.source_cursor < self.composition_buffer.len() {
                    self.composition_buffer.remove(self.source_cursor);
                }
            }
            VirtualKey::Escape => {
                if self.is_empty() {
                    return KeyAction::Bypass;
                }
                self.clear();
            }
            VirtualKey::Char(c) if is_composition_char(c) => {
                self.composition_buffer.insert(self.source_cursor, c);
                self.source_cursor += c.len_utf8();
            }
            VirtualKey::Right if !self.is_empty() => {
                if let Some(c) = self.composition_buffer[self.source_cursor..].chars().next() {
                    self.source_cursor += c.len_utf8();
                }
            }
            VirtualKey::Home if !self.is_empty() => self.source_cursor = 0,
            VirtualKey::End if !self.is_empty() => {
                self.source_cursor = self.composition_buffer.len();
            }
            _ => return KeyAction::Bypass,
        }

        let mut preedit = String::new();
        let mut source_end = 0;
        let mut cursor_pos = 0;
        for (source_len, text) in TranslationTokens::new(&self.composition_buffer) {
            source_end += source_len;
            // Inside a multi-character match, show the caret at its leading output boundary.
            if source_end <= self.source_cursor {
                cursor_pos += text.chars().count() as u32;
            }
            preedit.push_str(text);
        }
        // A typed backtick can keep composition active without producing visible text.
        let visible = !preedit.is_empty() || matches!(event.key, VirtualKey::Char(_));
        KeyAction::UpdatePreedit {
            text: preedit,
            cursor_pos,
            visible,
        }
    }
}

struct TranslationTokens<'a> {
    remaining: &'a str,
    can_take_dependent_vowel: bool,
    can_form_conjunct: bool,
}

impl<'a> TranslationTokens<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            remaining: input,
            can_take_dependent_vowel: false,
            can_form_conjunct: false,
        }
    }
}

impl<'a> Iterator for TranslationTokens<'a> {
    // Each item carries the consumed source byte length and its rendered text.
    type Item = (usize, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.remaining.chars().next()?;
        let (source_len, text, can_form_conjunct, can_take_dependent_vowel) = if let Some(rule) =
            rules::RULES
                .iter()
                .find(|rule| self.remaining.starts_with(rule.roman))
        {
            self.remaining = &self.remaining[rule.roman.len()..];
            let (text, can_form_conjunct, can_take_dependent_vowel) = match rule.token_type {
                rules::TokenType::Vowel {
                    independent,
                    dependent,
                } => (
                    if self.can_take_dependent_vowel {
                        dependent
                    } else {
                        independent
                    },
                    false,
                    false,
                ),
                rules::TokenType::Consonant(forms) => {
                    if self.can_form_conjunct {
                        (
                            forms.after_consonant,
                            forms.chain_after_consonant == utils::Chain::Allows,
                            forms.vowel_link_after_consonant == utils::VowelLink::Allows,
                        )
                    } else {
                        (
                            forms.after_vowel,
                            forms.chain_after_vowel == utils::Chain::Allows,
                            forms.vowel_link_after_vowel == utils::VowelLink::Allows,
                        )
                    }
                }
                rules::TokenType::Exact(text)
                | rules::TokenType::Sign(text)
                | rules::TokenType::Punctuation(text) => (text, false, false),
                rules::TokenType::ForceSeparate => ("", false, false),
            };
            (
                rule.roman.len(),
                text,
                can_form_conjunct,
                can_take_dependent_vowel,
            )
        } else {
            let (text, remaining) = self.remaining.split_at(first.len_utf8());
            self.remaining = remaining;
            (first.len_utf8(), text, false, false)
        };

        self.can_form_conjunct = can_form_conjunct;
        self.can_take_dependent_vowel = can_take_dependent_vowel;
        Some((source_len, text))
    }
}

/// Core translation algorithm that parses a Romanized input string into Bengali Unicode
pub fn translate(input: &str) -> String {
    let mut output = String::new();
    for (_, text) in TranslationTokens::new(input) {
        output.push_str(text);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_clamps_at_composition_boundaries() {
        let mut engine = PhoneticEngine::default();
        engine.set_buffer("ami".into());
        for (key, expected_cursor) in [
            (VirtualKey::Left, 2),
            (VirtualKey::Left, 1),
            (VirtualKey::Left, 0),
            (VirtualKey::Left, 0),
            (VirtualKey::Right, 1),
            (VirtualKey::Right, 2),
            (VirtualKey::Right, 3),
            (VirtualKey::Right, 3),
            (VirtualKey::Home, 0),
            (VirtualKey::Home, 0),
            (VirtualKey::End, 3),
            (VirtualKey::End, 3),
        ] {
            assert_eq!(
                engine.process_key_event(KeyEvent {
                    key,
                    ctrl: false,
                    alt: false,
                    shift: false,
                    is_release: false,
                }),
                KeyAction::UpdatePreedit {
                    text: "আমি".into(),
                    cursor_pos: expected_cursor,
                    visible: true,
                }
            );
            assert_eq!(engine.source_cursor, expected_cursor as usize);
            assert_eq!(engine.get_buffer(), "ami");
        }
    }

    #[test]
    fn test_navigation_maps_complete_phonetic_tokens() {
        let cases: &[(&str, &str, &[u32])] = &[
            ("kh", "খ", &[0, 0, 1]),
            ("rri", "ঋ", &[0, 0, 0, 1]),
            ("kkh", "ক্ষ", &[0, 0, 0, 3]),
            ("kt", "ক্ত", &[0, 1, 3]),
            ("ka", "কা", &[0, 1, 2]),
            ("ko", "ক", &[0, 1, 1]),
            ("kOI", "কৈ", &[0, 1, 1, 2]),
            ("kwa", "ক্বা", &[0, 1, 1, 4]),
            ("wa", "ওয়া", &[0, 0, 3]),
            ("kya", "ক্যা", &[0, 1, 3, 4]),
            ("k`kh", "কখ", &[0, 1, 1, 1, 2]),
            ("k,,k", "ক্ক", &[0, 1, 1, 2, 3]),
            ("ami..", "আমি.", &[0, 1, 2, 3, 3, 4]),
            ("k1a", "ক১আ", &[0, 1, 2, 3]),
            ("ক🙂kh", "ক🙂খ", &[0, 1, 2, 2, 3]),
            ("`", "", &[0, 0]),
            (
                "vorrt`sonapUrrNo",
                "ভর্ৎসনাপূর্ণ",
                &[0, 1, 1, 1, 2, 2, 4, 5, 5, 6, 7, 8, 9, 9, 10, 12, 12],
            ),
        ];
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };

        for &(input, expected, positions) in cases {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer(input.into());
            assert_eq!(positions.len(), input.chars().count() + 1);
            assert_eq!(translate(input), expected);
            assert_eq!(
                engine.process_key_event(home),
                KeyAction::UpdatePreedit {
                    text: expected.into(),
                    cursor_pos: 0,
                    visible: !expected.is_empty(),
                }
            );

            let steps = positions
                .iter()
                .enumerate()
                .skip(1)
                .map(|(source, &display)| (VirtualKey::Right, source, display))
                .chain(
                    positions
                        .iter()
                        .enumerate()
                        .rev()
                        .skip(1)
                        .map(|(source, &display)| (VirtualKey::Left, source, display)),
                );
            for (key, source, display) in steps {
                assert_eq!(
                    engine.process_key_event(KeyEvent { key, ..home }),
                    KeyAction::UpdatePreedit {
                        text: expected.into(),
                        cursor_pos: display,
                        visible: !expected.is_empty(),
                    },
                    "input={input:?}, key={key:?}, source_cursor={source}"
                );
                let byte_offset = input
                    .char_indices()
                    .map(|(offset, _)| offset)
                    .chain(std::iter::once(input.len()))
                    .nth(source)
                    .unwrap();
                assert_eq!(engine.source_cursor, byte_offset);
                assert_eq!(engine.get_buffer(), input);
            }
        }
    }

    #[test]
    fn test_buffer_set_and_clear_keep_byte_cursor_in_sync() {
        let mut engine = PhoneticEngine::default();
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        assert_eq!(engine.source_cursor, 0);
        engine.set_buffer("ক🙂kh".into());
        assert_eq!(engine.source_cursor, "ক🙂kh".len());
        engine.process_key_event(home);
        assert_eq!(engine.source_cursor, 0);
        engine.set_buffer("🙂".into());
        assert_eq!(engine.source_cursor, "🙂".len());

        engine.set_buffer("ami".into());
        engine.process_key_event(home);
        engine.clear();
        assert!(engine.is_empty());
        assert_eq!(engine.source_cursor, 0);
        engine.set_buffer(String::new());
        assert_eq!(engine.source_cursor, 0);
    }

    #[test]
    fn test_navigation_bypasses_inactive_composition() {
        for (bangla_mode, buffer) in [(true, ""), (false, ""), (false, "ami")] {
            for key in [
                VirtualKey::Left,
                VirtualKey::Right,
                VirtualKey::Home,
                VirtualKey::End,
            ] {
                let mut engine = PhoneticEngine {
                    bangla_mode,
                    ..PhoneticEngine::default()
                };
                engine.set_buffer(buffer.into());
                assert_eq!(
                    engine.process_key_event(KeyEvent {
                        key,
                        ctrl: false,
                        alt: false,
                        shift: false,
                        is_release: false,
                    }),
                    KeyAction::Bypass
                );
                assert_eq!(engine.get_buffer(), buffer);
                assert_eq!(engine.source_cursor, buffer.len());
                assert_eq!(engine.bangla_mode, bangla_mode);
            }
        }
    }

    #[test]
    fn test_modified_navigation_preserves_composition_and_cursor() {
        for key in [
            VirtualKey::Left,
            VirtualKey::Right,
            VirtualKey::Home,
            VirtualKey::End,
        ] {
            for (ctrl, alt, shift) in [
                (true, false, false),
                (false, true, false),
                (false, false, true),
                (true, true, true),
            ] {
                let mut engine = PhoneticEngine::default();
                engine.set_buffer("ami".into());
                assert_eq!(
                    engine.process_key_event(KeyEvent {
                        key,
                        ctrl,
                        alt,
                        shift,
                        is_release: false,
                    }),
                    KeyAction::Bypass
                );
                assert_eq!(engine.source_cursor, 3);
                assert_eq!(engine.get_buffer(), "ami");
            }
        }
    }

    #[test]
    fn test_navigation_releases_do_not_move_cursor() {
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        for key in [
            VirtualKey::Left,
            VirtualKey::Right,
            VirtualKey::Home,
            VirtualKey::End,
        ] {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer("ami".into());
            engine.process_key_event(home);
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Right,
                ..home
            });
            assert_eq!(engine.source_cursor, 1);
            assert_eq!(
                engine.process_key_event(KeyEvent {
                    key,
                    is_release: true,
                    ..home
                }),
                KeyAction::Bypass
            );
            assert_eq!(engine.source_cursor, 1);
            assert_eq!(engine.get_buffer(), "ami");
        }
    }

    #[test]
    fn test_commit_cancel_and_toggle_reset_navigation_cursor() {
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        for (key, ctrl, expected) in [
            (
                VirtualKey::Tab,
                false,
                KeyAction::Commit {
                    text: "ঋ".into(),
                    bypass_key: false,
                },
            ),
            (
                VirtualKey::Space,
                false,
                KeyAction::Commit {
                    text: "ঋ".into(),
                    bypass_key: true,
                },
            ),
            (
                VirtualKey::Enter,
                false,
                KeyAction::Commit {
                    text: "ঋ".into(),
                    bypass_key: true,
                },
            ),
            (
                VirtualKey::Char('!'),
                false,
                KeyAction::Commit {
                    text: "ঋ".into(),
                    bypass_key: true,
                },
            ),
            (
                VirtualKey::Escape,
                false,
                KeyAction::UpdatePreedit {
                    text: String::new(),
                    cursor_pos: 0,
                    visible: false,
                },
            ),
            (
                VirtualKey::Space,
                true,
                KeyAction::ToggleMode { bangla_mode: false },
            ),
        ] {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer("rri".into());
            engine.process_key_event(home);
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Right,
                ..home
            });
            assert_eq!(engine.source_cursor, 1);
            assert_eq!(
                engine.process_key_event(KeyEvent { key, ctrl, ..home }),
                expected
            );
            assert!(engine.is_empty());
            assert_eq!(engine.source_cursor, 0);
        }
    }

    #[test]
    fn test_typing_and_backspace_edit_at_navigation_cursor() {
        let mut engine = PhoneticEngine::default();
        engine.set_buffer("ami".into());
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        engine.process_key_event(home);
        assert_eq!(
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Char('k'),
                ..home
            }),
            KeyAction::UpdatePreedit {
                text: "কামি".into(),
                cursor_pos: 1,
                visible: true,
            }
        );
        assert_eq!(engine.get_buffer(), "kami");
        assert_eq!(engine.source_cursor, 1);

        assert_eq!(
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Backspace,
                ..home
            }),
            KeyAction::UpdatePreedit {
                text: "আমি".into(),
                cursor_pos: 0,
                visible: true,
            }
        );
        assert_eq!(engine.get_buffer(), "ami");
        assert_eq!(engine.source_cursor, 0);
    }

    #[test]
    fn test_cursor_edits_retranslate_tokens_and_preserve_unicode_boundaries() {
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        for (input, position, key, expected_buffer, expected_text, display, bytes) in [
            ("ka", 1, VirtualKey::Char('h'), "kha", "খা", 1, 2),
            ("kh", 1, VirtualKey::Char('`'), "k`h", "কহ", 1, 2),
            ("k`h", 2, VirtualKey::Backspace, "kh", "খ", 0, 1),
            ("kh", 1, VirtualKey::Backspace, "h", "হ", 0, 0),
            ("kha", 1, VirtualKey::Delete, "ka", "কা", 1, 1),
            ("rri", 1, VirtualKey::Delete, "ri", "রি", 1, 1),
            ("rri", 2, VirtualKey::Backspace, "ri", "রি", 1, 1),
            ("rri", 2, VirtualKey::Delete, "rr", "র", 1, 2),
            ("kt", 1, VirtualKey::Delete, "k", "ক", 1, 1),
            ("k,,k", 2, VirtualKey::Delete, "k,k", "ক,ক", 2, 2),
            ("ক🙂kh", 1, VirtualKey::Delete, "কkh", "কখ", 1, 3),
            ("ক🙂kh", 2, VirtualKey::Backspace, "কkh", "কখ", 1, 3),
            ("ক🙂kh", 1, VirtualKey::Char('a'), "কa🙂kh", "কআ🙂খ", 2, 4),
            ("🙂", 0, VirtualKey::Delete, "", "", 0, 0),
            ("🙂", 1, VirtualKey::Backspace, "", "", 0, 0),
            ("kh", 2, VirtualKey::Char('a'), "kha", "খা", 2, 3),
            ("ami", 3, VirtualKey::Delete, "ami", "আমি", 3, 3),
            ("ami", 0, VirtualKey::Backspace, "ami", "আমি", 0, 0),
            ("`", 0, VirtualKey::Delete, "", "", 0, 0),
        ] {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer(input.into());
            engine.process_key_event(home);
            for _ in 0..position {
                engine.process_key_event(KeyEvent {
                    key: VirtualKey::Right,
                    ..home
                });
            }
            assert_eq!(
                engine.process_key_event(KeyEvent { key, ..home }),
                KeyAction::UpdatePreedit {
                    text: expected_text.into(),
                    cursor_pos: display,
                    visible: !expected_text.is_empty(),
                },
                "input={input:?}, position={position}, key={key:?}"
            );
            assert_eq!(engine.get_buffer(), expected_buffer);
            assert_eq!(engine.source_cursor, bytes);
            assert!(engine
                .composition_buffer
                .is_char_boundary(engine.source_cursor));
        }
    }

    #[test]
    fn test_cursor_edits_commit_the_complete_word_and_reset() {
        let mut engine = PhoneticEngine::default();
        engine.set_buffer("kh".into());
        let event = KeyEvent {
            key: VirtualKey::Left,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        for key in [
            VirtualKey::Left,
            VirtualKey::Char('`'),
            VirtualKey::Backspace,
            VirtualKey::End,
            VirtualKey::Char('a'),
        ] {
            engine.process_key_event(KeyEvent { key, ..event });
        }
        assert_eq!(engine.get_buffer(), "kha");
        assert_eq!(
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Tab,
                ..event
            }),
            KeyAction::Commit {
                text: "খা".into(),
                bypass_key: false,
            }
        );
        assert!(engine.is_empty());
        assert_eq!(engine.source_cursor, 0);
        assert_eq!(
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Char('g'),
                ..event
            }),
            KeyAction::UpdatePreedit {
                text: "গ".into(),
                cursor_pos: 1,
                visible: true,
            }
        );
    }

    #[test]
    fn test_cursor_edits_match_character_model_at_every_utf8_boundary() {
        let home = KeyEvent {
            key: VirtualKey::Home,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        for input in ["", "ami", "rri", "ক🙂kh", "e\u{301}kh"] {
            let source: Vec<char> = input.chars().collect();
            for position in 0..=source.len() {
                for key in [
                    VirtualKey::Char('a'),
                    VirtualKey::Backspace,
                    VirtualKey::Delete,
                ] {
                    let mut engine = PhoneticEngine::default();
                    engine.set_buffer(input.into());
                    engine.process_key_event(home);
                    for _ in 0..position {
                        engine.process_key_event(KeyEvent {
                            key: VirtualKey::Right,
                            ..home
                        });
                    }
                    let mut expected = source.clone();
                    let mut cursor = position;
                    match key {
                        VirtualKey::Char(c) => {
                            expected.insert(cursor, c);
                            cursor += 1;
                        }
                        VirtualKey::Backspace if cursor > 0 => {
                            cursor -= 1;
                            expected.remove(cursor);
                        }
                        VirtualKey::Delete if cursor < expected.len() => {
                            expected.remove(cursor);
                        }
                        _ => {}
                    }
                    engine.process_key_event(KeyEvent { key, ..home });
                    assert_eq!(engine.get_buffer(), expected.iter().collect::<String>());
                    assert_eq!(
                        engine.source_cursor,
                        expected[..cursor]
                            .iter()
                            .map(|c| c.len_utf8())
                            .sum::<usize>()
                    );
                    assert!(engine
                        .composition_buffer
                        .is_char_boundary(engine.source_cursor));
                }
            }
        }
    }

    #[test]
    fn test_escape_cancels_composition() {
        let escape = KeyEvent {
            key: VirtualKey::Escape,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };

        for input in ["ami", "`"] {
            let mut engine = PhoneticEngine::default();
            for c in input.chars() {
                engine.process_key_event(KeyEvent {
                    key: VirtualKey::Char(c),
                    ..escape
                });
            }
            assert_eq!(engine.get_buffer(), input);
            assert_eq!(
                engine.process_key_event(escape),
                KeyAction::UpdatePreedit {
                    text: String::new(),
                    cursor_pos: 0,
                    visible: false,
                }
            );
            assert!(engine.is_empty());
            assert!(engine.bangla_mode);
            assert_eq!(engine.process_key_event(escape), KeyAction::Bypass);

            assert_eq!(
                engine.process_key_event(KeyEvent {
                    key: VirtualKey::Char('k'),
                    ..escape
                }),
                KeyAction::UpdatePreedit {
                    text: "ক".into(),
                    cursor_pos: 1,
                    visible: true,
                }
            );
        }
    }

    #[test]
    fn test_tab_commits_composition_once() {
        let tab = KeyEvent {
            key: VirtualKey::Tab,
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };

        for (input, expected) in [("ami", "আমি"), ("vorrt`sonapUrrNo", "ভর্ৎসনাপূর্ণ"), ("`", "")]
        {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer(input.into());
            assert_eq!(
                engine.process_key_event(tab),
                KeyAction::Commit {
                    text: expected.into(),
                    bypass_key: false,
                }
            );
            assert!(engine.is_empty());
            assert!(engine.bangla_mode);
            assert_eq!(engine.process_key_event(tab), KeyAction::Bypass);
            assert_eq!(
                engine.process_key_event(KeyEvent {
                    is_release: true,
                    ..tab
                }),
                KeyAction::Bypass
            );
        }
    }

    #[test]
    fn test_composition_controls_bypass_when_inactive() {
        for (bangla_mode, buffer) in [(true, ""), (false, ""), (false, "ami")] {
            for key in [
                VirtualKey::Backspace,
                VirtualKey::Delete,
                VirtualKey::Escape,
                VirtualKey::Tab,
            ] {
                let mut engine = PhoneticEngine {
                    bangla_mode,
                    ..PhoneticEngine::default()
                };
                engine.set_buffer(buffer.into());
                assert_eq!(
                    engine.process_key_event(KeyEvent {
                        key,
                        ctrl: false,
                        alt: false,
                        shift: false,
                        is_release: false,
                    }),
                    KeyAction::Bypass
                );
                assert_eq!(engine.get_buffer(), buffer);
                assert_eq!(engine.bangla_mode, bangla_mode);
            }
        }
    }

    #[test]
    fn test_modified_composition_controls_bypass() {
        for key in [
            VirtualKey::Backspace,
            VirtualKey::Delete,
            VirtualKey::Escape,
            VirtualKey::Tab,
        ] {
            for (ctrl, alt, shift) in [
                (true, false, false),
                (false, true, false),
                (false, false, true),
                (true, true, true),
            ] {
                let mut engine = PhoneticEngine::default();
                engine.set_buffer("ami".into());
                assert_eq!(
                    engine.process_key_event(KeyEvent {
                        key,
                        ctrl,
                        alt,
                        shift,
                        is_release: false,
                    }),
                    KeyAction::Bypass
                );
                assert_eq!(engine.get_buffer(), "ami");
                assert!(engine.bangla_mode);
            }
        }
    }

    #[test]
    fn test_composition_control_releases_do_not_modify_buffer() {
        for key in [
            VirtualKey::Backspace,
            VirtualKey::Delete,
            VirtualKey::Escape,
            VirtualKey::Tab,
        ] {
            for buffer in ["", "ami"] {
                let mut engine = PhoneticEngine::default();
                engine.set_buffer(buffer.into());
                assert_eq!(
                    engine.process_key_event(KeyEvent {
                        key,
                        ctrl: false,
                        alt: false,
                        shift: false,
                        is_release: true,
                    }),
                    KeyAction::Bypass
                );
                assert_eq!(engine.get_buffer(), buffer);
                assert!(engine.bangla_mode);
            }
        }
    }

    #[test]
    fn test_space_enter_and_punctuation_still_bypass_commit_key() {
        for key in [VirtualKey::Space, VirtualKey::Enter, VirtualKey::Char('!')] {
            let mut engine = PhoneticEngine::default();
            engine.set_buffer("ami".into());
            let event = KeyEvent {
                key,
                ctrl: false,
                alt: false,
                shift: false,
                is_release: false,
            };
            assert_eq!(
                engine.process_key_event(event),
                KeyAction::Commit {
                    text: "আমি".into(),
                    bypass_key: true,
                }
            );
            assert!(engine.is_empty());
            assert_eq!(engine.process_key_event(event), KeyAction::Bypass);
        }
    }

    #[test]
    fn test_force_separate_preedit_visibility_and_backspace() {
        let mut engine = PhoneticEngine::default();
        let event = KeyEvent {
            key: VirtualKey::Char('`'),
            ctrl: false,
            alt: false,
            shift: false,
            is_release: false,
        };
        assert_eq!(
            engine.process_key_event(event),
            KeyAction::UpdatePreedit {
                text: String::new(),
                cursor_pos: 0,
                visible: true,
            }
        );
        assert_eq!(engine.get_buffer(), "`");
        let backspace = KeyEvent {
            key: VirtualKey::Backspace,
            ..event
        };
        assert_eq!(
            engine.process_key_event(backspace),
            KeyAction::UpdatePreedit {
                text: String::new(),
                cursor_pos: 0,
                visible: false,
            }
        );
        assert!(engine.is_empty());
        assert_eq!(engine.process_key_event(backspace), KeyAction::Bypass);
    }

    #[test]
    fn test_ctrl_space_still_toggles_mode_and_clears_composition() {
        let mut engine = PhoneticEngine::default();
        engine.set_buffer("ami".into());
        let toggle = KeyEvent {
            key: VirtualKey::Space,
            ctrl: true,
            alt: false,
            shift: false,
            is_release: false,
        };
        for bangla_mode in [false, true] {
            assert_eq!(
                engine.process_key_event(toggle),
                KeyAction::ToggleMode { bangla_mode }
            );
            assert_eq!(engine.bangla_mode, bangla_mode);
            assert!(engine.is_empty());
            assert_eq!(
                engine.process_key_event(KeyEvent {
                    is_release: true,
                    ..toggle
                }),
                KeyAction::Bypass
            );
            assert_eq!(engine.bangla_mode, bangla_mode);
        }
    }

    #[test]
    fn test_vowels() {
        assert_eq!(translate("a"), "আ");
        assert_eq!(translate("i"), "ই");
        assert_eq!(translate("u"), "উ");
        assert_eq!(translate("e"), "এ");
        assert_eq!(translate("o"), "অ");
        assert_eq!(translate("O"), "ও");
        assert_eq!(translate("ee"), "ঈ");
        assert_eq!(translate("oo"), "উ");
        assert_eq!(translate("OI"), "ঐ");
        assert_eq!(translate("OU"), "ঔ");
        assert_eq!(translate("rri"), "ঋ");
    }

    #[test]
    fn test_dependent_vowels() {
        assert_eq!(translate("ka"), "কা");
        assert_eq!(translate("ki"), "কি");
        assert_eq!(translate("ku"), "কু");
        assert_eq!(translate("ke"), "কে");
        assert_eq!(translate("ko"), "ক");
        assert_eq!(translate("kO"), "কো");
        assert_eq!(translate("kee"), "কী");
        assert_eq!(translate("koo"), "কু");
        assert_eq!(translate("kOI"), "কৈ");
        assert_eq!(translate("kOU"), "কৌ");
        assert_eq!(translate("krri"), "কৃ");
    }

    #[test]
    fn test_consonants() {
        assert_eq!(translate("k"), "ক");
        assert_eq!(translate("kh"), "খ");
        assert_eq!(translate("g"), "গ");
        assert_eq!(translate("gh"), "ঘ");
        assert_eq!(translate("S"), "শ");
        assert_eq!(translate("Sh"), "ষ");
        assert_eq!(translate("sh"), "শ");
        assert_eq!(translate("Ng"), "ঙ");
        assert_eq!(translate("NG"), "ঞ");
        assert_eq!(translate("x"), "ক্স");
        assert_eq!(translate("kkh"), "ক্ষ");
    }

    #[test]
    fn test_conjuncts() {
        assert_eq!(translate("kt"), "ক্ত");
        assert_eq!(translate("sp"), "স্প");
        assert_eq!(translate("pl"), "প্ল");
    }

    #[test]
    fn test_khanda_ta_preserves_preceding_conjunct() {
        assert_eq!(translate("vorrt`sonapUrrNo"), "ভর্ৎসনাপূর্ণ");
        assert_eq!(translate("t`"), "ৎ");
        assert_eq!(translate("ut`sob"), "উৎসব");
        assert_eq!(translate("t`a"), "ৎআ");
        assert_eq!(translate("r`t`"), "রৎ");
        assert_eq!(translate("borrD"), "বর্ড");
    }

    #[test]
    fn test_manual_typing_khanda_ta_conjunct() {
        let mut engine = PhoneticEngine::default();
        for c in "vorrt`sonapUrrNo".chars() {
            let action = engine.process_key_event(KeyEvent {
                key: VirtualKey::Char(c),
                ctrl: false,
                alt: false,
                shift: c.is_ascii_uppercase(),
                is_release: false,
            });
            if c == '`' {
                assert_eq!(
                    action,
                    KeyAction::UpdatePreedit {
                        text: "ভর্ৎ".into(),
                        cursor_pos: 4,
                        visible: true,
                    }
                );
            }
        }
        assert_eq!(engine.translate(), "ভর্ৎসনাপূর্ণ");
        assert_eq!(
            engine.process_key_event(KeyEvent {
                key: VirtualKey::Space,
                ctrl: false,
                alt: false,
                shift: false,
                is_release: false,
            }),
            KeyAction::Commit {
                text: "ভর্ৎসনাপূর্ণ".into(),
                bypass_key: true,
            }
        );
        assert!(engine.is_empty());
    }

    #[test]
    fn test_force_separate() {
        assert_eq!(translate("k`kh"), "কখ");
        assert_eq!(translate("k`a"), "কআ");
    }

    #[test]
    fn test_y_ja_phala() {
        assert_eq!(translate("ky"), "ক্য"); // ja-phala
        assert_eq!(translate("ay"), "আয়"); // yya
        assert_eq!(translate("bybohar"), "ব্যবহার");
        assert_eq!(translate("byakti"), "ব্যাক্তি");
        assert_eq!(translate("kya"), "ক্যা");
    }

    #[test]
    fn test_z_contextual() {
        assert_eq!(translate("oZaDmin"), "অযাড্মিন");
        assert_eq!(translate("kZ"), "ক্য");
        assert_eq!(translate("kZa"), "ক্যা");
        assert_eq!(translate("oZa"), "অযা");
    }

    #[test]
    fn test_w_contextual_vowel_links() {
        // standalone w produces ও
        assert_eq!(translate("w"), "ও");

        // w followed by a vowel composes from the normal vowel rules, except wa
        assert_eq!(translate("wi"), "ওই");
        assert_eq!(translate("wI"), "ওঈ");
        assert_eq!(translate("wee"), "ওঈ");
        assert_eq!(translate("wu"), "ওউ");
        assert_eq!(translate("wU"), "ওঊ");
        assert_eq!(translate("wrri"), "ওঋ");
        assert_eq!(translate("we"), "ওএ");
        assert_eq!(translate("wo"), "ওঅ");
        assert_eq!(translate("wO"), "ওও");
        assert_eq!(translate("wOI"), "ওঐ");
        assert_eq!(translate("wOU"), "ওঔ");
        assert_eq!(translate("wa"), "ওয়া");

        // after a consonant, w produces ba-phala and allows a following kar
        assert_eq!(translate("kw"), "ক্ব");
        assert_eq!(translate("kwi"), "ক্বি");
        assert_eq!(translate("kwI"), "ক্বী");
        assert_eq!(translate("kwu"), "ক্বু");
        assert_eq!(translate("kwU"), "ক্বূ");
        assert_eq!(translate("kwrri"), "ক্বৃ");
        assert_eq!(translate("kwe"), "ক্বে");
        assert_eq!(translate("kwo"), "ক্ব");
        assert_eq!(translate("kwO"), "ক্বো");
        assert_eq!(translate("kwOI"), "ক্বৈ");
        assert_eq!(translate("kwOU"), "ক্বৌ");
        assert_eq!(translate("kwa"), "ক্বা");
        assert_eq!(translate("swadhIn"), "স্বাধীন");
        assert_eq!(translate("swosti"), "স্বস্তি");
        assert_eq!(translate("swopno"), "স্বপ্ন");
        assert_eq!(translate("udweg"), "উদ্বেগ");
    }

    #[test]
    fn test_hasant_double_comma() {
        assert_eq!(translate("k,,k"), "ক্ক");
        assert_eq!(translate("k,,"), "ক্");
    }

    #[test]
    fn test_colon_escape() {
        assert_eq!(translate(":`"), ":");
        assert_eq!(translate("k:`"), "ক:");
    }

    #[test]
    fn test_bisorgo_and_chandrabindu() {
        assert_eq!(translate("k:"), "কঃ");
        assert_eq!(translate("k^"), "কঁ");
    }

    #[test]
    fn test_bengali_numerals() {
        assert_eq!(translate("0123456789"), "০১২৩৪৫৬৭৮৯");
        assert_eq!(translate("ami123"), "আমি১২৩");
        assert_eq!(translate("2026."), "২০২৬।");
    }

    #[test]
    fn test_punctuation() {
        assert_eq!(translate("ami."), "আমি।");
        assert_eq!(translate("ami.."), "আমি.");
    }
}
