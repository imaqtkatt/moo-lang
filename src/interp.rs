// since we don't have a backend lets just use a simple interpreter

use std::{collections::BTreeMap, rc::Rc};

use crate::{shared::Selector, tree};

pub struct Env {
    pub self_this: Value,
    pub variables: BTreeMap<String, Value>,
    pub classes: BTreeMap<String, Class>,
}

impl Clone for Env {
    fn clone(&self) -> Self {
        Self {
            self_this: self.self_this.clone(),
            variables: self.variables.clone(),
            classes: self.classes.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Int(i32),
    Bool(bool),
    Str(String),
    Instance(Instance),
    Class(Class),
}

// #[derive(Debug)]
pub struct Instance {
    pub class: Rc<Class>,
    pub fields: Rc<std::cell::RefCell<BTreeMap<String, Value>>>,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{:?}}}", self.fields)
    }
}

impl Clone for Instance {
    fn clone(&self) -> Self {
        Self {
            class: Rc::clone(&self.class),
            fields: Rc::clone(&self.fields),
        }
    }
}

#[derive(Debug)]
pub struct Class {
    class_type: crate::sema::ClassType,
    instance_methods: Rc<BTreeMap<Selector, Method>>,
    class_methods: Rc<BTreeMap<Selector, Method>>,
}

impl Class {
    pub fn instantiate(self, fields: Vec<(String, Value)>) -> Value {
        Value::Instance(Instance {
            class: Rc::new(self),
            fields: Rc::new(std::cell::RefCell::new(fields.into_iter().collect())),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Method {
    parameters: Vec<String>,
    body: MethodBody,
}

pub type Builtin = fn(&mut Env) -> Value;

#[derive(Clone)]
pub enum MethodBody {
    Tree(tree::typed::Expression),
    Builtin(Builtin),
}

impl std::fmt::Debug for MethodBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tree(tree) => write!(f, "Tree({tree:?})"),
            Self::Builtin(_) => write!(f, "Rust(..)"),
        }
    }
}

impl Clone for Class {
    fn clone(&self) -> Self {
        Self {
            class_type: self.class_type,
            instance_methods: Rc::clone(&self.instance_methods),
            class_methods: Rc::clone(&self.class_methods),
        }
    }
}

trait Eval {
    type Output;

    fn eval(self, env: &mut Env) -> Self::Output;
}

impl Eval for tree::typed::Constant {
    type Output = Value;

    fn eval(self, _: &mut Env) -> Self::Output {
        match self {
            tree::typed::Constant::Null => Value::Null,
            tree::typed::Constant::Integer(i) => Value::Int(i),
            tree::typed::Constant::Boolean(b) => Value::Bool(b),
            tree::typed::Constant::String(s) => Value::Str(s),
        }
    }
}

impl Eval for tree::typed::Expression {
    type Output = Value;

    fn eval(self, env: &mut Env) -> Self::Output {
        match self {
            tree::typed::Expression::Variable(name) => env
                .variables
                .get(&name)
                .unwrap_or_else(|| panic!("{name} exists"))
                .clone(),
            tree::typed::Expression::Constant(constant) => constant.eval(env),
            tree::typed::Expression::ESelf => env.self_this.clone(),
            tree::typed::Expression::LetIn(bind, value, next) => {
                let evaled_value = value.eval(env);

                env.variables.insert(bind, evaled_value);

                next.eval(env)
            }
            tree::typed::Expression::IfThenElse(condition, consequence, alternative) => {
                let evaled_condition = condition.eval(env);

                if let Value::Bool(true) = evaled_condition {
                    consequence.eval(env)
                } else {
                    alternative.eval(env)
                }
            }
            tree::typed::Expression::Seq(a, b) => {
                a.eval(env);
                b.eval(env)
            }
            tree::typed::Expression::Cascade(receiver, messages) => {
                let Value::Instance(instance) = receiver.eval(env) else {
                    unreachable!()
                };
                let mut result = Value::Null;

                for (selector, arguments) in messages {
                    let method = instance.class.instance_methods[&selector].clone();

                    result = method_call(method, env, arguments, Value::Instance(instance.clone()));
                }

                result
            }
            tree::typed::Expression::Load(field) => {
                if let Value::Instance(instance) = &env.self_this {
                    instance.fields.borrow()[&field].clone()
                } else {
                    unreachable!()
                }
            }
            tree::typed::Expression::Store(field, value) => {
                let evaled_value = value.eval(env);

                let Value::Instance(instance) = &mut env.self_this else {
                    unreachable!()
                };

                instance.fields.borrow_mut().insert(field, evaled_value);

                Value::Null
            }
            tree::typed::Expression::InstanceCall(receiver, selector, arguments) => {
                // what if 'self' is allowed inside a static context?

                let Value::Instance(instance) = receiver.eval(env) else {
                    unreachable!()
                };

                let method = instance.class.instance_methods[&selector].clone();

                method_call(method, env, arguments, Value::Instance(instance))
            }
            tree::typed::Expression::ClassCall(class, selector, arguments) => {
                let class = env.classes[&class].clone();
                let method = class.class_methods[&selector].clone();

                method_call(method, env, arguments, Value::Class(class))
            }
            tree::typed::Expression::Instantiate(class_name, field_init) => {
                let class = env.classes[&class_name].clone();

                let fields = field_init
                    .into_iter()
                    .map(|(field, value)| (field, value.eval(env)))
                    .collect();

                class.instantiate(fields)
            }
        }
    }
}

fn method_call(
    method: Method,
    env: &mut Env,
    arguments: Vec<impl Eval<Output = Value>>,
    new_this: Value,
) -> Value {
    let evaled_arguments = arguments
        .into_iter()
        .map(|a| a.eval(env))
        .collect::<Vec<_>>();

    let old_this = std::mem::replace(&mut env.self_this, new_this);
    let old_variables = std::mem::take(&mut env.variables);

    let return_value = {
        for (param, argument) in method.parameters.iter().zip(evaled_arguments) {
            env.variables.insert(param.clone(), argument);
        }

        match method.body {
            MethodBody::Tree(expression) => expression.eval(env),
            MethodBody::Builtin(f) => f(env),
        }
    };

    env.self_this = old_this;
    env.variables = old_variables;

    return_value
}

impl Eval for tree::typed::Typed<tree::typed::Expression> {
    type Output = Value;

    fn eval(self, env: &mut Env) -> Self::Output {
        self.value.eval(env)
    }
}

trait InterpDefine {
    fn define(self, env: &mut Env);
}

impl InterpDefine for tree::typed::ClassDefinition {
    fn define(self, env: &mut Env) {
        env.classes.insert(
            self.class_name,
            Class {
                class_type: self.class_type,
                instance_methods: Rc::new(BTreeMap::new()),
                class_methods: Rc::new(BTreeMap::new()),
            },
        );
    }
}

impl InterpDefine for tree::typed::MethodDefinition {
    fn define(self, env: &mut Env) {
        let receiver = env.classes.get_mut(&self.receiver).unwrap();

        let mut slots = match self.method_type {
            tree::typed::MethodType::Class => receiver.class_methods.as_ref().clone(),
            tree::typed::MethodType::Instance => receiver.instance_methods.as_ref().clone(),
        };

        let method = Method {
            parameters: self.parameters.clone(),
            body: MethodBody::Tree(*self.body.value),
        };
        slots.insert(self.selector, method);

        match self.method_type {
            tree::typed::MethodType::Class => receiver.class_methods = Rc::new(slots),
            tree::typed::MethodType::Instance => receiver.instance_methods = Rc::new(slots),
        }
    }
}

fn eval_env(mut env: Env) -> Value {
    let main_class = env.classes.get("Main").cloned().expect("has main");

    env.self_this = Value::Class(main_class.clone());

    let main_method = main_class.class_methods[&Selector::unary("main")].clone();

    match main_method.body {
        MethodBody::Tree(expression) => expression.eval(&mut env),
        MethodBody::Builtin(_) => unreachable!(),
    }
}

pub fn eval_program(tree: tree::typed::Program) -> Value {
    let top_levels = tree.0;

    let mut class_defs = Vec::new();
    let mut method_defs = Vec::new();

    for top_level in top_levels {
        match top_level {
            tree::typed::TopLevel::ClassDefinition(class_definition) => {
                class_defs.push(class_definition)
            }
            tree::typed::TopLevel::MethodDefinition(method_definition) => {
                method_defs.push(method_definition)
            }
        }
    }

    let mut env = Env {
        self_this: Value::Null,
        variables: BTreeMap::new(),
        classes: BTreeMap::new(),
    };

    for class in class_defs {
        class.define(&mut env);
    }
    for method in method_defs {
        method.define(&mut env);
    }

    eval_env(env)
}
