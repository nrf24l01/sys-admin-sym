/// Tolerant lexer for the command at the cursor. It never executes input and
/// accepts unfinished quotes/escapes that are common while editing a line.
pub(super) struct CompletionInput {
    pub start: usize,
    pub words: Vec<String>,
    pub partial: String,
    pub redirect: bool,
}

impl CompletionInput {
    pub fn parse(input: &str) -> Self {
        let mut result = Self {
            start: 0,
            words: Vec::new(),
            partial: String::new(),
            redirect: false,
        };
        let mut quote = None;
        let mut escaped = false;
        let mut started = false;
        for (index, c) in input.char_indices() {
            if escaped {
                result.partial.push(c);
                escaped = false;
                continue;
            }
            if c == '\\' && quote != Some('\'') {
                if !started {
                    result.start = index;
                    started = true;
                }
                escaped = true;
                continue;
            }
            if let Some(delimiter) = quote {
                if c == delimiter {
                    quote = None;
                } else {
                    result.partial.push(c);
                }
                continue;
            }
            if c == '\'' || c == '"' {
                if !started {
                    result.start = index;
                    started = true;
                }
                quote = Some(c);
                continue;
            }
            if c.is_whitespace() || matches!(c, '|' | '&' | ';' | '>' | '<') {
                if started {
                    if result.redirect {
                        result.partial.clear();
                        result.redirect = false;
                    } else {
                        result.words.push(std::mem::take(&mut result.partial));
                    }
                    started = false;
                }
                match c {
                    '|' | '&' | ';' => {
                        result.words.clear();
                        result.redirect = false;
                    }
                    '>' | '<' => result.redirect = true,
                    _ => {}
                }
                result.start = index + c.len_utf8();
            } else {
                if !started {
                    result.start = index;
                    started = true;
                }
                result.partial.push(c);
            }
        }
        result
    }
}
