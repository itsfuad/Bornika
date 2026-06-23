mod utils;

pub mod rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualKey {
    Char(char),
    Backspace,
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
    Commit { text: String, bypass_key: bool },
    UpdatePreedit { text: String, cursor_pos: u32, visible: bool },
    ToggleMode { bangla_mode: bool },
}

fn is_composition_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || c == '`'
        || c == '.'
        || c == '^'
        || c == ':'
        || c == ','
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
        let bangla_mode = self.bangla_mode;

        if event.alt || (event.ctrl && event.key != VirtualKey::Space) {
            return KeyAction::Bypass;
        }

        if event.is_release {
            if bangla_mode && !self.is_empty() {
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
            return KeyAction::ToggleMode { bangla_mode: self.bangla_mode };
        }

        if !bangla_mode {
            return KeyAction::Bypass;
        }

        match event.key {
            VirtualKey::Backspace => {
                if !self.is_empty() {
                    self.pop_char();
                    let preedit = self.translate();
                    let cursor_pos = preedit.chars().count() as u32;
                    let visible = !preedit.is_empty();
                    KeyAction::UpdatePreedit {
                        text: preedit,
                        cursor_pos,
                        visible,
                    }
                } else {
                    KeyAction::Bypass
                }
            }
            VirtualKey::Space | VirtualKey::Enter => {
                if !self.is_empty() {
                    let committed = self.translate();
                    self.clear();
                    KeyAction::Commit {
                        text: committed,
                        bypass_key: true,
                    }
                } else {
                    KeyAction::Bypass
                }
            }
            VirtualKey::Char(c) => {
                if is_composition_char(c) {
                    self.push_char(c);
                    let preedit = self.translate();
                    let cursor_pos = preedit.chars().count() as u32;
                    KeyAction::UpdatePreedit {
                        text: preedit,
                        cursor_pos,
                        visible: true,
                    }
                } else if is_commit_punctuation(c) {
                    if !self.is_empty() {
                        let committed = self.translate();
                        self.clear();
                        KeyAction::Commit {
                            text: committed,
                            bypass_key: true,
                        }
                    } else {
                        KeyAction::Bypass
                    }
                } else {
                    KeyAction::Bypass
                }
            }
            VirtualKey::Other => KeyAction::Bypass,
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
            if remaining_slice.len() >= rule_len && 
               remaining_slice[..rule_len].iter().collect::<String>() == rule.roman {
                
                matched = true;
                i += rule_len;
                
                match &rule.token_type {
                    rules::TokenType::Vowel { independent, dependent } => {
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
                            can_take_dependent_vowel = forms.vowel_link_after_consonant == utils::VowelLink::Allows;
                        } else {
                            output.push_str(forms.after_vowel);
                            can_form_conjunct = forms.chain_after_vowel == utils::Chain::Allows;
                            can_take_dependent_vowel = forms.vowel_link_after_vowel == utils::VowelLink::Allows;
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
    fn test_exact_dictionary_words() {
        assert_eq!(translate("wifi"), "ওয়াইফাই");
        assert_eq!(translate("freewifi"), "ফ্রীওয়াইফাই");
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
        // w followed by vowel breaks link -> ও + independent vowel
        assert_eq!(translate("wi"), "ওই"); 
        assert_eq!(translate("wa"), "ওআ"); 
        
        // consonant + w produces ba-phala 
        assert_eq!(translate("kw"), "ক্ব");
        // consonant + w + vowel allows vowel link -> ba-phala + kar
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
    fn test_punctuation() {
        assert_eq!(translate("ami."), "আমি।");
        assert_eq!(translate("ami.."), "আমি.");
    }
}