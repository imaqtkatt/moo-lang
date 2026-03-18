use crate::{
    lexer::{Lexer, Token},
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
    Cascade,
    // TODO: x bar baz: _ is currently the same as Call(x, barbaz:, _)
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
pub enum ParseError {}

type ParseResult<T> = std::result::Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        Self { lexer, current }
    }

    fn expect(&mut self, expected: Token) -> ParseResult<Token> {
        if self.peek() == &expected {
            Ok(self.eat())
        } else {
            panic!("expected {expected:?} but got {:?}", self.peek())
        }
    }

    #[allow(unused)]
    fn consume(&mut self, token: &Token) -> bool {
        if self.peek() == token {
            self.eat();
            true
        } else {
            false
        }
    }

    fn eat(&mut self) -> crate::lexer::Token {
        std::mem::replace(&mut self.current, self.lexer.next_token())
    }

    fn peek(&self) -> &crate::lexer::Token {
        &self.current
    }

    fn is(&self, token: &Token) -> bool {
        self.peek() == token
    }

    fn parse_ident(&mut self) -> ParseResult<String> {
        match self.eat() {
            Token::Ident(i) => Ok(i),
            x => panic!("parse ident: {x:?}"),
        }
    }

    fn parse_keyword(&mut self) -> ParseResult<String> {
        if let Token::Keyword(k) = self.eat() {
            Ok(k)
        } else {
            panic!("parse keyword")
        }
    }

    fn parse_primary(&mut self) -> ParseResult<ast::Expression> {
        match self.eat() {
            Token::Ident(i) => Ok(ast::Expression::Variable(i)),
            Token::Number(n) => Ok(ast::Expression::Constant(ast::Constant::Integer(n))),
            Token::String(s) => Ok(ast::Expression::Constant(ast::Constant::String(s))),
            Token::True => Ok(ast::Expression::Constant(ast::Constant::Boolean(true))),
            Token::False => Ok(ast::Expression::Constant(ast::Constant::Boolean(false))),
            Token::Null => Ok(ast::Expression::Constant(ast::Constant::Null)),
            Token::TSelf => Ok(ast::Expression::SelfRef),
            Token::LParens => {
                let e = self.parse_expression()?;
                self.expect(Token::RParens)?;

                Ok(ast::Expression::Group(Box::new(e)))
            }
            x => panic!("parse primary: {x:?}"),
        }
    }

    fn parse_expression(&mut self) -> ParseResult<ast::Expression> {
        match self.peek() {
            Token::Let => self.parse_let_in(),
            Token::New => self.parse_new(),
            Token::If => self.parse_if(),
            _ => self.parse_infix(Precedence::Lowest),
        }
    }

    fn parse_let_in(&mut self) -> ParseResult<ast::Expression> {
        self.expect(Token::Let)?;
        let ident = self.parse_ident()?;
        self.expect(Token::Equal)?;
        let value = self.parse_expression()?;
        self.expect(Token::In)?;
        let next = self.parse_expression()?;

        Ok(ast::Expression::LetIn(
            ident,
            Box::new(value),
            Box::new(next),
        ))
    }

    fn parse_new(&mut self) -> ParseResult<ast::Expression> {
        self.expect(Token::New)?;
        let class_name = self.parse_ident()?;
        let generics = self.parse_generics(|p| p.parse_type())?;

        let mut field_init = Vec::new();

        while let Token::Keyword(_) = self.peek() {
            let keyword = self.parse_keyword()?;
            let parameter = self.parse_infix(Precedence::KeywordCall.left())?;

            field_init.push((keyword, parameter));
        }

        Ok(ast::Expression::Instantiate(
            class_name, generics, field_init,
        ))
    }

    fn parse_if(&mut self) -> ParseResult<ast::Expression> {
        self.expect(Token::If)?;

        if self.consume(&Token::Let) {
            let nullable = self.parse_expression()?;

            let refined = if self.consume(&Token::As) {
                Some(self.parse_ident()?)
            } else {
                None
            };

            self.expect(Token::Then)?;
            let consequence = self.parse_expression()?;
            self.expect(Token::Else)?;
            let alternative = self.parse_expression()?;

            Ok(ast::Expression::IfLetThenElse(
                Box::new(nullable),
                refined,
                Box::new(consequence),
                Box::new(alternative),
            ))
        } else {
            let condition = self.parse_expression()?;
            self.expect(Token::Then)?;
            let consequence = self.parse_expression()?;
            self.expect(Token::Else)?;
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
                    Token::Equal => Self::parse_assign,
                    Token::Comma => Self::parse_cascade,
                    Token::Semicolon => Self::parse_seq,
                    Token::Ident(_) => Self::parse_unary_call,
                    Token::Keyword(_) => Self::parse_keyword_call,
                    t => todo!("todo with {t:?}"),
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
            Token::Number(_) => Precedence::End,
            Token::String(_) => Precedence::End,
            Token::True => Precedence::End,
            Token::False => Precedence::End,
            Token::Null => Precedence::End,
            Token::TSelf => Precedence::End,
            //
            Token::Ident(_) => Precedence::UnaryCall,
            Token::Keyword(_) => Precedence::KeywordCall,
            Token::LParens => Precedence::UnaryCall,
            //
            Token::RParens => Precedence::End,
            Token::LBracket | Token::RBracket => Precedence::End,
            Token::Comma => Precedence::Cascade,
            Token::Semicolon => Precedence::Seq,
            Token::Equal => Precedence::Assign,
            Token::FatArrow => Precedence::End,
            Token::QuestionMark => Precedence::End,
            Token::TypeInt => Precedence::End,
            Token::TypeBool => Precedence::End,
            Token::TypeStr => Precedence::End,
            Token::TypeVoid => Precedence::End,
            Token::Class => Precedence::End,
            Token::New => Precedence::End,
            Token::If | Token::Then | Token::Else => Precedence::End,
            Token::Let | Token::As | Token::In => Precedence::End,
            Token::Def => Precedence::End,
            Token::ErrorChar(_) | Token::ErrorString(_) => Precedence::End,
            Token::Eof => Precedence::End,
        }
    }

    fn parse_assign(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        self.expect(Token::Equal)?;
        let rhs = self.parse_infix(Precedence::Assign.left())?;
        Ok(ast::Expression::Assignment(Box::new(lhs), Box::new(rhs)))
    }

    fn parse_cascade(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        let (mut receiver, mut messages) = match lhs {
            ast::Expression::Call(receiver, selector, arguments) => {
                (*receiver, vec![(selector, arguments)])
            }
            x => panic!("parse cascade: {x:?}"),
        };

        loop {
            self.expect(Token::Comma)?;

            match self.parse_call(receiver)? {
                ast::Expression::Call(new_receiver, selector, arguments) => {
                    messages.push((selector, arguments));
                    receiver = *new_receiver;
                }
                _ => panic!("parse cascade"),
            }

            if !self.is(&Token::Comma) {
                break;
            }
        }

        Ok(ast::Expression::Cascade(Box::new(receiver), messages))
    }

    fn parse_seq(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        self.expect(Token::Semicolon)?;
        let rhs = self.parse_infix(Precedence::Seq.left())?;
        Ok(ast::Expression::Seq(Box::new(lhs), Box::new(rhs)))
    }

    fn parse_call(&mut self, lhs: ast::Expression) -> ParseResult<ast::Expression> {
        match self.peek() {
            Token::Ident(_) => self.parse_unary_call(lhs),
            Token::Keyword(_) => self.parse_keyword_call(lhs),
            _ => unreachable!(),
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
        match self.eat() {
            Token::Ident(i) => self.parse_generic_type(i),
            Token::TypeVoid => Ok(ast::Type::Void),
            Token::TypeInt => Ok(ast::Type::Int),
            Token::TypeBool => Ok(ast::Type::Bool),
            Token::TypeStr => Ok(ast::Type::Str),
            Token::LParens => {
                let t = self.parse_type()?;
                self.expect(Token::RParens)?;
                Ok(t)
            }
            _ => panic!("primary type"),
        }
    }

    fn parse_type(&mut self) -> ParseResult<ast::Type> {
        match self.peek() {
            Token::QuestionMark => self.parse_nullable_type(),
            _ => self.parse_primary_type(),
        }
    }

    fn parse_nullable_type(&mut self) -> ParseResult<ast::Type> {
        self.expect(Token::QuestionMark)?;
        let t = self.parse_type()?;
        Ok(ast::Type::Nullable(Box::new(t)))
    }

    fn parse_generic_type(&mut self, named: String) -> ParseResult<ast::Type> {
        let types = self.parse_generics(|p| p.parse_type())?;

        Ok(ast::Type::Named(named, types))
    }
}

impl<'a> Parser<'a> {
    pub fn parse_program(&mut self) -> ParseResult<ast::Program> {
        let mut top_levels = Vec::new();

        loop {
            if self.peek() == &Token::Eof {
                break;
            }

            let top_level = self.parse_top_level()?;
            top_levels.push(top_level);
        }

        Ok(ast::Program(top_levels))
    }

    fn parse_top_level(&mut self) -> ParseResult<ast::TopLevel> {
        match self.peek() {
            Token::Class => self.parse_class().map(ast::TopLevel::ClassDefinition),
            Token::Let => self
                .parse_method_let()
                .map(ast::TopLevel::MethodDeclaration),
            Token::Def => self.parse_method_def().map(ast::TopLevel::MethodDefinition),
            x => panic!("parse top level: {x:?}"),
        }
    }

    fn parse_class(&mut self) -> ParseResult<ast::ClassDefinition> {
        self.expect(Token::Class)?;
        let class_name = self.parse_ident()?;

        let generics = self.parse_generics(|p| p.parse_ident())?;

        let mut fields = vec![];

        while let Token::Keyword(_) = self.peek() {
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
        if let Token::Class = self.peek() {
            self.expect(Token::Class)?;
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

        if self.consume(&Token::LBracket) {
            while !self.is(&Token::RBracket) {
                generics.push(f(self)?);
                if self.consume(&Token::Comma) {
                    continue;
                } else {
                    break;
                }
            }
            self.expect(Token::RBracket)?;
        }

        Ok(generics)
    }

    fn parse_method_let(&mut self) -> ParseResult<ast::MethodDeclaration> {
        self.expect(Token::Let)?;
        let method_type = self.parse_method_type()?;

        let class_name = self.parse_ident()?;

        let mut selector = Selector::new();
        let mut parameters = Vec::new();

        if let Token::Keyword(_) = self.peek() {
            while let Token::Keyword(_) = self.peek() {
                let parameter = self.parse_keyword()?;
                let parameter_type = self.parse_type()?;

                selector = selector.push(&parameter);
                parameters.push(parameter_type);
            }
        } else {
            let unary = self.parse_ident()?;
            selector = Selector::unary(&unary);
        }

        self.expect(Token::FatArrow)?;

        let return_type = self.parse_type()?;

        Ok(ast::MethodDeclaration {
            method_type,
            receiver: class_name,
            selector,
            parameters,
            body: return_type,
        })
    }

    fn parse_method_def(&mut self) -> ParseResult<ast::MethodDefinition> {
        self.expect(Token::Def)?;
        let method_type = self.parse_method_type()?;

        let class_name = self.parse_ident()?;

        let mut selector = Selector::new();
        let mut parameters = Vec::new();

        if let Token::Keyword(_) = self.peek() {
            while let Token::Keyword(_) = self.peek() {
                let keyword = self.parse_keyword()?;
                let parameter = self.parse_ident()?;

                selector = selector.push(&keyword);
                parameters.push(parameter);
            }
        } else {
            let unary = self.parse_ident()?;
            selector = Selector::unary(&unary);
        }

        self.expect(Token::FatArrow)?;

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
