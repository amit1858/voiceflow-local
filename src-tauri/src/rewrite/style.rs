//! Central writing-style rules.
//!
//! Style is enforced in two places for defense in depth:
//! 1. Injected into the LLM **system prompt** so the model aims for it.
//! 2. Applied as a deterministic **post-filter** on every output (including the
//!    Raw transcript), so forbidden words never reach the clipboard even if the
//!    model ignores instructions.
//!
//! The current rules: simple, polished English; crisp and collaborative; and
//! the word "kindly" is never allowed (it is stripped/replaced).

/// Immutable set of writing-style rules shared across the app.
#[derive(Debug, Clone)]
pub struct StyleRules {
    /// Words that must never appear in output, each with a replacement.
    /// An empty replacement means "strip the word entirely".
    forbidden: Vec<(&'static str, &'static str)>,
}

impl Default for StyleRules {
    fn default() -> Self {
        StyleRules {
            // "kindly" is banned outright — replace with "please" where it reads
            // as a request, otherwise the post-filter cleanup drops stray spaces.
            forbidden: vec![("kindly", "please")],
        }
    }
}

impl StyleRules {
    /// The natural-language description injected into the LLM system prompt.
    pub fn system_prompt(&self) -> String {
        "You are a writing assistant that rewrites transcribed speech into clear, \
polished text. Follow these style rules exactly:\n\
- Use simple, polished English.\n\
- Be crisp and practical.\n\
- Sound warm but professional, and collaborative.\n\
- Never use the word \"kindly\".\n\
- Avoid overly formal or stiff phrasing.\n\
- Avoid an escalatory or confrontational tone unless the speaker explicitly asked for it.\n\
- The result should paste cleanly into Teams, Outlook, OneNote, a PRD, or leadership notes.\n\
Preserve the speaker's intent and facts; do not add information that was not said. \
Output only the rewritten text with no preamble, notes, or markdown code fences."
            .to_string()
    }

    /// Deterministic post-filter applied to all output. Removes/replaces
    /// forbidden words (case-insensitive, whole-word) and tidies whitespace.
    pub fn apply(&self, input: &str) -> String {
        let mut text = input.to_string();
        for (word, replacement) in &self.forbidden {
            text = replace_word_ci(&text, word, replacement);
        }
        tidy_whitespace(&text)
    }
}

/// Case-insensitive whole-word replacement without a regex dependency.
fn replace_word_ci(input: &str, word: &str, replacement: &str) -> String {
    let lower_input = input.to_lowercase();
    let lower_word = word.to_lowercase();
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < input.len() {
        if lower_input[i..].starts_with(&lower_word)
            && is_boundary(bytes, i.wrapping_sub(1), i > 0)
            && is_boundary(bytes, i + word.len(), i + word.len() < input.len())
        {
            // Preserve capitalization: if the matched word started uppercase and
            // the replacement is non-empty, capitalize the replacement.
            if replacement.is_empty() {
                // strip: skip the word
            } else if bytes[i].is_ascii_uppercase() {
                let mut chars = replacement.chars();
                if let Some(first) = chars.next() {
                    result.extend(first.to_uppercase());
                    result.push_str(chars.as_str());
                }
            } else {
                result.push_str(replacement);
            }
            i += word.len();
        } else {
            // Copy one UTF-8 char.
            let ch_len = utf8_char_len(bytes[i]);
            result.push_str(&input[i..i + ch_len]);
            i += ch_len;
        }
    }
    result
}

/// A boundary is where the neighboring byte is not an ASCII alphanumeric.
fn is_boundary(bytes: &[u8], idx: usize, exists: bool) -> bool {
    if !exists {
        return true;
    }
    match bytes.get(idx) {
        Some(b) => !b.is_ascii_alphanumeric(),
        None => true,
    }
}

fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    }
}

/// Collapse repeated spaces and fix spacing left behind by word removal.
fn tidy_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            out.push(ch);
        }
    }
    // Fix " ," / " ." / " !" / " ?" spacing introduced by stripping words.
    out.replace(" ,", ",")
        .replace(" .", ".")
        .replace(" !", "!")
        .replace(" ?", "?")
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_kindly_case_insensitively() {
        let rules = StyleRules::default();
        assert_eq!(rules.apply("Kindly review this"), "Please review this");
        assert_eq!(
            rules.apply("Please kindly send it"),
            "Please please send it"
        );
    }

    #[test]
    fn does_not_touch_substrings() {
        let rules = StyleRules::default();
        // "kindliness" contains "kindl" but is not the whole word "kindly".
        assert_eq!(rules.apply("kindliness matters"), "kindliness matters");
    }

    #[test]
    fn collapses_whitespace() {
        let rules = StyleRules::default();
        assert_eq!(rules.apply("hello    world  "), "hello world");
    }
}
