use std::{iter::Peekable, str::Chars};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Ident(String),
    Number(i32),
    String(String),
    True,
    False,
    Null,
    TSelf,

    Keyword(String),

    LParens,
    RParens,
    LBracket,
    RBracket,

    Comma,
    Semicolon,
    Equal,
    FatArrow,
    QuestionMark,

    TypeInt,
    TypeBool,
    TypeStr,
    TypeVoid,

    Class,
    New,
    If,
    Then,
    Else,
    Let,
    As,
    In,
    Def,

    ErrorChar(char),
    ErrorString(String),
    Eof,
}

pub struct Lexer<'a> {
    index: usize,
    start: usize,

    source: &'a str,
    peekable: Peekable<Chars<'a>>,
}

fn is_ident(c: &char) -> bool {
    " \r\n\t()[]=>:;,".find(*c).is_none()
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            index: 0,
            start: 0,
            source,
            peekable: source.chars().peekable(),
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.peekable.next()?;
        self.index += c.len_utf8();
        Some(c)
    }

    fn save(&mut self) {
        self.start = self.index;
    }

    fn consume(&mut self, c: char) -> bool {
        if let Some(d) = self.peekable.peek().copied() {
            if c == d {
                _ = self.next_char().unwrap();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn skip_while(&mut self, predicate: impl Fn(&char) -> bool) {
        while let Some(c) = self.peekable.peek() {
            if predicate(c) {
                _ = self.next_char().unwrap();
            } else {
                break;
            }
        }
    }

    fn skip_whitespaces(&mut self) {
        self.skip_while(char::is_ascii_whitespace);
    }

    fn skip_comments(&mut self) {
        if self.consume('-') && self.consume('-') {
            self.skip_while(|c| *c != '\n');
        }
    }

    fn skip(&mut self) {
        while let Some(c) = self.peekable.peek() {
            match c {
                c if c.is_ascii_whitespace() => self.skip_whitespaces(),
                '-' => {
                    let mut it = self.peekable.clone();
                    it.next();

                    if let Some('-') = it.peek() {
                        self.skip_comments();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self) -> Token {
        self.save();
        let mut buf = String::with_capacity(16);

        loop {
            match self.peekable.peek() {
                Some('\'') => break,
                Some(_) => buf.push(self.next_char().unwrap()),
                None => return Token::ErrorString(buf),
            }
        }

        self.consume('\'');

        Token::String(buf)
    }

    pub fn next_token(&mut self) -> Token {
        // self.skip_whitespaces();
        self.skip();
        self.save();

        if let Some(c) = self.next_char() {
            match c {
                '(' => Token::LParens,
                ')' => Token::RParens,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                ',' => Token::Comma,
                ';' => Token::Semicolon,
                '?' => Token::QuestionMark,
                '\'' => self.string(),
                '=' if self.consume('>') => Token::FatArrow,
                '=' => Token::Equal,
                '-' => todo!(),
                c if c.is_ascii_digit() => {
                    self.skip_while(char::is_ascii_digit);

                    let lexeme = String::from(&self.source[self.start..self.index]);
                    Token::Number(lexeme.parse().unwrap())
                }
                c if c.is_ascii_alphanumeric() => {
                    self.skip_while(is_ident);
                    let lexeme = String::from(&self.source[self.start..self.index]);
                    if self.consume(':') {
                        Token::Keyword(lexeme)
                    } else {
                        self.ident(lexeme)
                    }
                }
                c => Token::ErrorChar(c),
            }
        } else {
            Token::Eof
        }
    }

    fn ident(&self, lexeme: String) -> Token {
        match lexeme.as_str() {
            "let" => Token::Let,
            "in" => Token::In,
            "def" => Token::Def,
            "class" => Token::Class,
            "new" => Token::New,
            "if" => Token::If,
            "as" => Token::As,
            "then" => Token::Then,
            "else" => Token::Else,
            "true" => Token::True,
            "false" => Token::False,
            "null" => Token::Null,
            "self" => Token::TSelf,
            "int" => Token::TypeInt,
            "bool" => Token::TypeBool,
            "str" => Token::TypeStr,
            "void" => Token::TypeVoid,
            _ => Token::Ident(lexeme),
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.source.len() {
            None
        } else {
            Some(self.next_token())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::lexer::Lexer;

    #[test]
    fn test_lexer() {
        let source = r#"
            class Person name: str

            let Person set-name: str => void
            def Person set-name: new-name => name = new-name
        "#;

        let mut lexer = Lexer::new(source);
        while let Some(token) = lexer.next() {
            println!("{token:?}");
        }
    }
}
