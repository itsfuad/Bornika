mod utils;

pub mod rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualKey {
    Char(char),
    Backspace,
    Escape,
    Tab,
    Space,
    Enter,
    Other,
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

#[derive(Debug, Clone, Default)]
pub struct PhoneticEngine {
    composition_buffer: String,
    pub bangla_mode: bool,
}

impl PhoneticEngine {
    pub fn new() -> Self {
        Self {
            composition_buffer: String::new(),
            bangla_mode: true,
        }
    }

    pub fn get_buffer(&self) -> &str {
        &self.composition_buffer
    }

    pub fn set_buffer(&mut self, val: String) {
        self.composition_buffer = val;
    }

    pub fn clear(&mut self) {
        self.composition_buffer.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.composition_buffer.is_empty()
    }

    pub fn translate(&self) -> String {
        translate(&self.composition_buffer)
    }

    pub fn push_char(&mut self, c: char) {
        self.composition_buffer.push(c);
    }

    pub fn pop_char(&mut self) -> bool {
        if !self.composition_buffer.is_empty() {
            self.composition_buffer.pop();
            true
        } else {
            false
        }
    }

    pub fn process_key_event(&mut self, event: KeyEvent) -> KeyAction {
        if event.alt
            || (event.ctrl && event.key != VirtualKey::Space)
            || (event.shift && matches!(event.key, VirtualKey::Escape | VirtualKey::Tab))
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

        let preedit = match event.key {
            VirtualKey::Backspace => {
                if !self.pop_char() {
                    return KeyAction::Bypass;
                }
                self.translate()
            }
            VirtualKey::Escape => {
                if self.is_empty() {
                    return KeyAction::Bypass;
                }
                self.clear();
                String::new()
            }
            VirtualKey::Char(c) if is_composition_char(c) => {
                self.push_char(c);
                self.translate()
            }
            _ => return KeyAction::Bypass,
        };
        let cursor_pos = preedit.chars().count() as u32;
        // A typed backtick can keep composition active without producing visible text.
        let visible = !preedit.is_empty() || matches!(event.key, VirtualKey::Char(_));
        KeyAction::UpdatePreedit {
            text: preedit,
            cursor_pos,
            visible,
        }
    }
}

/// Core translation algorithm that parses a Romanized input string into Bengali Unicode
pub fn translate(input: &str) -> String {
    let mut output = String::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // State trackers mapping directly to the Chain and VowelLink data models
    let mut can_take_dependent_vowel = false;
    let mut can_form_conjunct = false;

    while i < len {
        let mut matched = false;
        let remaining_slice = &chars[i..];

        for rule in rules::RULES {
            let rule_len = rule.roman.chars().count();
            if remaining_slice.len() >= rule_len
                && remaining_slice[..rule_len].iter().collect::<String>() == rule.roman
            {
                matched = true;
                i += rule_len;

                match &rule.token_type {
                    rules::TokenType::Vowel {
                        independent,
                        dependent,
                    } => {
                        if can_take_dependent_vowel {
                            output.push_str(dependent);
                        } else {
                            output.push_str(independent);
                        }
                        can_take_dependent_vowel = false;
                        can_form_conjunct = false;
                    }
                    rules::TokenType::Consonant(forms) => {
                        if can_form_conjunct {
                            output.push_str(forms.after_consonant);
                            can_form_conjunct = forms.chain_after_consonant == utils::Chain::Allows;
                            can_take_dependent_vowel =
                                forms.vowel_link_after_consonant == utils::VowelLink::Allows;
                        } else {
                            output.push_str(forms.after_vowel);
                            can_form_conjunct = forms.chain_after_vowel == utils::Chain::Allows;
                            can_take_dependent_vowel =
                                forms.vowel_link_after_vowel == utils::VowelLink::Allows;
                        }
                    }
                    rules::TokenType::Exact(val) => {
                        output.push_str(val);
                        // Reset state because we inserted a complete, pre-formatted chunk
                        can_take_dependent_vowel = false;
                        can_form_conjunct = false;
                    }
                    rules::TokenType::Sign(val) => {
                        output.push_str(val);
                        can_take_dependent_vowel = false;
                        can_form_conjunct = false;
                    }
                    rules::TokenType::ForceSeparate => {
                        can_take_dependent_vowel = false;
                        can_form_conjunct = false;
                    }
                    rules::TokenType::Punctuation(val) => {
                        output.push_str(val);
                        can_take_dependent_vowel = false;
                        can_form_conjunct = false;
                    }
                }
                break;
            }
        }

        if !matched {
            let next_char = chars[i];
            output.push(next_char);
            i += 1;
            can_take_dependent_vowel = false;
            can_form_conjunct = false;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let mut engine = PhoneticEngine::new();
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
            let mut engine = PhoneticEngine::new();
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
            for key in [VirtualKey::Escape, VirtualKey::Tab] {
                let mut engine = PhoneticEngine::new();
                engine.bangla_mode = bangla_mode;
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
        for key in [VirtualKey::Escape, VirtualKey::Tab] {
            for (ctrl, alt, shift) in [
                (true, false, false),
                (false, true, false),
                (false, false, true),
                (true, true, true),
            ] {
                let mut engine = PhoneticEngine::new();
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
        for key in [VirtualKey::Escape, VirtualKey::Tab] {
            for buffer in ["", "ami"] {
                let mut engine = PhoneticEngine::new();
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
            let mut engine = PhoneticEngine::new();
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
        let mut engine = PhoneticEngine::new();
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
        let mut engine = PhoneticEngine::new();
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
        let mut engine = PhoneticEngine::new();
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
