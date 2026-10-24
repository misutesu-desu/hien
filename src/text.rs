use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MatchOptions {
    pub match_case: bool,
    pub whole_word: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Side {
    Before,
    After,
}

// Turkish I/ı and İ/i do not pair up under standard lowercasing. Treating all
// four as one letter in search surprises less than missing a match.
fn fold(c: char) -> char {
    match c {
        'I' | 'İ' | 'ı' => 'i',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

fn same(a: char, b: char, o: MatchOptions) -> bool {
    if o.match_case {
        a == b
    } else {
        fold(a) == fold(b)
    }
}

// The apostrophe is not part of a word, so "Ali" is found inside "Ali'nin".
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn find(text: &str, needle: &str, o: MatchOptions) -> Vec<Range<usize>> {
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = needle.len();
    let mut found = Vec::new();
    let mut i = 0;
    while i + n <= chars.len() {
        let matches = (0..n).all(|j| same(chars[i + j].1, needle[j], o));
        let on_boundary = !o.whole_word
            || ((i == 0 || !is_word_char(chars[i - 1].1))
                && chars.get(i + n).is_none_or(|&(_, c)| !is_word_char(c)));
        if matches && on_boundary {
            let start = chars[i].0;
            let end = chars.get(i + n).map_or(text.len(), |&(b, _)| b);
            found.push(start..end);
            i += n;
        } else {
            i += 1;
        }
    }
    found
}

pub fn replace(text: &str, matches: &[Range<usize>], with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in matches {
        out.push_str(&text[last..m.start]);
        out.push_str(with);
        last = m.end;
    }
    out.push_str(&text[last..]);
    out
}

fn ends_with(text: &str, suffix: &str, o: MatchOptions) -> bool {
    let mut t = text.chars().rev();
    suffix
        .chars()
        .rev()
        .all(|s| t.next().is_some_and(|c| same(c, s, o)))
}

fn starts_with(text: &str, prefix: &str, o: MatchOptions) -> bool {
    let mut t = text.chars();
    prefix
        .chars()
        .all(|p| t.next().is_some_and(|c| same(c, p, o)))
}

/// Inserts `word` before or after each match. A match that already has the word
/// there is skipped, so running the same insert twice leaves the text intact.
/// Returns the new text and how many places changed.
pub fn insert(
    text: &str,
    matches: &[Range<usize>],
    word: &str,
    side: Side,
    space: bool,
    o: MatchOptions,
) -> (String, usize) {
    let gap = if space { " " } else { "" };
    let piece = match side {
        Side::Before => format!("{word}{gap}"),
        Side::After => format!("{gap}{word}"),
    };
    let mut out = String::with_capacity(text.len() + matches.len() * piece.len());
    let mut last = 0;
    let mut count = 0;
    for m in matches {
        match side {
            Side::Before => {
                out.push_str(&text[last..m.start]);
                if !ends_with(&text[..m.start], &piece, o) {
                    out.push_str(&piece);
                    count += 1;
                }
                out.push_str(&text[m.clone()]);
            }
            Side::After => {
                out.push_str(&text[last..m.end]);
                if !starts_with(&text[m.end..], &piece, o) {
                    out.push_str(&piece);
                    count += 1;
                }
            }
        }
        last = m.end;
    }
    out.push_str(&text[last..]);
    (out, count)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub words: usize,
    pub chars: usize,
    pub lines: usize,
}

pub fn count(text: &str) -> Counts {
    Counts {
        words: text.split_whitespace().count(),
        chars: text.chars().count(),
        lines: text.lines().count().max(1),
    }
}

/// Line and column for a char index, both starting at 1.
pub fn line_col(text: &str, char_index: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for c in text.chars().take(char_index) {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn char_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: MatchOptions = MatchOptions {
        match_case: false,
        whole_word: false,
    };
    const WORD: MatchOptions = MatchOptions {
        match_case: false,
        whole_word: true,
    };

    fn slices<'a>(text: &'a str, m: &[Range<usize>]) -> Vec<&'a str> {
        m.iter().map(|r| &text[r.clone()]).collect()
    }

    #[test]
    fn turkish_case_folds_together() {
        let t = "İstanbul ve istanbul, ISPARTA ıspanak";
        assert_eq!(
            slices(t, &find(t, "istanbul", PLAIN)),
            ["İstanbul", "istanbul"]
        );
        assert_eq!(find(t, "ıs", PLAIN).len(), 4);
    }

    #[test]
    fn match_case() {
        let o = MatchOptions {
            match_case: true,
            whole_word: false,
        };
        assert_eq!(find("Kedi kedi KEDİ", "kedi", o), [5..9]);
    }

    #[test]
    fn whole_word_stops_at_apostrophe() {
        let t = "Ali'nin Alişan ali";
        assert_eq!(slices(t, &find(t, "ali", WORD)), ["Ali", "ali"]);
    }

    #[test]
    fn no_overlapping_matches() {
        assert_eq!(find("aaaa", "aa", PLAIN), [0..2, 2..4]);
    }

    #[test]
    fn empty_needle_finds_nothing() {
        assert!(find("text", "", PLAIN).is_empty());
    }

    #[test]
    fn replace_all() {
        let t = "tea and tea";
        assert_eq!(
            replace(t, &find(t, "tea", PLAIN), "coffee"),
            "coffee and coffee"
        );
    }

    #[test]
    fn insert_before() {
        let t = "the cat slept, the cat woke";
        let (out, n) = insert(t, &find(t, "cat", WORD), "tabby", Side::Before, true, WORD);
        assert_eq!(out, "the tabby cat slept, the tabby cat woke");
        assert_eq!(n, 2);
    }

    #[test]
    fn insert_skips_existing() {
        let t = "tabby cat and cat";
        let (out, n) = insert(t, &find(t, "cat", WORD), "Tabby", Side::Before, true, WORD);
        assert_eq!(out, "tabby cat and Tabby cat");
        assert_eq!(n, 1);
    }

    #[test]
    fn insert_after_without_space() {
        let t = "note1 note2";
        let (out, _) = insert(t, &find(t, "note", PLAIN), "#", Side::After, false, PLAIN);
        assert_eq!(out, "note#1 note#2");
    }

    #[test]
    fn counts_and_position() {
        let t = "bir iki\nüç";
        assert_eq!(
            count(t),
            Counts {
                words: 3,
                chars: 10,
                lines: 2
            }
        );
        assert_eq!(line_col(t, 9), (2, 2));
        assert_eq!(char_index(t, t.len()), 10);
    }
}
