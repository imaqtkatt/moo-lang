pub mod ast {
    use crate::shared::Selector;

    #[derive(Clone, Debug)]
    pub enum Expression {
        Variable(String),
        Constant(Constant),
        ESelf,
        LetIn(String, Box<Expression>, Box<Expression>),
        IfThenElse(Box<Expression>, Box<Expression>, Box<Expression>),
        IfLetThenElse(
            Box<Expression>,
            Option<String>,
            Box<Expression>,
            Box<Expression>,
        ),
        Seq(Box<Expression>, Box<Expression>),
        Cascade(Box<Expression>, Vec<(Selector, Vec<Expression>)>),
        Assignment(Box<Expression>, Box<Expression>),
        Call(Box<Expression>, Selector, Vec<Expression>),
        Instantiate(String, Vec<(String, Expression)>),
        Group(Box<Expression>),
    }

    #[derive(Clone, Debug)]
    pub enum Constant {
        Null,
        Integer(i32),
        Boolean(bool),
        String(String),
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Type {
        Void,
        Int,
        Bool,
        Str,

        Named(String),
        Nullable(Box<Type>),
    }

    #[derive(Clone, Debug)]
    pub struct ClassDefinition {
        pub class_name: String,
        pub fields: Vec<(String, Type)>,
    }

    #[derive(Clone, Debug)]
    pub struct MethodDeclaration {
        pub method_type: MethodType,
        pub receiver: String,
        pub selector: Selector,
        pub parameters: Vec<Type>,
        pub body: Type,
    }

    #[derive(Clone, Debug)]
    pub struct MethodDefinition {
        pub method_type: MethodType,
        pub receiver: String,
        pub selector: Selector,
        pub parameters: Vec<String>,
        pub body: Expression,
    }

    #[derive(Clone, Debug)]
    pub enum MethodType {
        Class,
        Instance,
    }

    #[derive(Clone, Debug)]
    pub enum TopLevel {
        ClassDefinition(ClassDefinition),
        MethodDeclaration(MethodDeclaration),
        MethodDefinition(MethodDefinition),
    }

    #[derive(Clone, Debug)]
    pub struct Program(pub Vec<TopLevel>);
}

pub mod typed {
    use crate::shared::Selector;

    #[derive(Clone, Debug)]
    pub enum Expression {
        Variable(String),
        Constant(Constant),
        ESelf,
        LetIn(String, Typed<Expression>, Typed<Expression>),
        IfThenElse(Typed<Expression>, Typed<Expression>, Typed<Expression>),
        IfLetThenElse(
            Typed<Expression>,
            Option<String>,
            Typed<Expression>,
            Typed<Expression>,
        ),
        Seq(Typed<Expression>, Typed<Expression>),
        Cascade(Typed<Expression>, Vec<(Selector, Vec<Typed<Expression>>)>),
        Load(String),
        Store(String, Typed<Expression>),
        InstanceCall(Typed<Expression>, Selector, Vec<Typed<Expression>>),
        ClassCall(String, Selector, Vec<Typed<Expression>>),
        Instantiate(String, Vec<(String, Typed<Expression>)>),
    }

    #[derive(Clone, Debug)]
    pub enum Constant {
        Null,
        Integer(i32),
        Boolean(bool),
        String(String),
    }

    #[derive(Clone, Debug)]
    pub struct Typed<A: Clone> {
        pub value: Box<A>,
        pub r#type: crate::sema::Type,
    }

    #[derive(Clone, Debug)]
    pub struct ClassDefinition {
        pub class_type: crate::sema::ClassType,
        pub class_name: String,
        pub fields: Vec<(String, crate::sema::Type)>,
    }

    #[derive(Clone, Debug)]
    pub struct MethodDefinition {
        pub method_type: MethodType,
        pub receiver: String,
        pub selector: Selector,
        pub parameters: Vec<String>,
        pub body: Typed<Expression>,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum MethodType {
        Class,
        Instance,
    }

    pub enum TopLevel {
        ClassDefinition(ClassDefinition),
        MethodDefinition(MethodDefinition),
    }

    pub struct Program(pub Vec<TopLevel>);

    impl From<crate::tree::ast::MethodType> for MethodType {
        fn from(value: crate::tree::ast::MethodType) -> Self {
            match value {
                super::ast::MethodType::Class => Self::Class,
                super::ast::MethodType::Instance => Self::Instance,
            }
        }
    }
}

#[allow(unused)]
pub mod ir {
    #[derive(Clone, Debug)]
    pub enum Expression {
        Variable(Local),
        Constant(Constant),
        SelfRef,

        Let(Local, Box<Expression>, Box<Expression>),

        If(Box<Expression>, Box<Expression>, Box<Expression>),
        Seq(Box<Expression>, Box<Expression>),

        FieldGet(Box<Expression>, FieldId),
        FieldSet(Box<Expression>, FieldId, Box<Expression>),

        InstanceCall(Box<Expression>, MethodId, Vec<Expression>),
        ClassCall(ClassId, MethodId, Vec<Expression>),

        Instantiate(ClassId, Vec<Expression>),
    }

    #[derive(Clone, Debug)]
    pub enum Constant {
        Null,
        Int(i32),
        Bool(bool),
        Str(String),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Local(usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FieldId(usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct MethodId(usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ClassId(usize);
}
