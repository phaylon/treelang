use crate::{Span, Offset};

use super::token;


#[derive(Clone)]
pub struct Input<'a> {
    content: &'a str,
    offset: Offset,
}

impl<'a> Input<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            offset: Offset::new(),
        }
    }

    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub fn to_end(&self) -> Self {
        self.skip_bytes(self.content.len())
    }

    fn skip_bytes(&self, len: usize) -> Self {
        let lines = self.content[..len].split('\n').count() - 1;
        Self {
            content: &self.content[len..],
            offset: self.offset.increase_bytes(len).increase_line_number(lines),
        }
    }

    fn truncate(&self, len: usize) -> Self {
        Self {
            content: &self.content[..len],
            offset: self.offset,
        }
    }

    pub fn next_char(&self) -> Option<char> {
        self.content.chars().next()
    }

    pub fn leading_whitespace_span(&self) -> Option<Span> {
        let len = self.content.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum();
        (len > 0).then(|| self.offset.span(len))
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn try_skip_char_sequence<I>(&self, chars: I) -> Option<Self>
    where
        I: IntoIterator<Item = char>,
    {
        let mut len = 0;
        for (search_c, content_c) in chars.into_iter().zip(self.content.chars()) {
            if search_c != content_c {
                return None;
            }
            len += search_c.len_utf8();
        }
        Some(self.skip_bytes(len))
    }

    pub fn try_skip_char(&self, c: char) -> Option<Self> {
        if self.content.starts_with(c) {
            Some(self.skip_bytes(c.len_utf8()))
        } else {
            None
        }
    }

    pub fn skip_whitespace_and_comments(&self) -> Self {
        let content = self.content.trim_start();
        if content.starts_with(token::COMMENT) {
            self.to_end()
        } else {
            let len = self.content.len() - content.len();
            self.skip_bytes(len)
        }
    }

    pub fn split_line(self) -> (Self, Option<Self>) {
        if let Some(index) = self.content.find('\n') {
            (self.truncate(index), Some(self.skip_bytes(index + 1)))
        } else {
            (self.clone(), None)
        }
    }

    pub fn try_take_chars<F>(&self, mut is_taken: F) -> Option<(&str, Span, Self)>
    where
        F: FnMut(char) -> bool,
    {
        let index = self.content.find(|c| !is_taken(c)).unwrap_or(self.content.len());
        if index > 0 {
            Some((
                &self.content[..index],
                self.offset.span(index),
                self.skip_bytes(index),
            ))
        } else {
            None
        }
    }
}
