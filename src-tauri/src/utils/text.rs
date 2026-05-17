use html2text::render::text_renderer::TrivialDecorator;

pub fn html_to_plain_text(html: &str) -> String {
    html2text::from_read_with_decorator(html.as_bytes(), usize::MAX, TrivialDecorator::new())
}

pub fn rtf_to_plain_text(rtf: &str) -> String {
    let decoded = decode_rtf_hex_escapes(rtf);
    let mut output = String::with_capacity(decoded.len());
    let mut chars = decoded.chars().peekable();
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if consume_rtf_control(&mut chars) {
                    pending_space = true;
                }
            }
            '{' | '}' => {
                pending_space = true;
            }
            '\r' | '\n' | '\t' => {
                push_space_once(&mut output, &mut pending_space);
            }
            ch if ch.is_whitespace() => {
                pending_space = true;
            }
            ch => {
                push_space_once(&mut output, &mut pending_space);
                output.push(ch);
            }
        }
    }

    output.trim().to_string()
}

fn decode_rtf_hex_escapes(rtf: &str) -> String {
    let mut output = String::with_capacity(rtf.len());
    let mut chars = rtf.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'\'') {
            chars.next();
            let hi = chars.next();
            let lo = chars.next();
            if let (Some(hi), Some(lo)) = (hi, lo) {
                let mut hex = String::with_capacity(2);
                hex.push(hi);
                hex.push(lo);
                if let Ok(value) = u8::from_str_radix(&hex, 16) {
                    output.push(value as char);
                    continue;
                }
                output.push('\\');
                output.push('\'');
                output.push(hi);
                output.push(lo);
                continue;
            }
            output.push('\\');
            output.push('\'');
            if let Some(hi) = hi {
                output.push(hi);
            }
            continue;
        }

        output.push(ch);
    }

    output
}

fn consume_rtf_control<I>(chars: &mut std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = char>,
{
    match chars.peek().copied() {
        Some('\\' | '{' | '}') => {
            chars.next();
            false
        }
        Some('~') => {
            chars.next();
            true
        }
        Some('-' | '_') => {
            chars.next();
            false
        }
        Some('*') => {
            chars.next();
            false
        }
        Some(ch) if ch.is_ascii_alphabetic() => {
            while matches!(chars.peek(), Some(ch) if ch.is_ascii_alphabetic()) {
                chars.next();
            }
            if matches!(chars.peek(), Some('-')) {
                chars.next();
            }
            while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
                chars.next();
            }
            if matches!(chars.peek(), Some(' ')) {
                chars.next();
            }
            true
        }
        Some(_) => {
            chars.next();
            false
        }
        None => false,
    }
}

fn push_space_once(output: &mut String, pending_space: &mut bool) {
    if *pending_space && !output.is_empty() && !output.ends_with(' ') {
        output.push(' ');
    }
    *pending_space = false;
}

#[cfg(test)]
mod tests {
    use super::{html_to_plain_text, rtf_to_plain_text};

    #[test]
    fn html_plain_text_does_not_emit_markdown_markers() {
        let html = "<h1>Title</h1><p><strong>Bold</strong> text</p>";
        let plain = html_to_plain_text(html);

        assert!(plain.contains("Title"));
        assert!(plain.contains("Bold text"));
        assert!(!plain.contains("# Title"));
        assert!(!plain.contains("**Bold**"));
    }

    #[test]
    fn rtf_plain_text_removes_control_words() {
        let rtf = r"{\rtf1\ansi\b Bold\b0  text\par Title}";

        assert_eq!(rtf_to_plain_text(rtf), "Bold text Title");
    }
}
