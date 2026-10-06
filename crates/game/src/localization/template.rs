//! Explicit numbered template arguments; no matching against rendered English.
use std::collections::BTreeSet;

pub(super) fn fields(template: &str) -> Result<BTreeSet<usize>, String> {
    let mut fields = BTreeSet::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c, '{' | '}') && chars.peek() == Some(&c) {
            chars.next();
        } else if c == '{' {
            let mut field = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                field.push(c);
            }
            if !closed {
                return Err("Unclosed template placeholder".into());
            }
            fields.insert(
                field
                    .parse::<usize>()
                    .map_err(|_| "Expected numbered template placeholder")?,
            );
        } else if c == '}' {
            return Err("Unescaped closing brace".into());
        }
    }
    Ok(fields)
}

pub(super) fn render(template: &str, values: &[String]) -> String {
    let mut output = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c, '{' | '}') && chars.peek() == Some(&c) {
            chars.next();
            output.push(c);
        } else if c == '{' {
            let field: String = chars.by_ref().take_while(|c| *c != '}').collect();
            if let Some(value) = field.parse::<usize>().ok().and_then(|i| values.get(i)) {
                output.push_str(value);
            } else {
                output.push_str(&format!("[argument:{field}]"));
            }
        } else {
            output.push(c);
        }
    }
    output
}
