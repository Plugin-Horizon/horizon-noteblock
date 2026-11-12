//! The texts of `messages_<language>.yml`: `%placeholders%` filled, `&` colour codes turned into `§` codes
//! (Minecraft's section sign, which `Text::legacy` reads) and plain text for the console.

use std::collections::BTreeMap;

/// Placeholder values for a message.
pub type Vars = Vec<(&'static str, String)>;

/// The messages file as a map from key to text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Messages {
    map: BTreeMap<String, String>,
}

impl Messages {
    pub fn from_pairs(pairs: Vec<(String, String)>) -> Messages {
        Messages {
            map: pairs.into_iter().collect(),
        }
    }

    /// The text of a key, empty when the file has none.
    pub fn raw(&self, key: &str) -> String {
        self.map.get(key).cloned().unwrap_or_default()
    }

    fn fill(&self, key: &str, vars: &[(&str, String)]) -> String {
        let mut text = self.raw(key);
        for (name, value) in vars {
            text = text.replace(&format!("%{name}%"), value);
        }
        text
    }

    /// The message with its placeholders filled, colours as `§` codes.
    pub fn get(&self, key: &str, vars: &[(&str, String)]) -> String {
        legacy(&self.fill(key, vars))
    }

    /// The same behind the file's `prefix` (the replies of commands).
    pub fn prefixed(&self, key: &str, vars: &[(&str, String)]) -> String {
        legacy(&format!("{}{}", self.raw("prefix"), self.fill(key, vars)))
    }
}

/// `&` followed by a colour or format code becomes `§` followed by the code (lower case); other `&` stay.
pub fn legacy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if let Some(&code) = chars.peek() {
                if is_code(code) {
                    out.push('§');
                    out.push(code.to_ascii_lowercase());
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// The text without its colour codes (for the console).
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if let Some(&code) = chars.peek() {
                if is_code(code) {
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn is_code(c: char) -> bool {
    c.is_ascii_hexdigit() || "klmnor".contains(c.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Messages {
        Messages::from_pairs(vec![
            ("prefix".into(), "&9[NoteBlockMusicPlayer] ".into()),
            (
                "actionbar".into(),
                "You are now listening %song_title%".into(),
            ),
            (
                "command_state_enable".into(),
                "&aState of %radio_type% radio '%radio%' was set to enable.".into(),
            ),
        ])
    }

    #[test]
    fn placeholders_are_filled_and_colours_converted() {
        let m = sample();
        assert_eq!(
            m.get(
                "command_state_enable",
                &[
                    ("radio_type", "world".into()),
                    ("radio", "fixture_world".into())
                ]
            ),
            "§aState of world radio 'fixture_world' was set to enable."
        );
        assert_eq!(
            m.get("actionbar", &[("song_title", "Jingle Bells".into())]),
            "You are now listening Jingle Bells"
        );
    }

    #[test]
    fn prefixed_puts_the_file_prefix_in_front() {
        let m = sample();
        assert_eq!(
            m.prefixed(
                "command_state_enable",
                &[("radio_type", "server".into()), ("radio", "x".into())]
            ),
            "§9[NoteBlockMusicPlayer] §aState of server radio 'x' was set to enable."
        );
    }

    #[test]
    fn missing_key_is_empty_and_unknown_codes_stay() {
        let m = sample();
        assert_eq!(m.get("nope", &[]), "");
        assert_eq!(legacy("Tom & Jerry &q"), "Tom & Jerry &q");
        assert_eq!(legacy("&lbold &Ffine"), "§lbold §ffine");
    }

    #[test]
    fn plain_strips_codes_for_the_console() {
        assert_eq!(plain("&9[NBMP] &aok&r"), "[NBMP] ok");
        assert_eq!(plain("100% &&"), "100% &&");
    }
}
