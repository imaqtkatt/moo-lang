use crate::{shared::Selector, tree::ir};

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Int(i32),
    Bool(bool),
    Str(String),
    Instance(Instance),
}

#[derive(Debug)]
pub struct Instance {
    class: ir::ClassId,
    fields: std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
}

impl Clone for Instance {
    fn clone(&self) -> Self {
        Self {
            class: self.class,
            fields: std::rc::Rc::clone(&self.fields),
        }
    }
}

impl Instance {
    fn instantiate(class_id: ir::ClassId, fields: Vec<Value>) -> Self {
        Self {
            class: class_id,
            fields: std::rc::Rc::new(std::cell::RefCell::new(fields)),
        }
    }
}

pub struct Env {
    self_ref: Value,
    locals: Vec<Value>,

    //
    classes: Vec<ir::Class>,
    methods: Vec<ir::Method>,
    fields: Vec<ir::Field>,
}

impl Env {
    fn get_local(&self, ir::Local(idx): &ir::Local) -> Value {
        self.locals[*idx].clone()
    }

    fn put_local(&mut self, ir::Local(idx): &ir::Local, value: Value) {
        self.locals[*idx] = value;
    }

    fn lookup_method(&self, ir::MethodId(idx): &ir::MethodId) -> ir::Method {
        self.methods[*idx].clone()
    }

    fn lookup_class(&self, ir::ClassId(idx): &ir::ClassId) -> &ir::Class {
        &self.classes[*idx]
    }
}

pub fn eval_ir_expr(env: &mut Env, e: &ir::Expr) -> Value {
    match e {
        ir::Expr::Variable(local) => env.get_local(local),
        ir::Expr::Constant(constant) => match constant {
            ir::Constant::Null => Value::Null,
            ir::Constant::Int(i) => Value::Int(*i),
            ir::Constant::Bool(b) => Value::Bool(*b),
            ir::Constant::Str(s) => Value::Str(String::from(s)),
        },
        ir::Expr::SelfRef => env.self_ref.clone(),
        ir::Expr::Let(local, value, next) => {
            let evaled_value = eval_ir_expr(env, value);

            env.put_local(local, evaled_value);

            eval_ir_expr(env, next)
        }
        ir::Expr::If(condition, consequence, alternative) => match eval_ir_expr(env, condition) {
            Value::Bool(true) => eval_ir_expr(env, consequence),
            Value::Bool(false) => eval_ir_expr(env, alternative),
            _ => unreachable!(),
        },
        ir::Expr::Seq(a, b) => {
            eval_ir_expr(env, a);
            eval_ir_expr(env, b)
        }
        ir::Expr::FieldGet(_instance, _field_id, offset) => {
            let Value::Instance(instance) = &env.self_ref else {
                unreachable!()
            };
            let fields = instance.fields.borrow();
            fields[*offset].clone()
        }
        ir::Expr::FieldSet(_receiver, _field_id, offset, value) => {
            let Value::Instance(instance) = env.self_ref.clone() else {
                unreachable!()
            };
            let mut fields = instance.fields.borrow_mut();
            fields[*offset] = eval_ir_expr(env, value);
            Value::Null
        }
        ir::Expr::InstanceCall(receiver, method_id, arguments) => {
            let Value::Instance(receiver) = eval_ir_expr(env, receiver) else {
                unreachable!()
            };

            let method = env.lookup_method(method_id);
            let arguments = eval_ir_many(env, arguments);
            method_call(env, method, Value::Instance(receiver), arguments)
        }
        ir::Expr::ClassCall(_class_id, method_id, arguments) => {
            let method = env.lookup_method(method_id);
            let arguments = eval_ir_many(env, arguments);
            method_call(env, method, Value::Null, arguments)
        }
        ir::Expr::Instantiate(class_id, init) => {
            let class = env.lookup_class(class_id);
            let class_id = class.id;

            let init = eval_ir_many(env, init);

            Value::Instance(Instance::instantiate(class_id, init))
        }
        ir::Expr::NotNull(nullable) => {
            let evaled_nullable = eval_ir_expr(env, nullable);

            if let Value::Null = evaled_nullable {
                Value::Bool(false)
            } else {
                Value::Bool(true)
            }
        }
    }
}

fn eval_ir_many(env: &mut Env, exs: &Vec<ir::Expression>) -> Vec<Value> {
    exs.into_iter().map(|e| eval_ir_expr(env, e)).collect()
}

fn method_call(env: &mut Env, method: ir::Method, new_this: Value, arguments: Vec<Value>) -> Value {
    let mut new_locals = vec![Value::Null; method.locals];

    for (slot, argument) in arguments.into_iter().enumerate() {
        new_locals[slot] = argument;
    }

    let old_this = std::mem::replace(&mut env.self_ref, new_this);
    let old_locals = std::mem::replace(&mut env.locals, new_locals);

    let result = eval_ir_expr(env, &method.body);

    env.self_ref = old_this;
    env.locals = old_locals;

    result
}

pub fn eval_ir_program(tree: ir::Program) -> Value {
    let env = Env {
        self_ref: Value::Null,
        locals: vec![],
        classes: tree.classes,
        methods: tree.methods,
        fields: tree.fields,
    };

    // rust programming moment
    let main_class = *env
        .classes
        .iter()
        .find_map(|c| if c.name == "Main" { Some(c.id) } else { None })
        .iter()
        .next()
        .unwrap();
    let main_method = env
        .methods
        .iter()
        .find_map(|m| {
            if m.receiver == main_class && m.selector == Selector::unary("main") {
                Some(m)
            } else {
                None
            }
        })
        .cloned()
        .into_iter()
        .next()
        .unwrap();

    let mut env = env;

    method_call(&mut env, main_method, Value::Null, vec![])
}

#[cfg(test)]
mod test {
    use crate::interp_ir::eval_ir_program;

    #[test]
    fn test_interp_ir() {
        let source = include_str!("../examples/test-setter.moo");
        let lexer = crate::lexer::Lexer::new(source);
        let mut parser = crate::parser::Parser::new(lexer);
        let program = parser.parse_program().unwrap();
        let (analyzed, _) = crate::sema::analyze_program(program).unwrap();
        let lowered = crate::lowering::lower_program(analyzed);

        println!("{:?}", eval_ir_program(lowered));
    }
}
