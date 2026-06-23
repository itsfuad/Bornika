pub use crate::utils::{Chain, Rule, TokenType, VowelLink};

use crate::utils::{
    consonant, count_rules, expand_rules, punctuation, sign, sort_rules, trigger, vowel, RuleSpec,
};

// Keep this list easy to edit. RULES below expands aliases and sorts matches at compile time.
const RULE_SPECS: &[RuleSpec] = &[
    // --- Standard Consonants ---
    (trigger!("k", "K"), consonant!("ক")),
    (trigger!("kh", "kH", "Kh", "KH"), consonant!("খ")),
    (trigger!("g", "G"), consonant!("গ")),
    (trigger!("gh", "gH", "Gh", "GH"), consonant!("ঘ")),
    (trigger!("Ng"), consonant!("ঙ")),
    (trigger!("c", "C"), consonant!("চ")),
    (trigger!("ch", "cH", "Ch", "CH"), consonant!("ছ")),
    (trigger!("j", "J"), consonant!("জ")),
    (trigger!("jh", "jH", "Jh", "JH"), consonant!("ঝ")),
    (trigger!("NG"), consonant!("ঞ")),
    (trigger!("T"), consonant!("ট")),
    (trigger!("Th", "TH"), consonant!("ঠ")),
    (trigger!("D"), consonant!("ড")),
    (trigger!("Dh", "DH"), consonant!("ঢ")),
    (trigger!("N"), consonant!("ণ")),
    (trigger!("t"), consonant!("ত")),
    (trigger!("th", "tH"), consonant!("থ")),
    (trigger!("d"), consonant!("দ")),
    (trigger!("dh", "dH"), consonant!("ধ")),
    (trigger!("n"), consonant!("ন")),
    (trigger!("p", "P"), consonant!("প")),
    (trigger!("f", "F", "ph", "pH", "Ph", "PH"), consonant!("ফ")),
    (trigger!("b", "B"), consonant!("ব")),
    (trigger!("v", "V", "bh", "bH", "Bh", "BH"), consonant!("ভ")),
    (trigger!("m", "M"), consonant!("ম")),
    (trigger!("r", "rr"), consonant!("র")),
    (trigger!("l", "L"), consonant!("ল")),
    (trigger!("S"), consonant!("শ")),
    (trigger!("sh", "sH"), consonant!("শ")),
    (trigger!("Sh", "SH"), consonant!("ষ")),
    (trigger!("s"), consonant!("স")),
    (trigger!("h", "H"), consonant!("হ")),
    (trigger!("kkh"), consonant!("ক্ষ")),
    (trigger!("R"), consonant!("ড়")),
    (trigger!("Rh", "RH"), consonant!("ঢ়")),
    
    // Special rules
    (trigger!("x"), consonant!("ক্স")),
    
    // --- Contextual Special Consonants ---
    
    // z / Z: Natively 'য', Ja-phala '্য' if after consonant.
    (trigger!("z", "Z"), consonant!(
        after_vowel: "য" => Chain::Allows, Vowel::Allows, 
        after_consonant: "্য" => Chain::Breaks, Vowel::Allows
    )),

    // y / Y: Natively 'য়', Ja-phala '্য' if preceded by a consonant.
    (trigger!("y", "Y"), consonant!(
        after_vowel: "য়" => Chain::Allows, Vowel::Allows,
        after_consonant: "্য" => Chain::Breaks, Vowel::Allows
    )),

    // --- The "W + Vowel" Phonetic Blocks ---
    (trigger!("wa", "wA", "Wa", "WA"), consonant!(
        after_vowel: "ওয়া" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বা" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wi", "Wi"), consonant!(
        after_vowel: "ওয়ি" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বি" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wI", "WI", "wee", "Wee"), consonant!(
        after_vowel: "ওয়ী" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বী" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wu", "Wu", "woo", "Woo"), consonant!(
        after_vowel: "য়ু" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বু" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wU", "WU"), consonant!(
        after_vowel: "য়ূ" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বূ" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wrri", "Wrri"), consonant!(
        after_vowel: "ওয়ৃ" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বৃ" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("we", "We", "wE", "WE"), consonant!(
        after_vowel: "ওয়ে" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বে" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wo", "Wo", "wO", "WO"), consonant!(
        after_vowel: "ওয়ো" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বো" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wOI", "WOI"), consonant!(
        after_vowel: "ওয়ৈ" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বৈ" => Chain::Breaks, Vowel::Breaks
    )),
    (trigger!("wOU", "WOU"), consonant!(
        after_vowel: "ওয়ৌ" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্বৌ" => Chain::Breaks, Vowel::Breaks
    )),
    
    // w / W Fallback: If 'w' is typed ALONE.
    (trigger!("w", "W"), consonant!(
        after_vowel: "ও" => Chain::Breaks, Vowel::Breaks,
        after_consonant: "্ব" => Chain::Breaks, Vowel::Allows
    )),

    // Signs
    (trigger!("t`"), sign("ৎ")),
    (trigger!("ng", "nG"), sign("ং")),
    (trigger!(":"), sign("ঃ")),
    (trigger!("^"), sign("ঁ")),
    (trigger!(",,"), sign("্")),
    
    // Vowels
    (trigger!("o"), vowel("অ", "")),
    (trigger!("a", "A"), vowel("আ", "া")),
    (trigger!("i"), vowel("ই", "ি")),
    (trigger!("I", "ee"), vowel("ঈ", "ী")),
    (trigger!("u"), vowel("উ", "ু")),
    (trigger!("U"), vowel("ঊ", "ূ")),
    (trigger!("rri"), vowel("ঋ", "ৃ")),
    (trigger!("e", "E"), vowel("এ", "ে")),
    (trigger!("OI"), vowel("ঐ", "ৈ")),
    (trigger!("O"), vowel("ও", "ো")),
    (trigger!("OU"), vowel("ঔ", "ৌ")),
    (trigger!("oo"), vowel("উ", "ু")),
    
    // Punctuation
    (trigger!("."), punctuation("।")),
    (trigger!(".."), punctuation(".")),
    (trigger!(":`"), punctuation(":")),
    
    // Special Trigger
    (trigger!("`"), TokenType::ForceSeparate),
];

const RULE_COUNT: usize = count_rules(RULE_SPECS);
const SORTED_RULES: [Rule; RULE_COUNT] = sort_rules(expand_rules::<RULE_COUNT>(RULE_SPECS));

pub const RULES: &[Rule] = &SORTED_RULES;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_sorted_by_longest_roman_then_lexicographic() {
        for pair in RULES.windows(2) {
            let left = pair[0].roman;
            let right = pair[1].roman;

            assert!(
                left.len() > right.len() || (left.len() == right.len() && left < right),
                "{left:?} should sort before {right:?}"
            );
        }
    }

    #[test]
    fn romans_aliases_expand_to_equivalent_rules() {
        assert_eq!(token_type_for("b"), token_type_for("B"));
        assert_eq!(token_type_for("v"), token_type_for("V"));
        assert_eq!(token_type_for("y"), token_type_for("Y"));
        assert_eq!(token_type_for("w"), token_type_for("W"));
        assert_eq!(token_type_for("z"), token_type_for("Z"));
        assert_eq!(token_type_for("we"), token_type_for("WE"));
    }

    fn token_type_for(roman: &str) -> Option<TokenType> {
        RULES
            .iter()
            .find(|rule| rule.roman == roman)
            .map(|rule| rule.token_type)
    }
}