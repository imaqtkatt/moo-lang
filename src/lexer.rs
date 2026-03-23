use std::{iter::Peekable, str::Chars};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TokenType {
    Ident,
    Number,
    String,
    True,
    False,
    Null,
    SelfRef,

    Keyword,

    LParens,
    RParens,
    LBracket,
    RBracket,

    Comma,
    Semicolon,
    Ampersand,
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

    Error,
    Eof,
}

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub token_type: TokenType,
    pub start: usize,
    pub end: usize,
}

pub struct Lexer<'a> {
    index: usize,
    start: usize,

    source: &'a str,
    peekable: Peekable<Chars<'a>>,
}

// TODO: revise idents
fn is_ident(c: &char) -> bool {
    " \r\n\t()[]=>:;,&".find(*c).is_none()
}

fn is_ident_end(c: &char) -> bool {
    const IDENT_END: &'static str = "!?";
    todo!()
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

    const fn token(&self, token_type: TokenType) -> Token {
        Token {
            token_type,
            start: self.start,
            end: self.index,
        }
    }

    const fn keyword(&self) -> Token {
        Token {
            token_type: TokenType::Keyword,
            start: self.start,
            end: self.index - 1,
        }
    }

    fn string(&mut self) -> Token {
        self.save();
        let mut buf = String::with_capacity(16);

        loop {
            match self.peekable.peek() {
                Some('\'') => break,
                Some(_) => buf.push(self.next_char().unwrap()),
                None => return self.token(TokenType::Error),
            }
        }

        self.consume('\'');

        Token {
            token_type: TokenType::String,
            start: self.start,
            end: self.index - 1,
        }
    }

    pub fn next_token(&mut self) -> Token {
        // self.skip_whitespaces();
        self.skip();
        self.save();

        let token_type = if let Some(c) = self.next_char() {
            match c {
                '(' => TokenType::LParens,
                ')' => TokenType::RParens,
                '[' => TokenType::LBracket,
                ']' => TokenType::RBracket,
                ',' => TokenType::Comma,
                ';' => TokenType::Semicolon,
                '&' => TokenType::Ampersand,
                '?' => TokenType::QuestionMark,
                '\'' => return self.string(),
                '=' if self.consume('>') => TokenType::FatArrow,
                '=' => TokenType::Equal,
                '-' => todo!(),
                c if c.is_ascii_digit() => {
                    self.skip_while(char::is_ascii_digit);

                    // let lexeme = String::from(&self.source[self.start..self.index]);
                    TokenType::Number
                }
                c if c.is_ascii_alphanumeric() => {
                    self.skip_while(is_ident);
                    let lexeme = String::from(&self.source[self.start..self.index]);
                    if self.consume(':') {
                        return self.keyword();
                    } else {
                        self.ident(lexeme)
                    }
                }
                _ => TokenType::Error,
            }
        } else {
            TokenType::Eof
        };

        self.token(token_type)
    }

    fn ident(&self, lexeme: String) -> TokenType {
        match lexeme.as_str() {
            "let" => TokenType::Let,
            "in" => TokenType::In,
            "def" => TokenType::Def,
            "class" => TokenType::Class,
            "new" => TokenType::New,
            "if" => TokenType::If,
            "as" => TokenType::As,
            "then" => TokenType::Then,
            "else" => TokenType::Else,
            "true" => TokenType::True,
            "false" => TokenType::False,
            "null" => TokenType::Null,
            "self" => TokenType::SelfRef,
            "int" => TokenType::TypeInt,
            "bool" => TokenType::TypeBool,
            "str" => TokenType::TypeStr,
            "void" => TokenType::TypeVoid,
            _ => TokenType::Ident,
        }
    }
}

impl Token {
    pub fn lexeme<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

impl<'a> Lexer<'a> {
    pub fn lexeme(&self, token: Token) -> &'a str {
        token.lexeme(self.source)
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
