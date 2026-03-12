use crate::interp::{self, Env, Value};

pub fn string_with(env: &mut Env) -> Value {
    let string_class = env.classes["String"].clone();

    let Value::Instance(instance) = &env.self_this else {
        unreachable!()
    };
    let Value::Str(inner) = instance.fields.borrow()["inner"].clone() else {
        unreachable!()
    };

    let Value::Instance(other) = env.variables["other"].clone() else {
        unreachable!()
    };
    let Value::Str(other) = other.fields.borrow()["inner"].clone() else {
        unreachable!()
    };

    let mut new_inner = String::new();
    new_inner.push_str(&inner);
    new_inner.push_str(&other);

    let new_string = Value::Str(new_inner);
    string_class.instantiate(vec![(String::from("inner"), new_string)])
}

thread_local! {
    pub static BUILTINS: [(String, interp::Builtin); 1] = [
        (String::from("string_with"), string_with),
    ];
}
