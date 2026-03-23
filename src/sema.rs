use std::collections::BTreeMap;

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

    pub class_instantiations: BTreeMap<(ClassType, Vec<TypeId>), ClassInstance>,

    // pub method_instantiations: BTreeMap<(ClassType, Selector)>,
    locals: BTreeMap<String, (Bind, TypeId)>,

    type_variables: BTreeMap<String, TypeId>,

    pub type_context: TypeContext,
    // substitutions: BTreeMap<TypeVar, Type>,
}

#[derive(Clone, Debug)]
pub enum AnalysisError {
    UnboundVariable(String),
    UnboundInstanceMethod(Selector),
    UnboundClassMethod(Selector),
    UnboundClass(String),
    MissingDeclaration(Selector),
    ExpectedClass,
    ExpectedNullable,
    ExpectedVariable,
    ExpectedField,
    ConstructorInitError,
    FieldInitError,
    TypeError { expected: Type, got: Type },
}

#[derive(Clone, Debug)]
pub struct Method {
    param_types: Vec<TypeId>,
    return_type: TypeId,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub class_type: ClassType,
    pub generics: Vec<String>,
    type_vars: Vec<TypeId>,
    pub fields: Vec<(String, TypeId)>,
}

impl Class {
    fn non_defined(class_type: ClassType, generics: &[String]) -> Self {
        Self {
            class_type,
            generics: Vec::from(generics),
            type_vars: Vec::new(),
            fields: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClassInstance {
    pub class_type: ClassType,
    pub type_id: TypeId,
    pub generics: Vec<TypeId>,
    pub fields: Vec<(String, TypeId)>,
}

#[derive(Clone, Copy, Debug)]
enum Bind {
    Field,
    Local,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeId(usize);

#[derive(Debug, Default)]
pub struct TypeContext {
    next_id: usize,
    types: Vec<Type>,
}

const NULL_TYPE: TypeId = TypeId(0);
const VOID_TYPE: TypeId = TypeId(1);
const INT_TYPE: TypeId = TypeId(2);
const BOOL_TYPE: TypeId = TypeId(3);
const STR_TYPE: TypeId = TypeId(4);

impl TypeContext {
    pub fn new() -> Self {
        let mut context = Self::default();
        context.put(Type::Null);
        context.put(Type::Void);
        context.put(Type::Int);
        context.put(Type::Bool);
        context.put(Type::Str);
        context
    }

    pub fn put(&mut self, t: Type) -> TypeId {
        let id = self.next_id;
        let type_id = TypeId(id);
        self.next_id += 1;

        self.types.push(t);

        type_id
    }

    pub fn get(&self, TypeId(id): TypeId) -> &Type {
        &self.types[id]
    }
}

// TODO: implement pretty print for Type
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Void,
    Int,
    Bool,
    Str,
    Class(ClassType, Vec<TypeId>),
    Nullable(TypeId),
    TypeVar(TypeVar),

    Null,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeVar(usize);

impl Type {
    fn subtype(type_context: &TypeContext, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Void, Type::Void) => true,
            (Type::Int, Type::Int) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Str, Type::Str) => true,

            (Type::Class(a, b), Type::Class(c, d)) if a == c => {
                b.iter().zip(d.iter()).all(|(x, y)| {
                    let x = type_context.get(*x);
                    let y = type_context.get(*y);
                    Type::subtype(type_context, x, y)
                })
            }

            (Type::Nullable(a), Type::Nullable(b)) => {
                let a = type_context.get(*a);
                let b = type_context.get(*b);
                Type::subtype(type_context, a, b)
            }

            (Type::Null, Type::Nullable(_)) => true,
            (a, Type::Nullable(b)) => {
                let b = type_context.get(*b);
                Type::subtype(type_context, a, b)
            }

            (Type::TypeVar(a), Type::TypeVar(b)) => a == b,

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

impl Context {
    fn new_class(&mut self) -> ClassType {
        let current = self.next_class;
        self.next_class += 1;
        ClassType(current)
    }

    fn scope<T>(
        &mut self,
        name: &str,
        value: (Bind, TypeId),
        scope: impl FnOnce(&mut Context) -> T,
    ) -> T {
        self.bind(name, value);
        let result = scope(self);
        _ = self.locals.remove(name);
        result
    }

    fn lookup_local(&self, name: &str) -> Result<(Bind, TypeId), AnalysisError> {
        self.locals
            .get(name)
            .cloned()
            .ok_or(AnalysisError::UnboundVariable(String::from(name)))
    }

    fn bind(&mut self, name: &str, value: (Bind, TypeId)) -> Option<(Bind, TypeId)> {
        self.locals.insert(String::from(name), value)
    }

    fn lookup_class(&self, class_name: &str) -> Result<Class, AnalysisError> {
        self.classes.get(class_name).cloned().ok_or(
            AnalysisError::UnboundClass(String::from(class_name)), // panic!("unbound {class_name}")
        )
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

    fn bind_type_variable(&mut self, name: &str, t: TypeId) {
        self.type_variables.insert(String::from(name), t);
    }

    fn lookup_type_variable(&self, name: &str) -> Result<TypeId, AnalysisError> {
        self.type_variables
            .get(name)
            .cloned()
            .ok_or(AnalysisError::UnboundVariable(String::from(name)))
    }

    fn instantiate_class(
        &mut self,
        class_name: &str,
        args: Vec<TypeId>,
    ) -> Result<ClassInstance, AnalysisError> {
        let class = self.lookup_class(class_name)?;

        assert_eq!(class.generics.len(), args.len());

        if let Some(class_instance) = self
            .class_instantiations
            .get(&(class.class_type, args.clone()))
        {
            // println!("{class_instance:?}...");
            return Ok(class_instance.clone());
        }

        // println!("instantiating class with args = {args:?}");

        let instantiated_fields = class
            .fields
            .iter()
            .map(|(name, t)| (name.clone(), self.instantiate(*t, &args)))
            .collect::<Vec<_>>();

        // println!("class fields = {:?}", class.fields);
        println!("instantiated_fields = {instantiated_fields:?}");

        let type_id = self
            .type_context
            .put(Type::Class(class.class_type, args.clone()));

        use std::collections::btree_map::Entry;

        match self
            .class_instantiations
            .entry((class.class_type, args.clone()))
        {
            Entry::Vacant(v) => Ok(v
                .insert(ClassInstance {
                    class_type: class.class_type,
                    type_id,
                    generics: args,
                    fields: instantiated_fields,
                })
                .clone()),
            Entry::Occupied(_) => unreachable!(),
        }
    }

    fn instantiate(&mut self, a: TypeId, args: &[TypeId]) -> TypeId {
        match self.type_context.get(a).clone() {
            Type::TypeVar(type_var) => args[type_var.0],
            Type::Class(class_type, generics) => {
                let new_generics = generics
                    .into_iter()
                    .map(|g| self.instantiate(g, args))
                    .collect();

                self.type_context.put(Type::Class(class_type, new_generics))
            }
            Type::Nullable(nullable) => {
                let new_nullable = self.instantiate(nullable, args);
                self.type_context.put(Type::Nullable(new_nullable))
            }
            Type::Void | Type::Int | Type::Bool | Type::Str | Type::Null => a,
        }
    }

    fn type_equality(&self, a: TypeId, b: TypeId) -> Result<(), AnalysisError> {
        if a == b {
            return Ok(());
        }

        let a = self.type_context.get(a);
        let b = self.type_context.get(b);

        if Type::subtype(&self.type_context, a, b) {
            Ok(())
        } else {
            Err(AnalysisError::TypeError {
                expected: b.clone(),
                got: a.clone(),
            })
        }
    }
}

// impl Type {
//     fn occurs(&self, type_var: TypeVar) -> bool {
//         match self {
//             Type::TypeVar(tv) => *tv == type_var,
//             Type::Void | Type::Int | Type::Bool | Type::Str | Type::Null => false,
//             Type::Class(_, items) => items.iter().any(|i| i.occurs(type_var)),
//             Type::Nullable(nullable) => nullable.occurs(type_var),
//         }
//     }
// }

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
                    tree::ast::Constant::Null => (tree::typed::Constant::Null, NULL_TYPE),
                    tree::ast::Constant::Integer(i) => {
                        (tree::typed::Constant::Integer(i), INT_TYPE)
                    }
                    tree::ast::Constant::Boolean(b) => {
                        (tree::typed::Constant::Boolean(b), BOOL_TYPE)
                    }
                    tree::ast::Constant::String(s) => (tree::typed::Constant::String(s), STR_TYPE),
                };

                let constant = tree::typed::Expression::Constant(constant);

                Ok(Self::Output {
                    value: Box::new(constant),
                    r#type,
                })
            }
            tree::ast::Expression::SelfRef => {
                let e_self = tree::typed::Expression::SelfRef;

                Ok(Self::Output {
                    value: Box::new(e_self),
                    r#type: ctx.locals["self"].1,
                })
            }
            tree::ast::Expression::LetIn(bind, value, next) => {
                let value = r_value(*value)?;
                let value = value.analyze(ctx)?;

                let next =
                    ctx.scope(&bind, (Bind::Local, value.r#type), |ctx| next.analyze(ctx))?;
                let next_t = next.r#type;

                let let_in = tree::typed::Expression::LetIn(bind, value, next);

                Ok(Self::Output {
                    value: Box::new(let_in),
                    r#type: next_t,
                })
            }
            tree::ast::Expression::IfThenElse(condition, consequence, alternative) => {
                let condition = condition.analyze(ctx)?;
                ctx.type_equality(condition.r#type, BOOL_TYPE)?;

                let consequence = consequence.analyze(ctx)?;
                let alternative = alternative.analyze(ctx)?;

                ctx.type_equality(consequence.r#type, alternative.r#type)?;

                let branch_type = consequence.r#type;
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
                    let inner = nullable_type(ctx.type_context.get(t))?;

                    if refined.is_some() {
                        todo!("alias on variable")
                    }

                    let consequence =
                        ctx.scope(&name, (bind, inner), |ctx| consequence.analyze(ctx))?;

                    let alternative = alternative.analyze(ctx)?;

                    ctx.type_equality(consequence.r#type, alternative.r#type)?;
                    let branch_type = consequence.r#type;

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
                    let nullable = r_value(*nullable)?;
                    let nullable = nullable.analyze(ctx)?;
                    let inner = nullable_type(ctx.type_context.get(nullable.r#type))?;

                    let consequence = if let Some(name) = &refined {
                        ctx.scope(name, (Bind::Local, inner), |ctx| consequence.analyze(ctx))?
                    } else {
                        consequence.analyze(ctx)?
                    };

                    let alternative = alternative.analyze(ctx)?;

                    ctx.type_equality(consequence.r#type, alternative.r#type)?;

                    let branch_type = consequence.r#type;
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
                ctx.type_equality(a.r#type, VOID_TYPE)?;

                let b = b.analyze(ctx)?;
                let seq_type = b.r#type;

                let seq = tree::typed::Expression::Seq(a, b);

                Ok(Self::Output {
                    value: Box::new(seq),
                    r#type: seq_type,
                })
            }
            tree::ast::Expression::Cascade(receiver, messages) => {
                assert!(!messages.is_empty());

                let receiver = receiver.analyze(ctx)?;
                let (class_type, types) = class_type(ctx.type_context.get(receiver.r#type))?;

                let mut new_messages = vec![];
                let mut return_type = VOID_TYPE;

                for (selector, arguments) in messages {
                    let selector = selector.selector();

                    let method = ctx.lookup_instance_method(class_type, selector.clone())?;

                    // TODO: create an instantiate_method function
                    let method_param_types = method
                        .param_types
                        .into_iter()
                        .map(|t| ctx.instantiate(t, &types))
                        .collect::<Vec<_>>();
                    // let method_return_type = method.return_type.instantiate(&types);
                    let method_return_type = ctx.instantiate(method.return_type, &types);

                    assert!(method_param_types.len() == arguments.len());

                    let arguments = arguments
                        .into_iter()
                        .map(|a| a.analyze(ctx))
                        .collect::<Result<Vec<_>, _>>()?;

                    for (a, b) in arguments.iter().zip(method_param_types.iter()) {
                        ctx.type_equality(a.r#type, *b)?;
                    }

                    new_messages.push((selector, arguments));
                    return_type = method_return_type;
                }

                let cascade = tree::typed::Expression::Cascade(receiver, new_messages);

                Ok(Self::Output {
                    value: Box::new(cascade),
                    r#type: return_type,
                })
            }
            tree::ast::Expression::Pipe(initial, calls) => {
                let initial = initial.analyze(ctx)?;

                let mut current_type = initial.r#type;

                let mut new_calls = vec![];

                for (selector, arguments) in calls {
                    let selector = selector.selector();

                    let (class_type, types) = class_type(ctx.type_context.get(current_type))?;

                    let method = ctx.lookup_instance_method(class_type, selector.clone())?;
                    // TODO: create an instantiate_method function
                    let method_param_types = method
                        .param_types
                        .into_iter()
                        .map(|t| ctx.instantiate(t, &types))
                        .collect::<Vec<_>>();
                    // let method_return_type = method.return_type.instantiate(&types);
                    let method_return_type = ctx.instantiate(method.return_type, &types);

                    assert!(method_param_types.len() == arguments.len());

                    let arguments = arguments
                        .into_iter()
                        .map(|a| a.analyze(ctx))
                        .collect::<Result<Vec<_>, _>>()?;

                    for (a, b) in arguments.iter().zip(method_param_types.iter()) {
                        ctx.type_equality(a.r#type, *b)?;
                    }

                    new_calls.push((selector, arguments));

                    current_type = method_return_type;
                }

                let pipe = tree::typed::Expression::Pipe(initial, new_calls);

                Ok(Self::Output {
                    value: Box::new(pipe),
                    r#type: current_type,
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
                ctx.type_equality(new_value.r#type, t)?;

                let field_set = tree::typed::Expression::Store(name, new_value);

                Ok(Self::Output {
                    value: Box::new(field_set),
                    r#type: VOID_TYPE,
                })
            }
            tree::ast::Expression::Call(receiver, selector, arguments) => {
                // if receiver is referencing a class name generate a ClassCall
                let selector = selector.selector();

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
                        ctx.type_equality(a.r#type, *b)?;
                    }

                    let class_call =
                        tree::typed::Expression::ClassCall(name.clone(), selector, arguments);

                    Ok(Self::Output {
                        value: Box::new(class_call),
                        r#type: method.return_type,
                    })
                } else {
                    let receiver = receiver.analyze(ctx)?;
                    let (class_type, types) = class_type(ctx.type_context.get(receiver.r#type))?;
                    println!("class_type = {class_type:?} with {types:?}");

                    let method = ctx.lookup_instance_method(class_type, selector.clone())?;

                    // TODO: create an instantiate_method function
                    let method_param_types = method
                        .param_types
                        .into_iter()
                        .map(|t| ctx.instantiate(t, &types))
                        .collect::<Vec<_>>();
                    // let method_return_type = method.return_type.instantiate(&types);
                    let method_return_type = ctx.instantiate(method.return_type, &types);

                    assert!(method_param_types.len() == arguments.len());

                    for (a, b) in arguments.iter().zip(method_param_types.iter()) {
                        ctx.type_equality(a.r#type, *b)?;
                    }

                    let instance_call =
                        tree::typed::Expression::InstanceCall(receiver, selector, arguments);

                    Ok(Self::Output {
                        value: Box::new(instance_call),
                        r#type: method_return_type,
                    })
                }
            }
            tree::ast::Expression::Instantiate(class_name, types, field_init) => {
                let analyzed_types = types
                    .into_iter()
                    .map(|t| t.analyze(ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                let class = ctx.instantiate_class(&class_name, analyzed_types.clone())?;

                // println!("expr instantiate = {class:?}");

                // let class = ctx.lookup_class(&class_name)?;

                let constructor = class.fields.clone();

                let field_init = field_init
                    .into_iter()
                    .map(|(name, value)| value.analyze(ctx).map(|v| (name, v)))
                    .collect::<Result<Vec<_>, _>>()?;

                if constructor.len() != field_init.len() {
                    println!(
                        "{:?} - {constructor:?} and {field_init:?}",
                        class.class_type
                    );
                    Err(AnalysisError::ConstructorInitError)?
                }

                for ((a, b), (c, d)) in constructor.iter().zip(field_init.iter()) {
                    if a != c {
                        Err(AnalysisError::FieldInitError)?
                    }
                    ctx.type_equality(d.r#type, *b)?;
                }

                let instantiate = tree::typed::Expression::Instantiate(class_name, field_init);

                let return_type = class.type_id;

                Ok(Self::Output {
                    value: Box::new(instantiate),
                    r#type: return_type,
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

fn r_value(tree: tree::ast::Expression) -> Result<tree::ast::Expression, AnalysisError> {
    match &tree {
        tree::ast::Expression::Assignment(..) => todo!("error"),
        tree::ast::Expression::Variable(..)
        | tree::ast::Expression::Constant(..)
        | tree::ast::Expression::SelfRef
        | tree::ast::Expression::LetIn(..)
        | tree::ast::Expression::IfThenElse(..)
        | tree::ast::Expression::IfLetThenElse(..)
        | tree::ast::Expression::Seq(..)
        | tree::ast::Expression::Cascade(..)
        | tree::ast::Expression::Pipe(..)
        | tree::ast::Expression::Call(..)
        | tree::ast::Expression::Instantiate(..)
        | tree::ast::Expression::Group(..) => Ok(tree),
    }
}

fn class_type(a: &Type) -> Result<(ClassType, Vec<TypeId>), AnalysisError> {
    if let Type::Class(class_type, types) = a {
        Ok((*class_type, types.clone()))
    } else {
        Err(AnalysisError::ExpectedClass)
    }
}

fn nullable_type(t: &Type) -> Result<TypeId, AnalysisError> {
    if let Type::Nullable(inner) = t {
        Ok(*inner)
    } else {
        Err(AnalysisError::ExpectedNullable)
    }
}

impl Analyze for tree::ast::Type {
    type Output = TypeId;

    fn analyze(self, ctx: &mut Context) -> Result<Self::Output, AnalysisError> {
        let analyzed = match self {
            tree::ast::Type::Void => VOID_TYPE,
            tree::ast::Type::Int => INT_TYPE,
            tree::ast::Type::Bool => BOOL_TYPE,
            tree::ast::Type::Str => STR_TYPE,
            tree::ast::Type::Named(name, args) => {
                // let mut scopes = vec![context.classes, context.type_variables];
                // println!("{:?}", context.type_variables);
                if let Ok(t) = ctx.lookup_type_variable(&name) {
                    assert!(args.is_empty());
                    return Ok(t);
                }

                let class = ctx.lookup_class(&name)?;
                assert!(class.generics.len() == args.len());

                // println!("args = {args:?}");

                let args = args
                    .into_iter()
                    .map(|a| a.analyze(ctx))
                    .collect::<Result<Vec<_>, _>>()?;

                // let instance = context.instantiate_class(&name, args.clone())?;

                // println!("instance = {instance:?}");

                ctx.type_context.put(Type::Class(class.class_type, args))
            }
            tree::ast::Type::Nullable(inner) => {
                let inner = inner.analyze(ctx)?;
                ctx.type_context.put(Type::Nullable(inner))
            }
        };
        Ok(analyzed)
    }
}

impl Declare for tree::ast::ClassDefinition {
    type Output = ();

    fn declare(&self, mut ctx: Context) -> Result<(Self::Output, Context), AnalysisError> {
        let this_class = ctx.new_class();

        ctx.classes.insert(
            self.class_name.clone(),
            Class::non_defined(this_class, &self.generics),
        );

        Ok(((), ctx))
    }
}

impl Define for tree::ast::ClassDefinition {
    type Output = tree::typed::ClassDefinition;

    fn define(self, ctx: &mut Context) -> Result<Self::Output, AnalysisError> {
        let mut this_class = ctx.lookup_class(&self.class_name)?;
        let class_type = this_class.class_type;

        {
            // println!("{:?}", this_class.generics);
            for (id, name) in this_class.generics.iter().enumerate() {
                let t = ctx.type_context.put(Type::TypeVar(TypeVar(id)));
                this_class.type_vars.push(t);
                ctx.bind_type_variable(name, t);
            }

            for (name, t) in self.fields.iter() {
                let t = t.clone().analyze(ctx)?;
                // println!("{name} = {t:?}");
                this_class.fields.push((name.clone(), t))
            }

            println!(
                "{} with id {:?} has fields {:?}",
                self.class_name, class_type, self.fields
            );

            ctx.type_variables.clear();
        }

        let fields = this_class.fields.clone();
        ctx.classes.insert(self.class_name.clone(), this_class);

        Ok(Self::Output {
            class_type,
            class_name: self.class_name,
            fields,
        })
    }
}

impl Declare for tree::ast::MethodDeclaration {
    type Output = ();

    fn declare(&self, mut ctx: Context) -> Result<(Self::Output, Context), AnalysisError> {
        let class = ctx.lookup_class(&self.receiver)?;

        let (param_types, return_type) = {
            // for (id, name) in class.generics.iter().enumerate() {
            //     // TODO: we are probably wasting type insertions
            //     let t = ctx.type_context.insert_type(Type::TypeVar(TypeVar(id)));
            //     ctx.bind_type_variable(name, t);
            // }
            for (name, tv) in class.generics.iter().zip(class.type_vars.iter()) {
                ctx.bind_type_variable(name, *tv);
            }

            let param_types = self
                .parameters
                .iter()
                .cloned()
                .map(|t| t.analyze(&mut ctx))
                .collect::<Result<Vec<_>, _>>()?;
            let return_type = self.body.clone().analyze(&mut ctx)?;

            ctx.type_variables.clear();

            (param_types, return_type)
        };

        let method = Method {
            param_types,
            return_type,
        };

        match self.method_type {
            tree::ast::MethodType::Class => {
                ctx.class_methods
                    .insert((class.class_type, self.selector.clone()), method);
            }
            tree::ast::MethodType::Instance => {
                ctx.instance_methods
                    .insert((class.class_type, self.selector.clone()), method);
            }
        };

        Ok(((), ctx))
    }
}

impl Define for tree::ast::MethodDefinition {
    type Output = tree::typed::MethodDefinition;

    fn define(self, ctx: &mut Context) -> Result<Self::Output, AnalysisError> {
        let class = ctx.lookup_class(&self.receiver)?;

        // let mut generics = Vec::new();
        // for (id, name) in class.generics.iter().enumerate() {
        //     // TODO: we are probably wasting type insertions
        //     let g = ctx.type_context.insert_type(Type::TypeVar(TypeVar(id)));
        //     ctx.bind_type_variable(name, g);
        //     generics.push(g);
        // }
        for (name, tv) in class.generics.iter().zip(class.type_vars.iter()) {
            ctx.bind_type_variable(name, *tv);
        }

        let method = match self.method_type {
            tree::ast::MethodType::Class => {
                ctx.lookup_class_method(class.class_type, self.selector.clone())
            }
            tree::ast::MethodType::Instance => {
                ctx.lookup_instance_method(class.class_type, self.selector.clone())
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
                ctx.locals.insert(
                    String::from("self"),
                    (
                        Bind::Local,
                        ctx.type_context
                            .put(Type::Class(class.class_type, class.type_vars)),
                    ),
                );

                for (a, b) in class.fields.iter().cloned() {
                    _ = ctx.bind(&a, (Bind::Field, b));
                }
            }
            for (a, b) in self.parameters.iter().zip(method.param_types.iter()) {
                _ = ctx.bind(a, (Bind::Local, *b));
            }

            let body = self.body.analyze(ctx)?;
            ctx.type_equality(body.r#type, method.return_type)?;
            ctx.locals.clear();
            body
        };

        ctx.type_variables.clear();

        Ok(Self::Output {
            method_type: self.method_type.into(),
            receiver: self.receiver.clone(),
            selector: self.selector.clone(),
            parameters: self.parameters.clone(),
            parameter_types: method.param_types.clone(),
            return_type: method.return_type,
            body,
        })
    }
}

// TODO: should assert that 'Main main' exists and has signature '[] => void'
pub fn analyze_program(
    tree: tree::ast::Program,
) -> Result<(tree::typed::Program, Context), AnalysisError> {
    let decls = tree.0;

    let mut ctx = Context {
        next_class: 0,
        classes: BTreeMap::new(),

        instance_methods: BTreeMap::new(),
        class_methods: BTreeMap::new(),
        class_instantiations: BTreeMap::new(),

        locals: BTreeMap::new(),

        type_variables: BTreeMap::new(),

        type_context: TypeContext::new(),
    };

    // let string_class = {
    //     let string_class = context.new_class();
    //     context.classes.insert(
    //         String::from("String"),
    //         Class::non_defined(string_class, &[]).with_field(String::from("inner"), STR_TYPE),
    //     );
    //     context.class_methods.insert(
    //         (string_class, Selector::new().push("str")),
    //         Method {
    //             param_types: vec![STR_TYPE],
    //             return_type: Type::Class(string_class, vec![]),
    //         },
    //     );
    //     context.instance_methods.insert(
    //         (string_class, Selector::new().push("with")),
    //         Method {
    //             param_types: vec![Type::Class(string_class, vec![])],
    //             return_type: Type::Class(string_class, vec![]),
    //         },
    //     );

    //     string_class
    // };
    // {
    //     let io_class = context.new_class();
    //     context
    //         .classes
    //         .insert(String::from("IO"), Class::non_defined(io_class, &[]));

    //     context.class_methods.insert(
    //         (io_class, Selector::new().push("print-line")),
    //         Method {
    //             param_types: vec![Type::Class(string_class, vec![])],
    //             return_type: Type::Void,
    //         },
    //     );
    // }

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
        let ((), new_context) = class_definition.clone().declare(ctx)?;
        ctx = new_context
    }

    for class_definition in class_defs.into_iter() {
        let c = class_definition.define(&mut ctx)?;
        top_levels.push(tree::typed::TopLevel::ClassDefinition(c));
    }

    for method_declaration in method_decls.into_iter() {
        let ((), new_context) = method_declaration.declare(ctx)?;
        ctx = new_context;
    }

    for method_definition in method_defs.into_iter() {
        let md = method_definition.define(&mut ctx)?;
        top_levels.push(tree::typed::TopLevel::MethodDefinition(md));
    }

    Ok((tree::typed::Program(top_levels), ctx))
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
            Ok((_, ctx)) => println!("{ctx:?}"),
            Err(e) => eprintln!("{e:?}"),
        }
    }
}
