#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellToken {
    Word(String),
    Pipe,
    And,
    Or,
    Sequence,
    Redirect(bool),
    Input,
}

pub struct ShellSyntax;

impl ShellSyntax {
    pub fn tokenize(input: &str) -> Result<Vec<ShellToken>, String> {
        let mut chars = input.chars().peekable();
        let mut tokens = Vec::new();
        let mut word = String::new();
        let mut started = false;
        let mut quote = None;
        while let Some(c) = chars.next() {
            if c == '\\' && quote != Some('\'') {
                let escaped = chars.next().ok_or("unexpected end of input after escape")?;
                if quote == Some('"') && !matches!(escaped, '$' | '`' | '"' | '\\' | '\n') {
                    word.push('\\');
                }
                if escaped == '$' {
                    word.push('\u{e000}');
                } else if escaped != '\n' {
                    word.push(escaped);
                }
                started = true;
                continue;
            }
            if let Some(delimiter) = quote {
                if c == delimiter {
                    quote = None;
                } else {
                    // Protect single-quoted dollars from expansion.
                    if c == '$' && delimiter == '\'' {
                        word.push('\u{e000}');
                    } else {
                        word.push(c);
                    }
                }
                continue;
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
                started = true;
                continue;
            }
            if c.is_whitespace() || matches!(c, '|' | '&' | ';' | '>' | '<') {
                if started {
                    tokens.push(ShellToken::Word(std::mem::take(&mut word)));
                    started = false;
                }
                match c {
                    '|' => tokens.push(if chars.peek() == Some(&'|') {
                        chars.next();
                        ShellToken::Or
                    } else {
                        ShellToken::Pipe
                    }),
                    '&' => {
                        if chars.next() != Some('&') {
                            return Err("background jobs are not supported; use &&".into());
                        }
                        tokens.push(ShellToken::And);
                    }
                    ';' => tokens.push(ShellToken::Sequence),
                    '>' => {
                        let append = chars.peek() == Some(&'>');
                        if append {
                            chars.next();
                        }
                        tokens.push(ShellToken::Redirect(append));
                    }
                    '<' => tokens.push(ShellToken::Input),
                    _ => {}
                }
            } else if c == '#' && !started {
                break;
            } else {
                word.push(c);
                started = true;
            }
        }
        if quote.is_some() {
            return Err("unexpected EOF while looking for matching quote".into());
        }
        if started {
            tokens.push(ShellToken::Word(word));
        }
        Ok(tokens)
    }
}
