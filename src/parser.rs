use crate::{
    lexer::{Lexer, Token, TokenType},
    shared::Selector,
    tree::ast,
};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: crate::lexer::Token,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Lowest = 0,
    Seq,
    Assign,
    Pipe,
    Cascade,
    KeywordCall,
    UnaryCall,
    End,
}

impl Precedence {
    fn left(&self) -> Self {
        if let Precedence::End = self {
            unreachable!()
        }
        unsafe { std::mem::transmute(*self as u8 + 1) }
    }
}

#[derive(Clone, Debug)]
pub enum ParseError {
    UnexpectedToken(Token),
    ExpectedButGot {
        expected: crate::lexer::TokenType,
        got: Token,
    },

    ExpectedCall,
    ExpectedTopLevel,
    ExpectedIdent,
    ExpectedKeyword,
}

type ParseResult<T> = std::result::Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        Self { lexer, current }
    }

    fn expect(&mut self, expected: crate::lexer::TokenType) -> ParseResult<Token> {
        if self.peek() == expected {
            Ok(self.eat())
        } else {
            Err(ParseError::ExpectedButGot {
                expected,
                got: self.current,
            })
        }
    }

    #[allow(unused)]
    fn consume(&mut self, token_type: crate::lexer::TokenType) -> bool {
        if self.peek() == token_type {
            self.eat();
            true
        } else {
            false
        }
    }

    fn eat(&mut self) -> crate::lexer::Token {
        std::mem::replace(&mut self.current, self.lexer.next_token())
    }

    fn peek(&self) -> crate::lexer::TokenType {
        self.current.token_type
    }

    fn is(&self, token_type: crate::lexer::TokenType) -> bool {
        self.peek() == token_type
    }

    fn just<T>(&mut self, value: T) -> ParseResult<T> {
        self.eat();
        Ok(value)
    }

    fn parse_ident(&mut self) -> ParseResult<String> {
        let token = self.eat();
        match token.token_type {
            TokenType::Ident => {
                let lexeme = self.lexer.lexeme(token);
                // println!("lexeme = {lexeme:?}");
                Ok(String::from(lexeme))
            }
            _ => Err(ParseError::ExpectedIdent),
        }
    }

    fn parse_keyword(&mut self) -> ParseResult<String> {
        let token = self.eat();
        match token.token_type {
            TokenType::Keyword => {
                let lexeme = self.lexer.lexeme(token);
                Ok(String::from(lexeme))
            }
            _ => Err(ParseError::ExpectedKeyword),
        }
    }

    fn parse_primary(&mut self) -> ParseResult<ast::Expression> {
        match self.peek() {
            TokenType::Ident => self.parse_ident().map(ast::Expression::Variable),
            TokenType::Number => Ok(ast::Expression::Constant(ast::Constant::Integer({
                let token = self.eat();
                let lexeme = self.lexer.lexeme(token);
                lexeme.parse().unwrap()
            }))),
            TokenType::String => Ok(ast::Expression::Constant(ast::Constant::String({
                let token = self.eat();
                let lexeme = self.lexer.lexeme(token);
                String::from(lexeme)
            }))),
            TokenType::True => self.just(ast::Expression::Constant(ast::Constant::Boolean(true))),
            TokenType::False => self.just(ast::Expression::Constant(ast::Constant::Boolean(false))),
            TokenType::Null => self.just(ast::Expression::Constant(ast::Constant::Null)),
            TokenType::SelfRef => self.just(ast::Expression::SelfRef),
            TokenType::LParens => {
                self.expect(TokenType::LParens)?;
                let e = self.parse_expression()?;
                self.expect(TokenType::RParens)?;

                Ok(ast::Expression::Group(Box::new(e)))
            }
            _ => Err(ParseError::UnexpectedToken(self.current)),
        }
    }

    fn parse_expression(&mut self) -> ParseResult<ast::Expression> {
        match self.peek() {
            TokenType::Let => self.parse_let_in(),
            TokenType::New => self.parse_new(),
            TokenType::If => self.parse_if(),
            _ => self.parse_infix(Precedence::Lowest),
        }
    }

    fn parse_let_in(&mut self) -> ParseResult<ast::Expression> {
        self.expect(TokenType::Let)?;
        let ident = self.parse_ident()?;
        self.expect(TokenType::Equal)?;
        let value = self.parse_expression()?;
        self.expect(TokenType::In)?;
        let next = self.parse_expression()?;

        Ok(ast::Expression::LetIn(
            ident,
            Box::new(value),
            Box::new(next),
        ))
    }

    fn parse_new(&mut self) -> ParseResult<ast::Expression> {
        self.expect(TokenType::New)?;
        let class_name = self.parse_ident()?;
        let generics = self.parse_generics(|p| p.parse_type())?;

        let mut field_init = Vec::new();

        while let TokenType::Keyword = self.peek() {
            let keyword = self.parse_keyword()?;
            let parameter = self.parse_infix(Precedence::KeywordCall.left())?;

            field_init.push((keyword, parameter));
        }

        Ok(ast::Expression::Instantiate(
            class_name, generics, field_init,
        ))
    }

    fn parse_if(&mut self) -> ParseResult<ast::Expression> {
        self.expect(TokenType::If)?;

        if self.consume(TokenType::Let) {
            let nullable = self.parse_expression()?;

            let refined = if self.consume(TokenType::As) {
                Some(self.parse_ident()?)
            } else {
                None
            };

            self.expect(TokenType::Then)?;
            let consequence = self.parse_expression()?;
            self.expect(TokenType::Else)?;
            let alternative = self.parse_expression()?;

            Ok(ast::Expression::IfLetThenElse(
                Box::new(nullable),
                refined,
                Box::new(consequence),
                Box::new(alternative),
            ))
        } else {
            let condition = self.parse_expression()?;
            self.expect(TokenType::Then)?;
            let consequence = self.parse_expression()?;
            self.expect(TokenType::Else)?;
            let alternative = self.parse_expression()?;

            Ok(ast::Expression::IfThenElse(
                Box::new(condition),
                Box::new(consequence),
                Box::new(alternative),
            ))
        }
    }

    fn parse_infix(&mut self, precedence: Precedence) -> ParseResult<ast::Expression> {
        let mut lhs = self.parse_primary()?;

        loop {
            let p = self.precedence();
            if p >= precedence && p != Precedence::End {
                let rule = match self.peek() {
                    TokenType::Equal => Self::parse_assign,
                    TokenType::Comma => Self::parse_cascade,
                    TokenType::Semicolon => Self::parse_seq,
                    TokenType::Ampersand => Self::parse_pipe,
                    TokenType::Ident => Self::parse_unary_call,
                    TokenType::Keyword => Self::parse_keyword_call,
                    _ => unreachable!(),
                };
                lhs = rule(self, lhs)?;
            } else {
                break;
            }
        }

        Ok(lhs)
    }

    fn precedence(&self) -> Precedence {
        match self.peek() {
            TokenType::Number => Precedence::End,
            TokenType::String => Precedence::End,
            TokenType::True => Precedence::End,
            TokenType::False => Precedence::End,
            TokenType::Null => Precedence::End,
            TokenType::SelfRef => Precedence::End,
            //
            TokenType::Ident => Precedence::UnaryCall,
            TokenType::Keyword => Precedence::KeywordCall,
            TokenType::LParens => Precedence::UnaryCall,
            //
            TokenType::RParens => Precedence::End,
            TokenType::LBracket | TokenType::RBracket => Precedence::End,
            TokenType::Comma => Precedence::Cascade,
            TokenType::Semicolon => Precedence::Seq,
            TokenType::Ampersand => Precedence::Pipe,
            TokenType::Equal => Precedence::Assign,
            TokenType::FatArrow => Precedence::End,
            TokenType::QuestionMark => Precedence::End,
            TokenType::TypeInt => Precedence::End,
            TokenType::TypeBool => Precedence::End,
            TokenType::TypeStr => Precedence::End,
            TokenType::TypeVoid => Precedence::End,
            TokenType::Class => Precedence::End,
            TokenType::New => Precedence::End,
            TokenType::If | TokenType::Then | TokenType::Else => Precedence::End,
            TokenType::Let | TokenType::As | TokenType::In => Precedence::End,
            TokenType::Def => Precedence::End,
            TokenType::Error => Precedence::End,
            TokenType::Eof => Precedence::End,
        }
    }

    fn parse_assign(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        self.expect(TokenType::Equal)?;
        let rhs = self.parse_infix(Precedence::Assign.left())?;
        Ok(ast::Expression::Assignment(Box::new(lhs), Box::new(rhs)))
    }

    fn parse_cascade(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        let (mut receiver, mut messages) = match lhs {
            ast::Expression::Call(receiver, selector, arguments) => {
                (*receiver, vec![(selector, arguments)])
            }
            _ => Err(ParseError::ExpectedCall)?,
        };

        loop {
            self.expect(TokenType::Comma)?;

            match self.parse_call(receiver)? {
                ast::Expression::Call(new_receiver, selector, arguments) => {
                    messages.push((selector, arguments));
                    receiver = *new_receiver;
                }
                _ => unreachable!(),
            }

            if !self.is(TokenType::Comma) {
                break;
            }
        }

        Ok(ast::Expression::Cascade(Box::new(receiver), messages))
    }

    fn parse_seq(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        self.expect(TokenType::Semicolon)?;
        let rhs = self.parse_infix(Precedence::Seq.left())?;
        Ok(ast::Expression::Seq(Box::new(lhs), Box::new(rhs)))
    }

    fn parse_pipe(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        let (mut receiver, mut messages) = match lhs {
            ast::Expression::Call(receiver, selector, arguments) => {
                (*receiver, vec![(selector, arguments)])
            }
            other => (other, vec![]),
        };

        loop {
            self.expect(TokenType::Ampersand)?;

            match self.parse_call(receiver)? {
                ast::Expression::Call(new_receiver, selector, arguments) => {
                    messages.push((selector, arguments));
                    receiver = *new_receiver;
                }
                _ => unreachable!(),
            }

            if !self.is(TokenType::Ampersand) {
                break;
            }
        }

        Ok(ast::Expression::Pipe(Box::new(receiver), messages))
    }

    fn parse_call(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        match self.peek() {
            TokenType::Ident => self.parse_unary_call(lhs),
            TokenType::Keyword => self.parse_keyword_call(lhs),
            _ => Err(ParseError::ExpectedCall),
        }
    }

    fn parse_unary_call(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        let selector = self.parse_ident().map(|op| Selector::unary(&op))?;
        Ok(ast::Expression::Call(
            Box::new(lhs),
            ast::CallType::Unary(selector),
            vec![],
        ))
    }

    fn parse_keyword_call(&mut self, mut lhs: ast::Expression) -> ParseResult<ast::Expression> {
        let keyword = self.parse_keyword()?;
        let argument = self.parse_infix(Precedence::KeywordCall.left())?;

        if let ast::Expression::Call(_r, ast::CallType::Keyword(s), a) = &mut lhs {
            *s = s.push(&keyword);
            a.push(argument);

            Ok(lhs)
        } else {
            Ok(ast::Expression::Call(
                Box::new(lhs),
                ast::CallType::Keyword(Selector::new().push(&keyword)),
                vec![argument],
            ))
        }
    }
}

impl<'a> Parser<'a> {
    fn parse_primary_type(&mut self) -> ParseResult<ast::Type> {
        match self.peek() {
            TokenType::Ident => self.parse_generic_type(),
            // TokenType::TypeVoid => self.just(ast::Type::Void),
            TokenType::TypeInt => self.just(ast::Type::Int),
            TokenType::TypeBool => self.just(ast::Type::Bool),
            TokenType::TypeStr => self.just(ast::Type::Str),
            TokenType::LParens => {
                self.expect(TokenType::LParens)?;
                let t = self.parse_type()?;
                self.expect(TokenType::RParens)?;
                Ok(t)
            }
            _ => Err(ParseError::UnexpectedToken(self.current)),
        }
    }

    fn parse_type(&mut self) -> ParseResult<ast::Type> {
        match self.peek() {
            TokenType::QuestionMark => self.parse_nullable_type(),
            _ => self.parse_primary_type(),
        }
    }

    fn parse_nullable_type(&mut self) -> ParseResult<ast::Type> {
        self.expect(TokenType::QuestionMark)?;
        let t = self.parse_type()?;
        Ok(ast::Type::Nullable(Box::new(t)))
    }

    fn parse_generic_type(&mut self) -> ParseResult<ast::Type> {
        let named = self.parse_ident()?;
        let types = self.parse_generics(|p| p.parse_type())?;

        Ok(ast::Type::Named(named, types))
    }
}

impl<'a> Parser<'a> {
    pub fn parse_program(&mut self) -> ParseResult<ast::Program> {
        let mut top_levels = Vec::new();

        loop {
            if self.is(TokenType::Eof) {
                break;
            }

            let top_level = self.parse_top_level()?;
            top_levels.push(top_level);
        }

        Ok(ast::Program(top_levels))
    }

    fn parse_top_level(&mut self) -> ParseResult<ast::TopLevel> {
        match self.peek() {
            TokenType::Class => self.parse_class().map(ast::TopLevel::ClassDefinition),
            TokenType::Let => self
                .parse_method_let()
                .map(ast::TopLevel::MethodDeclaration),
            TokenType::Def => self.parse_method_def().map(ast::TopLevel::MethodDefinition),
            _ => Err(ParseError::ExpectedTopLevel),
        }
    }

    fn parse_class(&mut self) -> ParseResult<ast::ClassDefinition> {
        self.expect(TokenType::Class)?;
        let class_name = self.parse_ident()?;

        let generics = self.parse_generics(|p| p.parse_ident())?;

        let mut fields = vec![];

        while let TokenType::Keyword = self.peek() {
            let field_name = self.parse_keyword()?;
            let field_type = self.parse_type()?;

            fields.push((field_name, field_type));
        }

        Ok(ast::ClassDefinition {
            class_name,
            generics,
            fields,
        })
    }

    fn parse_method_type(&mut self) -> ParseResult<ast::MethodType> {
        if let TokenType::Class = self.peek() {
            self.expect(TokenType::Class)?;
            Ok(ast::MethodType::Class)
        } else {
            Ok(ast::MethodType::Instance)
        }
    }

    fn parse_generics<T>(
        &mut self,
        f: impl Fn(&mut Parser) -> ParseResult<T>,
    ) -> ParseResult<Vec<T>> {
        let mut generics = vec![];

        if self.consume(TokenType::LBracket) {
            while !self.is(TokenType::RBracket) {
                generics.push(f(self)?);
                if self.consume(TokenType::Comma) {
                    continue;
                } else {
                    break;
                }
            }
            self.expect(TokenType::RBracket)?;
        }

        Ok(generics)
    }

    fn parse_method_let(&mut self) -> ParseResult<ast::MethodDeclaration> {
        self.expect(TokenType::Let)?;
        let method_type = self.parse_method_type()?;

        let class_name = self.parse_ident()?;

        let mut selector = Selector::new();
        let mut parameters = Vec::new();

        if let TokenType::Keyword = self.peek() {
            while let TokenType::Keyword = self.peek() {
                let parameter = self.parse_keyword()?;
                let parameter_type = self.parse_type()?;

                selector = selector.push(&parameter);
                parameters.push(parameter_type);
            }
        } else {
            let unary = self.parse_ident()?;
            selector = Selector::unary(&unary);
        }

        let return_type = if self.is(TokenType::FatArrow) {
            self.expect(TokenType::FatArrow)?;
            let return_type = self.parse_type()?;
            Some(return_type)
        } else {
            None
        };

        Ok(ast::MethodDeclaration {
            method_type,
            receiver: class_name,
            selector,
            parameters,
            body: return_type,
        })
    }

    fn parse_method_def(&mut self) -> ParseResult<ast::MethodDefinition> {
        self.expect(TokenType::Def)?;
        let method_type = self.parse_method_type()?;

        let class_name = self.parse_ident()?;

        let mut selector = Selector::new();
        let mut parameters = Vec::new();

        if let TokenType::Keyword = self.peek() {
            while let TokenType::Keyword = self.peek() {
                let keyword = self.parse_keyword()?;
                let parameter = self.parse_ident()?;

                selector = selector.push(&keyword);
                parameters.push(parameter);
            }
        } else {
            let unary = self.parse_ident()?;
            selector = Selector::unary(&unary);
        }

        self.expect(TokenType::FatArrow)?;

        let body = self.parse_expression()?;

        Ok(ast::MethodDefinition {
            method_type,
            receiver: class_name,
            selector,
            parameters,
            body,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::{lexer::Lexer, parser::Parser};

    #[test]
    fn test_parse_expression() {
        let source = r#"
            let x = a b c in
            point x: x y: 2;
            3 ;
            5
        "#;

        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);

        let e = parser.parse_expression();

        match e {
            Ok(e) => println!("{e:?}"),
            Err(e) => eprintln!("{e:?}"),
        }
    }

    #[test]
    fn test_parse_class() {
        let source = r#"
            class Point
                x: int
                y: int

            def let in
        "#;

        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);

        let c = parser.parse_class();

        match c {
            Ok(class) => println!("{class:?}"),
            Err(e) => eprintln!("{e:?}"),
        }
    }

    #[test]
    fn test_parse_program() {
        let source = include_str!("../examples/simple.moo");

        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();

        match program {
            Ok(p) => println!("{p:?}"),
            Err(e) => eprintln!("{e:?}"),
        }
    }
}
