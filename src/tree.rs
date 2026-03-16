pub mod ast {
    use crate::shared::Selector;

    #[derive(Clone, Debug)]
    pub enum Expression {
        Variable(String),
        Constant(Constant),
        SelfRef,
        LetIn(String, Box<Expression>, Box<Expression>),
        IfThenElse(Box<Expression>, Box<Expression>, Box<Expression>),
        IfLetThenElse(
            Box<Expression>,
            Option<String>,
            Box<Expression>,
            Box<Expression>,
        ),
        Seq(Box<Expression>, Box<Expression>),
        Cascade(Box<Expression>, Vec<(CallType, Vec<Expression>)>),
        Assignment(Box<Expression>, Box<Expression>),
        Call(Box<Expression>, CallType, Vec<Expression>),
        Instantiate(String, Vec<(String, Expression)>),
        Group(Box<Expression>),
    }

    #[derive(Clone, Debug)]
    pub enum CallType {
        Unary(Selector),
        Keyword(Selector),
    }

    impl CallType {
        pub fn selector(self) -> Selector {
            match self {
                CallType::Unary(selector) => selector,
                CallType::Keyword(selector) => selector,
            }
        }
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
        SelfRef,
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
        pub parameter_types: Vec<crate::sema::Type>,
        pub return_type: crate::sema::Type,
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
    use std::rc::Rc;

    use crate::{sema, shared::Selector};

    #[derive(Clone, Debug)]
    pub enum Expr {
        Variable(Local),
        Constant(Constant),
        SelfRef,

        Let(Local, Expression, Expression),

        If(Expression, Expression, Expression),
        Seq(Expression, Expression),

        FieldGet(Expression, FieldId, usize),
        FieldSet(Expression, FieldId, usize, Expression),

        InstanceCall(Expression, MethodId, Vec<Expression>),
        ClassCall(ClassId, MethodId, Vec<Expression>),

        Instantiate(ClassId, Vec<Expression>),

        NotNull(Expression),
    }

    pub type Expression = Rc<Expr>;

    #[derive(Clone, Debug)]
    pub enum Constant {
        Null,
        Int(i32),
        Bool(bool),
        Str(String),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Local(pub usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct FieldId(pub usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct MethodId(pub usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct ClassId(pub usize);

    #[derive(Clone, Debug)]
    pub struct Program {
        pub classes: Vec<Class>,
        pub methods: Vec<Method>,
        pub fields: Vec<Field>,

        pub entrypoint: Option<MethodId>,
    }

    impl Program {
        pub fn new(classes: Vec<Class>, methods: Vec<Method>, fields: Vec<Field>) -> Self {
            Self {
                classes,
                methods,
                fields,
                entrypoint: None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MethodType {
        Class,
        Instance,
    }

    impl From<crate::tree::typed::MethodType> for MethodType {
        fn from(value: crate::tree::typed::MethodType) -> Self {
            match value {
                super::typed::MethodType::Class => Self::Class,
                super::typed::MethodType::Instance => Self::Instance,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct Class {
        pub id: ClassId,
        pub name: String,
        pub fields: Vec<FieldId>,
        pub methods: Vec<MethodId>,
    }

    #[derive(Clone, Debug)]
    pub struct Field {
        pub id: FieldId,
        pub offset: usize,
        pub name: String,
        pub ty: sema::Type,
    }

    #[derive(Clone, Debug)]
    pub struct Method {
        pub id: MethodId,
        pub receiver: ClassId,
        pub selector: Selector,
        pub method_type: MethodType,
        pub parameters: Vec<sema::Type>,
        pub return_type: sema::Type,
        pub locals: usize,
        pub body: Expression,
    }
}
