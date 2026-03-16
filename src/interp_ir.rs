use crate::{shared::Selector, tree::ir};

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Int(i32),
    Bool(bool),
    Str(String),
    Instance(Instance),
}

// #[derive(Debug)]
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

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{:?}}}", self.fields.borrow())
    }
}

pub struct Env {
    frames: Vec<Frame>,

    //
    classes: Vec<ir::Class>,
    methods: Vec<ir::Method>,
    fields: Vec<ir::Field>,
}

struct Frame {
    curr_class: ir::ClassId,
    curr_method: ir::MethodId,

    self_ref: Value,
    locals: Vec<Value>,
}

impl Env {
    fn self_ref(&self) -> Value {
        let curr_frame = self.frames.len() - 1;
        self.frames[curr_frame].self_ref.clone()
    }

    fn get_local(&self, ir::Local(idx): &ir::Local) -> Value {
        let curr_frame = self.frames.len() - 1;
        self.frames[curr_frame].locals[*idx].clone()
    }

    fn put_local(&mut self, ir::Local(idx): &ir::Local, value: Value) {
        let curr_frame = self.frames.len() - 1;
        self.frames[curr_frame].locals[*idx] = value;
    }

    fn lookup_method(&self, ir::MethodId(idx): &ir::MethodId) -> ir::Method {
        self.methods[*idx].clone()
    }

    fn lookup_class(&self, ir::ClassId(idx): &ir::ClassId) -> &ir::Class {
        &self.classes[*idx]
    }
}

fn eval_ir_constant(constant: &ir::Constant) -> Value {
    match constant {
        ir::Constant::Null => Value::Null,
        ir::Constant::Int(i) => Value::Int(*i),
        ir::Constant::Bool(b) => Value::Bool(*b),
        ir::Constant::Str(s) => Value::Str(String::from(s)),
    }
}

fn eval_ir_expr(env: &mut Env, e: &ir::Expr) -> Value {
    match e {
        ir::Expr::Variable(local) => env.get_local(local),
        ir::Expr::Constant(constant) => eval_ir_constant(constant),
        ir::Expr::SelfRef => env.self_ref(),
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
            let Value::Instance(instance) = env.self_ref() else {
                unreachable!()
            };
            let fields = instance.fields.borrow();
            fields[*offset].clone()
        }
        ir::Expr::FieldSet(_receiver, _field_id, offset, value) => {
            let Value::Instance(instance) = env.self_ref() else {
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

fn eval_ir_many(env: &mut Env, exs: &[ir::Expression]) -> Vec<Value> {
    exs.iter().map(|e| eval_ir_expr(env, e)).collect()
}

fn method_call(env: &mut Env, method: ir::Method, new_this: Value, arguments: Vec<Value>) -> Value {
    let mut new_locals = vec![Value::Null; method.locals];

    for (slot, argument) in arguments.into_iter().enumerate() {
        new_locals[slot] = argument;
    }

    env.frames.push(Frame {
        curr_class: method.receiver,
        curr_method: method.id,
        self_ref: new_this,
        locals: new_locals,
    });

    let result = eval_ir_expr(env, &method.body);

    _ = env.frames.pop().unwrap();

    result
}

pub fn eval_ir_program(tree: ir::Program) -> Value {
    let env = Env {
        frames: vec![],
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
        .find(|m| m.receiver == main_class && m.selector == Selector::unary("main"))
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
        let source = include_str!("../examples/linked-list.moo");
        let lexer = crate::lexer::Lexer::new(source);
        let mut parser = crate::parser::Parser::new(lexer);
        let program = parser.parse_program().unwrap();
        let (analyzed, _) = crate::sema::analyze_program(program).unwrap();
        let lowered = crate::lowering::lower_program(analyzed);

        println!("{:?}", eval_ir_program(lowered));
    }
}
