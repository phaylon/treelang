use src_ctx::{Input, Span};

use super::token;

pub trait InputExt<'a> {
    fn input(&self) -> &Input<'a>;

    fn split_line(&self) -> (Input<'a>, Option<Input<'a>>) {
        let input = self.input();
        if let Some(index) = input.content().find('\n') {
            (input.truncate(index), Some(input.skip(index + 1)))
        } else {
            (input.clone(), None)
        }
    }

    fn skip_whitespace_and_comments(&self) -> Input<'a> {
        let input = self.input();
        let content = input.content().trim_start();
        if content.starts_with(token::COMMENT) {
            input.end()
        } else {
            let len = input.content().len() - content.len();
            input.skip(len)
        }
    }

    fn try_take_chars<F>(&self, mut is_taken: F) -> Option<(&'a str, Span, Input<'a>)>
    where
        F: FnMut(char) -> bool,
    {
        let input = self.input();
        let index = input.content().find(|c| !is_taken(c))
            .unwrap_or_else(|| input.content().len());
        if index > 0 {
            let rest = input.skip(index);
            Some((
                &input.content()[..index],
                input.offset().span(rest.offset()),
                rest,
            ))
        } else {
            None
        }
    }
}

impl<'a> InputExt<'a> for Input<'a> {
    fn input(&self) -> &Input<'a> {
        self
    }
}
