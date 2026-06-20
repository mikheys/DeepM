//! Lightweight OCR text normalization — no AI, no LLM, no second OCR pass.
//!
//! Script-specialised OCR models (e.g. a Cyrillic PP-OCR model) routinely
//! render the odd Latin letter inside an otherwise-Latin word as a visually
//! identical Cyrillic look-alike — and vice-versa. So `cyrillic` comes out as
//! `суrilliс` and `OCR` as `ОСR`. This pass repairs ONLY such mixed-alphabet
//! words by converting the look-alike letters to the script that the word's
//! *unambiguous* letters already prove it to be.
//!
//! Rules (deliberately conservative — never guess, never change meaning):
//! - Only touch words that actually mix Cyrillic and Latin letters.
//! - Decide the target script from the unambiguous letters (those without a
//!   look-alike twin). If both scripts have unambiguous letters, or neither
//!   does, the word is left untouched.
//! - Digits and punctuation are never altered.

/// Latin ⇄ Cyrillic look-alike pairs. Drives both the "is this letter
/// ambiguous?" test and the actual substitution.
const CONFUSABLES: &[(char, char)] = &[
    // lowercase
    ('a', 'а'), ('e', 'е'), ('o', 'о'), ('c', 'с'), ('p', 'р'),
    ('y', 'у'), ('x', 'х'), ('i', 'і'), ('j', 'ј'), ('s', 'ѕ'),
    // uppercase
    ('A', 'А'), ('B', 'В'), ('E', 'Е'), ('K', 'К'), ('M', 'М'),
    ('H', 'Н'), ('O', 'О'), ('P', 'Р'), ('C', 'С'), ('T', 'Т'),
    ('X', 'Х'), ('Y', 'У'), ('I', 'І'),
];

fn lat_to_cyr(c: char) -> Option<char> {
    CONFUSABLES.iter().find(|(l, _)| *l == c).map(|(_, r)| *r)
}
fn cyr_to_lat(c: char) -> Option<char> {
    CONFUSABLES.iter().find(|(_, r)| *r == c).map(|(l, _)| *l)
}

fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

/// Repair one alphabetic word; returns it unchanged unless it's a fixable
/// mixed-script word.
fn fix_word(word: &str) -> String {
    let mut has_cyr = false;
    let mut has_lat = false;
    let mut cyr_unambiguous = false;
    let mut lat_unambiguous = false;
    for c in word.chars() {
        if is_cyrillic(c) {
            has_cyr = true;
            if cyr_to_lat(c).is_none() {
                cyr_unambiguous = true;
            }
        } else if c.is_ascii_alphabetic() {
            has_lat = true;
            if lat_to_cyr(c).is_none() {
                lat_unambiguous = true;
            }
        }
    }

    // Only mixed-script words are candidates.
    if !(has_cyr && has_lat) {
        return word.to_string();
    }
    // The unambiguous letters must point to exactly one script, else don't guess.
    let target_latin = if lat_unambiguous && !cyr_unambiguous {
        true
    } else if cyr_unambiguous && !lat_unambiguous {
        false
    } else {
        return word.to_string();
    };

    word.chars()
        .map(|c| {
            if target_latin {
                if is_cyrillic(c) { cyr_to_lat(c).unwrap_or(c) } else { c }
            } else if c.is_ascii_alphabetic() {
                lat_to_cyr(c).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// Repair mixed Cyrillic/Latin look-alike words across the whole text.
pub fn normalize_ocr_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut word = String::new();
    for c in input.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            if !word.is_empty() {
                out.push_str(&fix_word(&word));
                word.clear();
            }
            out.push(c);
        }
    }
    if !word.is_empty() {
        out.push_str(&fix_word(&word));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_mixed_latin_words() {
        assert_eq!(normalize_ocr_text("суrilliс"), "cyrillic");
        assert_eq!(normalize_ocr_text("Lаtin"), "Latin");
        assert_eq!(normalize_ocr_text("ОСR"), "OCR");
        assert_eq!(normalize_ocr_text("RU/ЕN"), "RU/EN");
    }

    #[test]
    fn leaves_pure_words_untouched() {
        assert_eq!(normalize_ocr_text("кириллица"), "кириллица");
        assert_eq!(normalize_ocr_text("English"), "English");
        assert_eq!(normalize_ocr_text("привет world"), "привет world");
    }

    #[test]
    fn does_not_guess_when_ambiguous() {
        // Both scripts have unambiguous anchors (т is Cyrillic-only, b/l Latin-only)
        // → too risky to fix, leave as-is.
        assert_eq!(normalize_ocr_text("тоbilе"), "тоbilе");
    }
}
