// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 10:31:30.704087
//+ Revised: 	2026/11/13 07:53:54.320100
// -----------------------------------------------------------------------------

//! Syntax highlighting with the Sublime/TextMate grammars VS Code-style highlighting is built on.
//!
//! Grammar scopes are mapped to a small set of token kinds that every theme colours, much as VS
//! Code themes colour scopes. Highlighting is incremental: each line keeps the parser state it
//! ends in, so an edit re-highlights from the changed line, and only down to the last line shown.

use std::{ops::Range, path::Path, sync::OnceLock};
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

/// What a span of code is, for colouring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Comment,
    String,
    Keyword,
    Number,
    Constant,
    Function,
    Type,
    Variable,
    Operator,
    Tag,
    Attribute,
    Heading,
    Link,
}

/// Lines longer than this are left plain; grammars get very slow on minified files.
const MAX_LINE: usize = 20_000;

/// Scope prefixes in priority order; the innermost scope that matches any rule decides.
const RULES: [(&str, Token); 32] = [
    ("comment", Token::Comment),
    ("punctuation.definition.comment", Token::Comment),
    ("string", Token::String),
    ("constant.character.escape", Token::Operator),
    ("constant.numeric", Token::Number),
    ("constant.language", Token::Constant),
    ("constant", Token::Constant),
    ("entity.name.function", Token::Function),
    ("support.function", Token::Function),
    ("variable.function", Token::Function),
    ("meta.function-call.identifier", Token::Function),
    ("entity.name.type", Token::Type),
    ("entity.name.class", Token::Type),
    ("entity.name.struct", Token::Type),
    ("entity.name.enum", Token::Type),
    ("entity.name.trait", Token::Type),
    ("entity.name.impl", Token::Type),
    ("support.type", Token::Type),
    ("support.class", Token::Type),
    ("storage.type", Token::Keyword),
    ("storage.modifier", Token::Keyword),
    ("keyword.operator", Token::Operator),
    ("keyword", Token::Keyword),
    ("storage", Token::Keyword),
    ("variable.language", Token::Keyword),
    ("variable.parameter", Token::Variable),
    ("variable", Token::Variable),
    ("entity.name.tag", Token::Tag),
    ("entity.other.attribute-name", Token::Attribute),
    ("markup.heading", Token::Heading),
    ("markup.underline.link", Token::Link),
    ("entity.name", Token::Type),
];

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(|| {
        let set = SyntaxSet::load_defaults_newlines();
        tracing::debug!(target: "orin::syntax", count = set.syntaxes().len(), "grammars loaded");
        set
    })
}

fn rules() -> &'static [(Scope, Token)] {
    static RULE_SCOPES: OnceLock<Vec<(Scope, Token)>> = OnceLock::new();
    RULE_SCOPES.get_or_init(|| {
        RULES
            .iter()
            .filter_map(|(prefix, token)| match Scope::new(prefix) {
                Ok(scope) => Some((scope, *token)),
                Err(error) => {
                    tracing::error!(target: "orin::syntax", %error, prefix, "highlight rule is not a scope");
                    None
                }
            })
            .collect()
    })
}

fn token_for(stack: &ScopeStack) -> Option<Token> {
    stack.as_slice().iter().rev().find_map(|scope| {
        rules()
            .iter()
            .find(|(prefix, _)| prefix.is_prefix_of(*scope))
            .map(|(_, token)| *token)
    })
}

/// One highlighted line: token spans in byte offsets, plus the parser state at its end.
struct Line {
    text: String,
    end: (ParseState, ScopeStack),
    tokens: Vec<(Range<usize>, Token)>,
}

pub struct Highlighter {
    syntax: &'static SyntaxReference,
    lines: Vec<Line>,
    failed: bool,
}

impl Highlighter {
    /// A highlighter for the language of `path`, if a grammar knows its extension or name.
    pub fn for_path(path: &Path) -> Option<Self> {
        let set = syntaxes();
        let by_extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(|extension| set.find_syntax_by_extension(extension));
        let by_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| set.find_syntax_by_extension(name));
        by_extension.or(by_name).and_then(Self::with)
    }

    /// A highlighter for a fenced code block's language word, such as `rust` or `py`.
    pub fn for_language(language: &str) -> Option<Self> {
        let set = syntaxes();
        let word = language.split_whitespace().next()?;
        set.find_syntax_by_token(word).and_then(Self::with)
    }

    fn with(syntax: &'static SyntaxReference) -> Option<Self> {
        // Plain text has no grammar worth running.
        (syntax.name != "Plain Text").then_some(Self {
            syntax,
            lines: Vec::new(),
            failed: false,
        })
    }

    pub fn language(&self) -> &str {
        &self.syntax.name
    }

    /// Token spans for each of `lines[..=last]` (fewer if there are fewer lines). Lines
    /// unchanged since the previous call, and everything before them, are reused.
    pub fn highlight(&mut self, lines: &[&str], last: usize) -> Vec<&[(Range<usize>, Token)]> {
        let valid = self
            .lines
            .iter()
            .zip(lines)
            .take_while(|(cached, line)| cached.text == **line)
            .count();
        self.lines.truncate(valid);
        let wanted = last.saturating_add(1).min(lines.len());
        for line in lines.iter().take(wanted).skip(self.lines.len()) {
            let start = self.lines.last().map_or_else(
                || (ParseState::new(self.syntax), ScopeStack::new()),
                |previous| previous.end.clone(),
            );
            let parsed = self.parse(line, start);
            self.lines.push(parsed);
        }
        self.lines
            .iter()
            .take(wanted)
            .map(|line| line.tokens.as_slice())
            .collect()
    }

    fn parse(&mut self, text: &str, (mut state, mut stack): (ParseState, ScopeStack)) -> Line {
        let mut tokens: Vec<(Range<usize>, Token)> = Vec::new();
        // A skipped line hands the state before it to the next line, as if it were empty: a
        // comment or string it opens or closes is missed. Highlighting is a display aid, and
        // parsing a minified line can take seconds, so that error is accepted.
        if text.len() > MAX_LINE || self.failed {
            return Line {
                text: text.to_owned(),
                end: (state, stack),
                tokens,
            };
        }
        let with_newline = format!("{text}\n");
        let operations = match state.parse_line(&with_newline, syntaxes()) {
            Ok(operations) => operations,
            Err(error) => {
                tracing::warn!(target: "orin::syntax", %error, language = %self.syntax.name, "grammar failed; the rest is shown plain");
                self.failed = true;
                return Line {
                    text: text.to_owned(),
                    end: (state, stack),
                    tokens,
                };
            }
        };
        let mut push = |range: Range<usize>, token: Option<Token>| {
            let range = range.start.min(text.len())..range.end.min(text.len());
            let Some(token) = token.filter(|_| !range.is_empty()) else {
                return;
            };
            match tokens.last_mut() {
                Some((last, kind)) if *kind == token && last.end == range.start => {
                    last.end = range.end
                }
                _ => tokens.push((range, token)),
            }
        };
        let mut position = 0;
        for (offset, operation) in operations {
            push(position..offset, token_for(&stack));
            position = offset;
            if let Err(error) = stack.apply(&operation) {
                tracing::warn!(target: "orin::syntax", %error, "scope change failed; the rest is shown plain");
                self.failed = true;
                break;
            }
        }
        push(position..text.len(), token_for(&stack));
        Line {
            text: text.to_owned(),
            end: (state, stack),
            tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_of(highlighter: &mut Highlighter, source: &str) -> Vec<Vec<(String, Token)>> {
        let lines: Vec<&str> = source.split('\n').collect();
        highlighter
            .highlight(&lines, usize::MAX)
            .iter()
            .zip(&lines)
            .map(|(tokens, line)| {
                tokens
                    .iter()
                    .map(|(range, token)| (line[range.clone()].to_owned(), *token))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn unit_rust_keywords_strings_comments_and_functions_are_told_apart() {
        let mut rust = Highlighter::for_path(Path::new("src/main.rs")).unwrap();
        assert_eq!(rust.language(), "Rust");
        let lines = tokens_of(
            &mut rust,
            "// note\nfn main() {\n    println!(\"hi\", 42);\n}",
        );
        assert!(lines[0].contains(&("// note".into(), Token::Comment)));
        assert!(lines[1].contains(&("fn".into(), Token::Keyword)));
        assert!(lines[1].contains(&("main".into(), Token::Function)));
        assert!(
            lines[2]
                .iter()
                .any(|(text, token)| text.contains("hi") && *token == Token::String)
        );
        assert!(lines[2].contains(&("42".into(), Token::Number)));
    }

    #[test]
    fn unit_languages_come_from_extensions_names_and_fence_words() {
        for (path, language) in [
            ("a.py", "Python"),
            ("b.js", "JavaScript"),
            ("c.json", "JSON"),
            ("Makefile", "Makefile"),
        ] {
            assert_eq!(
                Highlighter::for_path(Path::new(path)).unwrap().language(),
                language,
                "{path}"
            );
        }
        assert!(Highlighter::for_path(Path::new("notes.txt")).is_none());
        assert!(Highlighter::for_path(Path::new("unknown.zzz")).is_none());
        assert_eq!(
            Highlighter::for_language("rust ignore").unwrap().language(),
            "Rust"
        );
        assert_eq!(
            Highlighter::for_language("py").unwrap().language(),
            "Python"
        );
        assert!(Highlighter::for_language("").is_none());
    }

    #[test]
    fn unit_incremental_highlighting_matches_a_fresh_pass_after_edits() {
        let before = "/* open\nstill comment */\nlet x = \"s\";\nfn f() {}";
        let after = "let y = 1;\nstill comment */\nlet x = \"s\";\nfn f() {}";
        let mut incremental = Highlighter::for_path(Path::new("x.rs")).unwrap();
        tokens_of(&mut incremental, before);
        let edited = tokens_of(&mut incremental, after);
        let fresh = tokens_of(
            &mut Highlighter::for_path(Path::new("x.rs")).unwrap(),
            after,
        );
        assert_eq!(edited, fresh);
        // The block comment no longer opens, so the second line is no longer a comment.
        let original = tokens_of(
            &mut Highlighter::for_path(Path::new("x.rs")).unwrap(),
            before,
        );
        assert!(
            original[1]
                .iter()
                .any(|(_, token)| *token == Token::Comment)
        );
        assert!(!edited[1].iter().any(|(_, token)| *token == Token::Comment));
    }

    #[test]
    fn unit_only_lines_up_to_the_last_shown_are_highlighted() {
        let mut rust = Highlighter::for_path(Path::new("x.rs")).unwrap();
        let lines = ["fn a() {}", "fn b() {}", "fn c() {}"];
        assert_eq!(rust.highlight(&lines, 0).len(), 1);
        assert_eq!(rust.lines.len(), 1);
        assert_eq!(rust.highlight(&lines, 9).len(), 3);
    }

    #[test]
    fn unit_very_long_lines_stay_plain() {
        let mut rust = Highlighter::for_path(Path::new("x.rs")).unwrap();
        let long = "let x = 1; ".repeat(MAX_LINE / 8);
        assert!(rust.highlight(&[long.as_str()], 0)[0].is_empty());
    }
}
