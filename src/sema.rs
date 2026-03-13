use std::{collections::BTreeMap, ptr::null};

use crate::{
    shared::Selector,
    tree::{self},
};

#[derive(Debug)]
pub struct Context {
    next_class: u32,

    pub classes: BTreeMap<String, Class>,

    pub instance_methods: BTreeMap<(ClassType, Selector), Method>,
    pub class_methods: BTreeMap<(ClassType, Selector), Method>,

    locals: BTreeMap<String, (Bind, Type)>,
}

#[derive(Clone, Debug)]
pub enum AnalysisError {
    UnboundVariable(String),
    UnboundInstanceMethod(Selector),
    UnboundClassMethod(Selector),
    UnboundClass(String),
    MissingDeclaration(Selector),
    ExpectedClass,
    ExpectedVariable,
    ExpectedField,
    ConstructorInitError,
    FieldInitError,
    TypeError { expected: Type, got: Type },
}

#[derive(Clone, Debug)]
pub struct Method {
    param_types: Vec<Type>,
    return_type: Type,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub class_type: ClassType,
    pub fields: Vec<(String, Type)>,
}

impl Class {
    fn non_defined(class_type: ClassType) -> Self {
        Self {
            class_type,
            fields: Vec::new(),
        }
    }

    fn with_field(mut self, field_name: String, field_type: Type) -> Self {
        self.fields.push((field_name, field_type));
        self
    }
}

#[derive(Clone, Copy, Debug)]
enum Bind {
    Field,
    Local,
}

#[derive(Clone, Debug)]
pub enum Type {
    Void,
    Int,
    Bool,
    Str,
    Class(ClassType),
    Nullable(Box<Type>),

    Null,
}

impl Type {
    fn subtype(a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Void, Type::Void) => true,
            (Type::Int, Type::Int) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Str, Type::Str) => true,
            (Type::Class(a), Type::Class(b)) => a == b,

            (Type::Nullable(a), Type::Nullable(b)) => Type::subtype(a, b),
            (Type::Null, Type::Nullable(_)) => true,
            (a, Type::Nullable(b)) => Type::subtype(a, b),

            (_, _) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ClassType(u32);

pub trait Analyze {
    type Output;

    fn analyze(self, ctx: &mut Context) -> Result<Self::Output, AnalysisError>;
}

pub trait Declare {
    type Output;

    fn declare(&self, ctx: Context) -> Result<(Self::Output, Context), AnalysisError>;
}

pub trait Define {
    type Output;

    fn define(self, ctx: &mut Context) -> Result<Self::Output, AnalysisError>;
}

fn type_equality(a: &Type, b: &Type) -> Result<(), AnalysisError> {
    if Type::subtype(a, b) {
        Ok(())
    } else {
        Err(AnalysisError::TypeError {
            expected: b.clone(),
            got: a.clone(),
        })
    }
}

fn expect_class(a: &Type) -> Result<ClassType, AnalysisError> {
    if let Type::Class(class_type) = a {
        Ok(*class_type)
    } else {
        Err(AnalysisError::ExpectedClass)
    }
}

impl Context {
    fn new_class(&mut self) -> ClassType {
        let current = self.next_class;
        self.next_class += 1;
        ClassType(current)
    }

    fn scope<T>(
        &mut self,
        name: &str,
        value: (Bind, Type),
        scope: impl FnOnce(&mut Context) -> T,
    ) -> T {
        self.bind(name, value);
        let result = scope(self);
        _ = self.locals.remove(name);
        result
    }

    fn lookup_local(&self, name: &str) -> Result<(Bind, Type), AnalysisError> {
        self.locals
            .get(name)
            .cloned()
            .ok_or(AnalysisError::UnboundVariable(String::from(name)))
    }

    fn bind(&mut self, name: &str, value: (Bind, Type)) -> Option<(Bind, Type)> {
        self.locals.insert(String::from(name), value)
    }

    fn lookup_class(&self, class_name: &str) -> Result<Class, AnalysisError> {
        self.classes
            .get(class_name)
            .cloned()
            .ok_or(AnalysisError::UnboundClass(String::from(class_name)))
    }

    fn lookup_instance_method(
        &self,
        class_type: ClassType,
        selector: Selector,
    ) -> Result<Method, AnalysisError> {
        self.instance_methods
            .get(&(class_type, selector.clone()))
            .cloned()
            .ok_or(AnalysisError::UnboundInstanceMethod(selector))
    }

    fn lookup_class_method(
        &self,
        class_type: ClassType,
        selector: Selector,
    ) -> Result<Method, AnalysisError> {
        self.class_methods
            .get(&(class_type, selector.clone()))
            .cloned()
            .ok_or(AnalysisError::UnboundClassMethod(selector))
    }
}

impl Analyze for tree::ast::Expression {
    type Output = tree::typed::Typed<tree::typed::Expression>;

    fn analyze(self, ctx: &mut Context) -> Result<Self::Output, AnalysisError> {
        match self {
            tree::ast::Expression::Variable(name) => {
                let (bind, t) = ctx.lookup_local(&name)?;

                let elaborated = match bind {
                    Bind::Field => tree::typed::Expression::Load(name),
                    Bind::Local => tree::typed::Expression::Variable(name),
                };

                Ok(Self::Output {
                    value: Box::new(elaborated),
                    r#type: t,
                })
            }
            tree::ast::Expression::Constant(constant) => {
                let (constant, r#type) = match constant {
                    tree::ast::Constant::Null => (tree::typed::Constant::Null, Type::Null),
                    tree::ast::Constant::Integer(i) => {
                        (tree::typed::Constant::Integer(i), Type::Int)
                    }
                    tree::ast::Constant::Boolean(b) => {
                        (tree::typed::Constant::Boolean(b), Type::Bool)
                    }
                    tree::ast::Constant::String(s) => (tree::typed::Constant::String(s), Type::Str),
                };

                let constant = tree::typed::Expression::Constant(constant);

                Ok(Self::Output {
                    value: Box::new(constant),
                    r#type,
                })
            }
            tree::ast::Expression::ESelf => {
                let e_self = tree::typed::Expression::ESelf;

                Ok(Self::Output {
                    value: Box::new(e_self),
                    r#type: ctx.locals["self"].1.clone(),
                })
            }
            tree::ast::Expression::LetIn(bind, value, next) => {
                let value = value.analyze(ctx)?;

                let next = ctx.scope(&bind, (Bind::Local, value.r#type.clone()), |ctx| {
                    next.analyze(ctx)
                })?;
                let next_t = next.r#type.clone();

                let let_in = tree::typed::Expression::LetIn(bind, value, next);

                Ok(Self::Output {
                    value: Box::new(let_in),
                    r#type: next_t,
                })
            }
            tree::ast::Expression::IfThenElse(condition, consequence, alternative) => {
                let condition = condition.analyze(ctx)?;
                type_equality(&condition.r#type, &Type::Bool)?;

                let consequence = consequence.analyze(ctx)?;
                let alternative = alternative.analyze(ctx)?;

                type_equality(&consequence.r#type, &alternative.r#type)?;

                let branch_type = consequence.r#type.clone();
                let if_then_else =
                    tree::typed::Expression::IfThenElse(condition, consequence, alternative);

                Ok(Self::Output {
                    value: Box::new(if_then_else),
                    r#type: branch_type,
                })
            }
            tree::ast::Expression::IfLetThenElse(nullable, refined, consequence, alternative) => {
                if let tree::ast::Expression::Variable(name) = *nullable {
                    let (bind, t) = ctx.lookup_local(&name)?;
                    let inner = expect_nullable(&t)?;

                    if refined.is_some() {
                        todo!("alias on variable")
                    }

                    let consequence =
                        ctx.scope(&name, (bind, inner), |ctx| consequence.analyze(ctx))?;

                    let alternative = alternative.analyze(ctx)?;

                    type_equality(&consequence.r#type, &alternative.r#type)?;
                    let branch_type = consequence.r#type.clone();

                    let nullable = match bind {
                        Bind::Field => tree::typed::Expression::Load(name.clone()),
                        Bind::Local => tree::typed::Expression::Variable(name.clone()),
                    };
                    let nullable = tree::typed::Typed {
                        value: Box::new(nullable),
                        r#type: t,
                    };

                    let if_let_then_else = tree::typed::Expression::IfLetThenElse(
                        nullable,
                        Some(name),
                        consequence,
                        alternative,
                    );

                    Ok(Self::Output {
                        value: Box::new(if_let_then_else),
                        r#type: branch_type,
                    })
                } else {
                    let nullable = nullable.analyze(ctx)?;
                    let inner = expect_nullable(&nullable.r#type)?;

                    let consequence = if let Some(name) = &refined {
                        ctx.scope(name, (Bind::Local, inner.clone()), |ctx| {
                            consequence.analyze(ctx)
                        })?
                    } else {
                        consequence.analyze(ctx)?
                    };

                    let alternative = alternative.analyze(ctx)?;

                    type_equality(&consequence.r#type, &alternative.r#type)?;

                    let branch_type = consequence.r#type.clone();
                    let if_let_then_else = tree::typed::Expression::IfLetThenElse(
                        nullable,
                        refined,
                        consequence,
                        alternative,
                    );

                    Ok(Self::Output {
                        value: Box::new(if_let_then_else),
                        r#type: branch_type,
                    })
                }
            }
            tree::ast::Expression::Seq(a, b) => {
                let a = a.analyze(ctx)?;
                type_equality(&a.r#type, &Type::Void)?;

                let b = b.analyze(ctx)?;
                let seq_type = b.r#type.clone();

                let seq = tree::typed::Expression::Seq(a, b);

                Ok(Self::Output {
                    value: Box::new(seq),
                    r#type: seq_type,
                })
            }
            tree::ast::Expression::Cascade(receiver, messages) => {
                assert!(!messages.is_empty());

                let receiver = receiver.analyze(ctx)?;
                let class_type = expect_class(&receiver.r#type)?;

                let mut new_messages = vec![];
                let mut return_type = Type::Void;

                for (selector, arguments) in messages {
                    let method = ctx.lookup_instance_method(class_type, selector.clone())?;

                    assert!(method.param_types.len() == arguments.len());

                    let arguments = arguments
                        .into_iter()
                        .map(|a| a.analyze(ctx))
                        .collect::<Result<Vec<_>, _>>()?;

                    for (a, b) in arguments.iter().zip(method.param_types.iter()) {
                        type_equality(&a.r#type, b)?;
                    }

                    new_messages.push((selector, arguments));
                    return_type = method.return_type.clone();
                }

                let cascade = tree::typed::Expression::Cascade(receiver, new_messages);

                Ok(Self::Output {
                    value: Box::new(cascade),
                    r#type: return_type,
                })
            }
            tree::ast::Expression::Assignment(field, new_value) => {
                let tree::ast::Expression::Variable(name) = l_value(*field)? else {
                    unreachable!()
                };

                let (bind, t) = ctx.lookup_local(&name)?;
                if let Bind::Local = bind {
                    Err(AnalysisError::ExpectedVariable)?
                }

                let new_value = new_value.analyze(ctx)?;
                type_equality(&new_value.r#type, &t)?;

                let field_set = tree::typed::Expression::Store(name, new_value);

                Ok(Self::Output {
                    value: Box::new(field_set),
                    r#type: Type::Void,
                })
            }
            tree::ast::Expression::Call(receiver, selector, arguments) => {
                // if receiver is referencing a class name generate a ClassCall

                let arguments = arguments
                    .into_iter()
                    .map(|a| a.analyze(ctx))
                    .collect::<Result<Vec<_>, _>>()?;

                if let tree::ast::Expression::Variable(name) = &*receiver
                    && let Ok(class) = ctx.lookup_class(name)
                {
                    let method = ctx.lookup_class_method(class.class_type, selector.clone())?;

                    assert!(method.param_types.len() == arguments.len());

                    for (a, b) in arguments.iter().zip(method.param_types.iter()) {
                        type_equality(&a.r#type, b)?;
                    }

                    let class_call =
                        tree::typed::Expression::ClassCall(name.clone(), selector, arguments);

                    Ok(Self::Output {
                        value: Box::new(class_call),
                        r#type: method.return_type,
                    })
                } else {
                    let receiver = receiver.analyze(ctx)?;
                    let class_type = expect_class(&receiver.r#type)?;

                    let method = ctx.lookup_instance_method(class_type, selector.clone())?;

                    assert!(method.param_types.len() == arguments.len());

                    for (a, b) in arguments.iter().zip(method.param_types.iter()) {
                        type_equality(&a.r#type, b)?;
                    }

                    let instance_call =
                        tree::typed::Expression::InstanceCall(receiver, selector, arguments);

                    Ok(Self::Output {
                        value: Box::new(instance_call),
                        r#type: method.return_type,
                    })
                }
            }
            tree::ast::Expression::Instantiate(class_name, field_init) => {
                let class = ctx.lookup_class(&class_name)?;

                let constructor = class.fields.clone();

                let field_init = field_init
                    .into_iter()
                    .map(|(name, value)| value.analyze(ctx).map(|v| (name, v)))
                    .collect::<Result<Vec<_>, _>>()?;

                if constructor.len() != field_init.len() {
                    Err(AnalysisError::ConstructorInitError)?
                }

                for ((a, b), (c, d)) in constructor.iter().zip(field_init.iter()) {
                    if a != c {
                        Err(AnalysisError::FieldInitError)?
                    }
                    type_equality(&d.r#type, b)?;
                }

                let instantiate = tree::typed::Expression::Instantiate(class_name, field_init);

                Ok(Self::Output {
                    value: Box::new(instantiate),
                    r#type: Type::Class(class.class_type),
                })
            }
            tree::ast::Expression::Group(e) => e.analyze(ctx),
        }
    }
}

fn l_value(tree: tree::ast::Expression) -> Result<tree::ast::Expression, AnalysisError> {
    if let tree::ast::Expression::Variable(_) = &tree {
        Ok(tree)
    } else {
        Err(AnalysisError::ExpectedVariable)
    }
}

fn expect_nullable(t: &Type) -> Result<Type, AnalysisError> {
    if let Type::Nullable(inner) = t {
        Ok(*inner.clone())
    } else {
        panic!("expect nullable")
    }
}

impl Analyze for tree::ast::Type {
    type Output = Type;

    fn analyze(self, context: &mut Context) -> Result<Self::Output, AnalysisError> {
        let analyzed = match self {
            tree::ast::Type::Void => Self::Output::Void,
            tree::ast::Type::Int => Self::Output::Int,
            tree::ast::Type::Bool => Self::Output::Bool,
            tree::ast::Type::Str => Self::Output::Str,
            tree::ast::Type::Named(c) => Self::Output::Class(context.lookup_class(&c)?.class_type),
            tree::ast::Type::Nullable(inner) => {
                let inner = inner.analyze(context)?;
                Self::Output::Nullable(Box::new(inner))
            }
        };
        Ok(analyzed)
    }
}

impl Declare for tree::ast::ClassDefinition {
    type Output = ();

    fn declare(&self, mut context: Context) -> Result<(Self::Output, Context), AnalysisError> {
        let this_class = context.new_class();

        context
            .classes
            .insert(self.class_name.clone(), Class::non_defined(this_class));

        Ok(((), context))
    }
}

impl Define for tree::ast::ClassDefinition {
    type Output = tree::typed::ClassDefinition;

    fn define(self, context: &mut Context) -> Result<Self::Output, AnalysisError> {
        let mut this_class = context.lookup_class(&self.class_name)?;
        let class_type = this_class.class_type;

        for (name, t) in self.fields.iter() {
            let t = t.clone().analyze(context)?;
            this_class.fields.push((name.clone(), t))
        }

        let fields = this_class.fields.clone();
        context.classes.insert(self.class_name.clone(), this_class);

        Ok(Self::Output {
            class_type,
            class_name: self.class_name,
            fields,
        })
    }
}

impl Declare for tree::ast::MethodDeclaration {
    type Output = ();

    fn declare(&self, mut context: Context) -> Result<(Self::Output, Context), AnalysisError> {
        let class = context.lookup_class(&self.receiver)?;

        let param_types = self
            .parameters
            .iter()
            .cloned()
            .map(|t| t.analyze(&mut context))
            .collect::<Result<Vec<_>, _>>()?;
        let return_type = self.body.clone().analyze(&mut context)?;

        let method = Method {
            param_types,
            return_type,
        };

        match self.method_type {
            tree::ast::MethodType::Class => {
                context
                    .class_methods
                    .insert((class.class_type, self.selector.clone()), method);
            }
            tree::ast::MethodType::Instance => {
                context
                    .instance_methods
                    .insert((class.class_type, self.selector.clone()), method);
            }
        };

        Ok(((), context))
    }
}

impl Define for tree::ast::MethodDefinition {
    type Output = tree::typed::MethodDefinition;

    fn define(self, context: &mut Context) -> Result<Self::Output, AnalysisError> {
        let class = context.lookup_class(&self.receiver)?;

        let method = match self.method_type {
            tree::ast::MethodType::Class => {
                context.lookup_class_method(class.class_type, self.selector.clone())
            }
            tree::ast::MethodType::Instance => {
                context.lookup_instance_method(class.class_type, self.selector.clone())
            }
        };
        let method = method.map_err(|e| match e {
            AnalysisError::UnboundClassMethod(s) | AnalysisError::UnboundInstanceMethod(s) => {
                AnalysisError::MissingDeclaration(s)
            }
            _ => unreachable!(),
        })?;

        let body = {
            if matches!(self.method_type, tree::ast::MethodType::Instance) {
                for (a, b) in class.fields.iter().cloned() {
                    _ = context.bind(&a, (Bind::Field, b));
                }
            }
            for (a, b) in self.parameters.iter().zip(method.param_types.iter()) {
                _ = context.bind(a, (Bind::Local, b.clone()));
            }

            context.locals.insert(
                String::from("self"),
                (Bind::Local, Type::Class(class.class_type)),
            );

            let body = self.body.analyze(context)?;
            type_equality(&body.r#type, &method.return_type)?;
            context.locals.clear();
            body
        };

        Ok(Self::Output {
            method_type: self.method_type.into(),
            receiver: self.receiver.clone(),
            selector: self.selector.clone(),
            parameters: self.parameters.clone(),
            body,
        })
    }
}

// TODO: should assert that 'Main main' exists and has signature '[] => void'
pub fn analyze_program(
    tree: tree::ast::Program,
) -> Result<(tree::typed::Program, Context), AnalysisError> {
    let decls = tree.0;

    let mut context = Context {
        next_class: 0,
        classes: BTreeMap::new(),

        instance_methods: BTreeMap::new(),
        class_methods: BTreeMap::new(),

        locals: BTreeMap::new(),
    };

    let string_class = {
        let string_class = context.new_class();
        context.classes.insert(
            String::from("String"),
            Class::non_defined(string_class).with_field(String::from("inner"), Type::Str),
        );
        context.class_methods.insert(
            (string_class, Selector::new().push("str")),
            Method {
                param_types: vec![Type::Str],
                return_type: Type::Class(string_class),
            },
        );
        context.instance_methods.insert(
            (string_class, Selector::new().push("with")),
            Method {
                param_types: vec![Type::Class(string_class)],
                return_type: Type::Class(string_class),
            },
        );

        string_class
    };
    {
        let io_class = context.new_class();
        context
            .classes
            .insert(String::from("IO"), Class::non_defined(io_class));

        context.class_methods.insert(
            (io_class, Selector::new().push("print-line")),
            Method {
                param_types: vec![Type::Class(string_class)],
                return_type: Type::Void,
            },
        );
    }

    let mut top_levels = Vec::new();

    let mut class_defs = Vec::new();
    let mut method_decls = Vec::new();
    let mut method_defs = Vec::new();

    {
        use tree::ast::TopLevel::*;
        for top_level in decls.into_iter() {
            match top_level {
                ClassDefinition(class_definition) => class_defs.push(class_definition),
                MethodDeclaration(method_declaration) => method_decls.push(method_declaration),
                MethodDefinition(method_definition) => method_defs.push(method_definition),
            }
        }
    }

    for class_definition in class_defs.iter() {
        let ((), new_context) = class_definition.clone().declare(context)?;
        context = new_context
    }

    for class_definition in class_defs.into_iter() {
        let c = class_definition.define(&mut context)?;
        top_levels.push(tree::typed::TopLevel::ClassDefinition(c));
    }

    for method_declaration in method_decls.into_iter() {
        let ((), new_context) = method_declaration.declare(context)?;
        context = new_context;
    }

    for method_definition in method_defs.into_iter() {
        let md = method_definition.define(&mut context)?;
        top_levels.push(tree::typed::TopLevel::MethodDefinition(md));
    }

    Ok((tree::typed::Program(top_levels), context))
}

#[cfg(test)]
mod test {
    use crate::{lexer::Lexer, parser::Parser, sema::analyze_program};

    #[test]
    fn test_analyze_program() {
        let source = include_str!("../examples/simple.moo");
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program().unwrap();

        match analyze_program(program) {
            Ok((_, context)) => println!("{context:?}"),
            Err(e) => eprintln!("{e:?}"),
        }
    }
}
