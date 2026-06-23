#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Allows,
    Breaks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VowelLink {
    Allows,
    Breaks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsonantForms {
    pub after_vowel: &'static str,
    pub after_consonant: &'static str,
    pub chain_after_vowel: Chain,
    pub chain_after_consonant: Chain,
    pub vowel_link_after_vowel: VowelLink,
    pub vowel_link_after_consonant: VowelLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Vowel {
        independent: &'static str,
        dependent: &'static str,
    },
    Consonant(ConsonantForms),
    Sign(&'static str),
    ForceSeparate,
    Punctuation(&'static str),
    Exact(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub roman: &'static str,
    pub token_type: TokenType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuleTrigger {
    One(&'static str),
    Many(&'static [&'static str]),
}

pub(crate) type RuleSpec = (RuleTrigger, TokenType);

macro_rules! trigger {
    ($first:expr, $($rest:expr),+ $(,)?) => {
        $crate::utils::RuleTrigger::Many(&[$first, $($rest),+])
    };
    ($roman:expr $(,)?) => {
        $crate::utils::RuleTrigger::One($roman)
    };
}

pub(crate) use trigger;

macro_rules! consonant {
    // 1. Standard consonant (auto-generates hasant form, allows chaining)
    ($char:expr) => {
        $crate::utils::TokenType::Consonant($crate::utils::ConsonantForms {
            after_vowel: $char,
            after_consonant: concat!("্", $char),
            chain_after_vowel: $crate::utils::Chain::Allows,
            chain_after_consonant: $crate::utils::Chain::Allows,
            vowel_link_after_vowel: $crate::utils::VowelLink::Allows,
            vowel_link_after_consonant: $crate::utils::VowelLink::Allows,
        })
    };

    // 2. Self-documenting custom rule for special contextual keys
    (
        after_vowel: $indep:expr => Chain::$chain_indep:ident, Vowel::$vowel_indep:ident,
        after_consonant: $conj:expr => Chain::$chain_conj:ident, Vowel::$vowel_conj:ident
    ) => {
        $crate::utils::TokenType::Consonant($crate::utils::ConsonantForms {
            after_vowel: $indep,
            after_consonant: $conj,
            chain_after_vowel: $crate::utils::Chain::$chain_indep,
            chain_after_consonant: $crate::utils::Chain::$chain_conj,
            vowel_link_after_vowel: $crate::utils::VowelLink::$vowel_indep,
            vowel_link_after_consonant: $crate::utils::VowelLink::$vowel_conj,
        })
    };
}

pub(crate) use consonant;

pub(crate) const fn vowel(independent: &'static str, dependent: &'static str) -> TokenType {
    TokenType::Vowel {
        independent,
        dependent,
    }
}

pub(crate) const fn sign(value: &'static str) -> TokenType {
    TokenType::Sign(value)
}

pub(crate) const fn punctuation(value: &'static str) -> TokenType {
    TokenType::Punctuation(value)
}

const EMPTY_RULE: Rule = Rule {
    roman: "",
    token_type: TokenType::ForceSeparate,
};

pub(crate) const fn count_rules(specs: &[RuleSpec]) -> usize {
    let mut count = 0;
    let mut i = 0;

    while i < specs.len() {
        count += match specs[i].0 {
            RuleTrigger::One(_) => 1,
            RuleTrigger::Many(romans) => {
                if romans.is_empty() {
                    panic!("romans rule must not be empty");
                }
                romans.len()
            }
        };
        i += 1;
    }

    count
}

pub(crate) const fn expand_rules<const N: usize>(specs: &[RuleSpec]) -> [Rule; N] {
    let mut rules = [EMPTY_RULE; N];
    let mut spec_index = 0;
    let mut rule_index = 0;

    while spec_index < specs.len() {
        match specs[spec_index].0 {
            RuleTrigger::One(roman) => {
                rules[rule_index] = expand_rule(roman, specs[spec_index].1);
                rule_index += 1;
            }
            RuleTrigger::Many(romans) => {
                let mut roman_index = 0;
                while roman_index < romans.len() {
                    rules[rule_index] = expand_rule(romans[roman_index], specs[spec_index].1);
                    rule_index += 1;
                    roman_index += 1;
                }
            }
        }
        spec_index += 1;
    }

    if rule_index != N {
        panic!("expanded rule count mismatch");
    }

    rules
}

const fn expand_rule(roman: &'static str, token_type: TokenType) -> Rule {
    if roman.is_empty() {
        panic!("roman rule must not be empty");
    }

    if !is_ascii(roman) {
        panic!("roman rule must be ASCII");
    }

    Rule { roman, token_type }
}

pub(crate) const fn sort_rules<const N: usize>(mut rules: [Rule; N]) -> [Rule; N] {
    let mut i = 1;

    while i < N {
        let rule = rules[i];
        let mut j = i;

        while j > 0 && rule_precedes(rule, rules[j - 1]) {
            rules[j] = rules[j - 1];
            j -= 1;
        }

        rules[j] = rule;
        i += 1;
    }

    assert_unique_romans(&rules);
    rules
}

const fn assert_unique_romans(rules: &[Rule]) {
    let mut i = 1;

    while i < rules.len() {
        if str_eq(rules[i - 1].roman, rules[i].roman) {
            panic!("duplicate roman rule");
        }
        i += 1;
    }
}

const fn rule_precedes(left: Rule, right: Rule) -> bool {
    if left.roman.len() == right.roman.len() {
        str_cmp(left.roman, right.roman) < 0
    } else {
        left.roman.len() > right.roman.len()
    }
}

const fn str_eq(left: &'static str, right: &'static str) -> bool {
    str_cmp(left, right) == 0
}

const fn str_cmp(left: &'static str, right: &'static str) -> i8 {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut i = 0;

    while i < left.len() && i < right.len() {
        if left[i] < right[i] {
            return -1;
        }

        if left[i] > right[i] {
            return 1;
        }

        i += 1;
    }

    if left.len() < right.len() {
        -1
    } else if left.len() > right.len() {
        1
    } else {
        0
    }
}

const fn is_ascii(value: &'static str) -> bool {
    let bytes = value.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] > 0x7f {
            return false;
        }
        i += 1;
    }

    true
}
