use std::fmt::Write;


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset {
    byte: usize,
    line_number: usize,
}

impl Offset {
    pub(crate) fn new() -> Self {
        Self { byte: 0, line_number: 1 }
    }

    pub(crate) fn increase_bytes(&self, len: usize) -> Self {
        Self { byte: self.byte + len, line_number: self.line_number }
    }

    pub(crate) fn increase_line_number(&self, lines: usize) -> Self {
        Self { byte: self.byte, line_number: self.line_number + lines }
    }

    pub(crate) fn span(&self, len: usize) -> Span {
        Span { offset: *self, len }
    }

    pub fn line_number(&self) -> usize {
        self.line_number
    }
}

impl From<Span> for Offset {
    fn from(span: Span) -> Self {
        span.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    offset: Offset,
    len: usize,
}

impl Span {
    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub fn line_number(&self) -> usize {
        self.offset.line_number
    }
}

pub trait SourceContext {
    fn full_str(&self) -> &str;

    fn line_span_before<T>(&self, offset: T) -> Option<Span>
    where
        T: Into<Offset>,
    {
        let line_span = self.line_span(offset);
        if line_span.offset.byte > 0 {
            let offset = Offset {
                byte: line_span.offset.byte - 1,
                line_number: line_span.offset.line_number - 1,
            };
            Some(self.line_span(offset))
        } else {
            None
        }
    }

    fn line_span<T>(&self, offset: T) -> Span
    where
        T: Into<Offset>,
    {
        let offset = offset.into();
        let content = self.full_str();
        let line_offset = content[..offset.byte].rfind('\n').map(|n| n + 1).unwrap_or(0);
        let content_rest = &content[line_offset..];
        let line_len = content_rest.find('\n').unwrap_or(content_rest.len());
        Span {
            offset: Offset {
                byte: line_offset,
                line_number: content[..line_offset].split('\n').count(),
            },
            len: line_len,
        }
    }

    fn span_str(&self, span: Span) -> &str {
        &self.full_str()[span.offset.byte..(span.offset.byte + span.len)]
    }

    fn line_str<T>(&self, offset: T) -> &str
    where
        T: Into<Offset>,
    {
        self.span_str(self.line_span(offset))
    }

    fn byte_offset_on_line<T>(&self, offset: T) -> usize
    where
        T: Into<Offset>,
    {
        let offset = offset.into();
        offset.byte - self.line_span(offset).offset.byte
    }

    fn offset_highlight_line(&self, offset: Offset) -> Highlight<'_> {
        let byte_offset = self.byte_offset_on_line(offset);
        let lead_template = &self.line_str(offset)[..byte_offset];
        Highlight { lead_template, len: 1 }
    }

    fn span_highlight_line(&self, span: Span) -> Highlight<'_> {
        let byte_offset = self.byte_offset_on_line(span);
        let lead_template = &self.line_str(span)[..byte_offset];
        let len = self.span_str(span).chars().count();
        Highlight { lead_template, len }
    }

    fn offset_section<T>(&self, offset: T) -> Section<'_>
    where
        T: Into<Offset>,
    {
        let offset = offset.into();
        Section {
            full_source_contest: self.full_str(),
            line_span: self.line_span(offset),
            highlight: self.offset_highlight_line(offset),
        }
    }

    fn span_section(&self, span: Span) -> Section<'_> {
        Section {
            full_source_contest: self.full_str(),
            line_span: self.line_span(span),
            highlight: self.span_highlight_line(span),
        }
    }
}

impl SourceContext for str {
    fn full_str(&self) -> &str {
        self
    }
}

impl SourceContext for String {
    fn full_str(&self) -> &str {
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    full_source_contest: &'a str,
    line_span: Span,
    highlight: Highlight<'a>,
}

impl<'a> std::fmt::Display for Section<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let previous_line_span = self.full_source_contest.line_span_before(self.line_span);

        let first_line_number = previous_line_span
            .map_or(self.line_span.line_number(), |span| span.line_number());
        let last_line_number = self.line_span.line_number();
        let line_number_len = count_digits(last_line_number);

        if first_line_number > 1 {
            writeln!(f, " {1:0$} | ...", line_number_len, first_line_number - 1)?;
        }
        if let Some(span) = previous_line_span {
            let line_content = self.full_source_contest.line_str(span);
            writeln!(f, " {1:0$} | {2}", line_number_len, span.line_number(), line_content)?;
        }
        let line_content = self.full_source_contest.line_str(self.line_span);
        writeln!(f, " {1:0$} | {2}", line_number_len, self.line_span.line_number(), line_content)?;
        writeln!(f, " {1:0$} | {2}", line_number_len, "", self.highlight)?;

        Ok(())
    }
}

fn count_digits(mut n: usize) -> usize {
    if n == 0 {
        1
    } else {
        let mut digits = 0;
        while n > 0 {
            digits += 1;
            n /= 10;
        }
        digits
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Highlight<'a> {
    lead_template: &'a str,
    len: usize,
}

impl<'a> std::fmt::Display for Highlight<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for c in self.lead_template.chars() {
            f.write_char(match c { '\t' => '\t', _ => ' ' })?;
        }
        for _ in 0..self.len {
            f.write_char('^')?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(byte: usize, line_number: usize) -> Offset {
        Offset { byte, line_number }
    }

    #[test]
    fn line_spans() {
        let content = "abc\ndef";
        assert_eq!(content.line_span(offset(0, 1)), Span { offset: offset(0, 1), len: 3 });
        assert_eq!(content.line_span(offset(3, 1)), Span { offset: offset(0, 1), len: 3 });
        assert_eq!(content.line_span(offset(4, 2)), Span { offset: offset(4, 2), len: 3 });
        assert_eq!(content.line_span(offset(7, 2)), Span { offset: offset(4, 2), len: 3 });
    }

    #[test]
    fn span_strs() {
        let content = "abc\ndef";
        assert_eq!(content.span_str(Span { offset: offset(4, 2), len: 3 }), "def");
    }

    #[test]
    fn line_span_before() {
        let content = "abc\ndef";
        assert_eq!(
            content.line_span_before(offset(4, 2)),
            Some(Span { offset: offset(0, 1), len: 3})
        );
    }

    #[test]
    fn line_strs() {
        let content = "abc\ndef";
        assert_eq!(content.line_str(offset(0, 1)), "abc");
        assert_eq!(content.line_str(offset(3, 1)), "abc");
        assert_eq!(content.line_str(offset(4, 2)), "def");
        assert_eq!(content.line_str(offset(7, 2)), "def");
    }

    #[test]
    fn byte_offsets_on_line() {
        let content = "abc\ndef";
        assert_eq!(content.byte_offset_on_line(offset(0, 1)), 0);
        assert_eq!(content.byte_offset_on_line(offset(3, 1)), 3);
        assert_eq!(content.byte_offset_on_line(offset(4, 2)), 0);
        assert_eq!(content.byte_offset_on_line(offset(7, 2)), 3);
    }
}