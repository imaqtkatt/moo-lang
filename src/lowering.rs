use std::collections::BTreeMap;

use crate::{
    shared::Selector,
    tree::{ir, typed},
};

#[derive(Debug)]
pub struct Context {
    // mutable fields
    locals: BTreeMap<String, ir::Local>,
    current_class: Option<ir::ClassId>,

    //
    // "persistent" fields
    //
    class_type_to_id: BTreeMap<crate::sema::ClassType, ir::ClassId>,
    class_id: BTreeMap<String, ir::ClassId>,
    classes: BTreeMap<ir::ClassId, ir::Class>,

    method_id: BTreeMap<(ir::ClassId, Selector), ir::MethodId>,
    methods: BTreeMap<ir::MethodId, ir::Method>,

    field_id: BTreeMap<(ir::ClassId, String), ir::FieldId>,
    fields: BTreeMap<(ir::ClassId, ir::FieldId), ir::Field>,
}

impl Context {
    fn add_local(&mut self, name: &str) -> ir::Local {
        let next_id = self.locals.len();
        let local = ir::Local::new(next_id);

        self.locals.insert(String::from(name), local);

        local
    }

    fn lookup_local(&self, name: &str) -> ir::Local {
        self.locals[name]
    }

    fn next_class_id(&mut self, class_name: &str) -> ir::ClassId {
        let next_id = self.class_id.len();
        let class_id = ir::ClassId::new(next_id);
        if self.class_id.insert(class_name.into(), class_id).is_some() {
            panic!("redefinition of {class_name}");
        };
        class_id
    }

    fn next_method_id(&mut self, selector: (ir::ClassId, Selector)) -> ir::MethodId {
        let next_id = self.method_id.len();
        let method_id = ir::MethodId::new(next_id);
        if self.method_id.insert(selector, method_id).is_some() {
            panic!("redefinition of method ..");
        }
        method_id
    }

    fn register_class(&mut self, class: &typed::ClassDefinition) {
        self.next_class_id(&class.class_name);
    }

    fn add_class(&mut self, class: typed::ClassDefinition) -> ir::ClassId {
        let class_id = self.class_id[&class.class_name];

        self.class_type_to_id.insert(class.class_type, class_id);

        let mut fields = vec![];
        for (id, (name, ty)) in class.fields.into_iter().enumerate() {
            let field_id = ir::FieldId::new(id);

            let field = ir::Field {
                id: field_id,
                name: name.clone(),
                ty,
            };
            fields.push(field.clone());

            self.field_id.insert((class_id, name), field_id);
            self.fields.insert((class_id, field_id), field);
        }

        let ir_class = ir::Class {
            id: class_id,
            name: class.class_name,
            fields,
            methods: vec![],
        };

        self.classes.insert(class_id, ir_class);

        class_id
    }

    fn register_method(&mut self, method: &typed::MethodDefinition) {
        let class_id = self.class_id[&method.receiver];
        self.next_method_id((class_id, method.selector.clone()));
    }

    fn add_method(&mut self, method: typed::MethodDefinition) -> ir::MethodId {
        let class_id = self.class_id[&method.receiver];
        let method_id = self.method_id[&(class_id, method.selector.clone())];

        let (lowered_body, locals) = {
            for param in method.parameters.iter() {
                self.add_local(param);
            }

            // TODO: self in static context?
            self.current_class = match method.method_type {
                typed::MethodType::Class => None,
                typed::MethodType::Instance => Some(self.class_id[&method.receiver]),
            };

            let result = lower_typed_expr(method.body, self);
            let locals = self.locals.len();

            self.current_class = None;
            self.locals.clear();

            (result, locals)
        };

        let ir_method = ir::Method {
            id: method_id,
            method_type: match method.method_type {
                typed::MethodType::Class => ir::MethodType::Class,
                typed::MethodType::Instance => ir::MethodType::Instance,
            },
            receiver: self.class_id[&method.receiver],
            selector: method.selector,
            parameters: method.parameter_types,
            return_type: method.return_type,
            locals,
            body: lowered_body,
        };

        self.methods.insert(method_id, ir_method);

        method_id
    }
}

fn lower_constant(constant: typed::Constant) -> ir::Constant {
    match constant {
        typed::Constant::Null => ir::Constant::Null,
        typed::Constant::Integer(i) => ir::Constant::Int(i),
        typed::Constant::Boolean(b) => ir::Constant::Bool(b),
        typed::Constant::String(s) => ir::Constant::Str(s),
    }
}

fn lower_typed_expr(e: typed::Typed<typed::Expression>, ctx: &mut Context) -> ir::Expression {
    ir::Expression::new(lower_expr(*e.value, ctx))
}

fn lower_many(xs: Vec<typed::Typed<typed::Expression>>, ctx: &mut Context) -> Vec<ir::Expression> {
    xs.into_iter().map(|a| lower_typed_expr(a, ctx)).collect()
}

fn lower_expr(e: typed::Expression, ctx: &mut Context) -> ir::Expr {
    match e {
        typed::Expression::Variable(name) => lower_variable(ctx, name),
        typed::Expression::Constant(constant) => ir::Expr::Constant(lower_constant(constant)),
        typed::Expression::SelfRef => ir::Expr::SelfRef,
        typed::Expression::LetIn(bind, value, next) => lower_let_in(ctx, bind, value, next),
        typed::Expression::IfThenElse(condition, consequence, alternative) => {
            lower_if_then_else(ctx, condition, consequence, alternative)
        }
        typed::Expression::IfLetThenElse(nullable, refined, consequence, alternative) => {
            lower_if_let_then_else(ctx, nullable, refined, consequence, alternative)
        }
        typed::Expression::Seq(a, b) => lower_seq(ctx, a, b),
        typed::Expression::Cascade(receiver, messages) => lower_cascade(ctx, receiver, messages),
        typed::Expression::Load(name) => lower_load(ctx, name),
        typed::Expression::Store(name, value) => lower_store(ctx, name, value),
        typed::Expression::InstanceCall(receiver, selector, arguments) => {
            lower_instance_call(ctx, receiver, selector, arguments)
        }
        typed::Expression::ClassCall(class_name, selector, arguments) => {
            lower_class_call(ctx, class_name, selector, arguments)
        }
        typed::Expression::Instantiate(class_name, field_init) => {
            lower_instantiate(ctx, class_name, field_init)
        }
    }
}

fn lower_variable(ctx: &mut Context, name: String) -> ir::Expr {
    ir::Expr::Variable(ctx.lookup_local(&name))
}

fn lower_let_in(
    ctx: &mut Context,
    bind: String,
    value: typed::Typed<typed::Expression>,
    next: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let lowered_value = lower_typed_expr(value, ctx);
    let local = ctx.add_local(&bind);

    let lowered_next = lower_typed_expr(next, ctx);

    ir::Expr::Let(local, lowered_value.into(), lowered_next.into())
}

fn lower_if_then_else(
    ctx: &mut Context,
    condition: typed::Typed<typed::Expression>,
    consequence: typed::Typed<typed::Expression>,
    alternative: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let lowered_condition = lower_typed_expr(condition, ctx);
    let lowered_consequence = lower_typed_expr(consequence, ctx);
    let lowered_alternative = lower_typed_expr(alternative, ctx);

    ir::Expr::If(
        lowered_condition.into(),
        lowered_consequence.into(),
        lowered_alternative.into(),
    )
}

fn lower_if_let_then_else(
    ctx: &mut Context,
    nullable: typed::Typed<typed::Expression>,
    refined: Option<String>,
    consequence: typed::Typed<typed::Expression>,
    alternative: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let lowered_nullable = lower_typed_expr(nullable, ctx);
    let lowered_consequence = lower_typed_expr(consequence, ctx);
    let lowered_alternative = lower_typed_expr(alternative, ctx);

    let check_is_null = ir::Expr::IsNull(lowered_nullable.clone().into());

    if let Some(refined) = refined {
        let local = ctx.add_local(&refined);
        let lowered_consequence =
            ir::Expr::Let(local, lowered_nullable.into(), lowered_consequence.into());

        ir::Expr::If(
            check_is_null.into(),
            lowered_consequence.into(),
            lowered_alternative.into(),
        )
    } else {
        ir::Expr::If(
            check_is_null.into(),
            lowered_consequence.into(),
            lowered_alternative.into(),
        )
    }
}

fn lower_seq(
    ctx: &mut Context,
    a: typed::Typed<typed::Expression>,
    b: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let a = lower_typed_expr(a, ctx);
    let b = lower_typed_expr(b, ctx);

    ir::Expr::Seq(a.into(), b.into())
}

fn lower_cascade(
    ctx: &mut Context,
    receiver: typed::Typed<typed::Expression>,
    messages: Vec<(Selector, Vec<typed::Typed<typed::Expression>>)>,
) -> ir::Expr {
    assert!(messages.len() >= 1);

    let crate::sema::Type::Class(class_type) = receiver.r#type else {
        unreachable!()
    };
    let class_id = ctx.class_type_to_id[&class_type];
    let lowered_receiver = lower_typed_expr(receiver, ctx);

    let mut lowered_calls = Vec::new();

    for (selector, arguments) in messages {
        let method_id = ctx.method_id[&(class_id, selector)];
        let lowered_arguments = lower_many(arguments, ctx);
        let lowered_call =
            ir::Expr::InstanceCall(lowered_receiver.clone(), method_id, lowered_arguments);
        lowered_calls.push(lowered_call);
    }

    let last = lowered_calls.pop().unwrap();

    lowered_calls.into_iter().rfold(last, |acc, next| {
        ir::Expr::Seq(ir::Expression::new(next), ir::Expression::new(acc))
    })
}

fn lower_load(ctx: &mut Context, name: String) -> ir::Expr {
    let class = ctx.current_class.unwrap();
    let field_id = ctx.field_id[&(class, name)];

    ir::Expr::FieldGet(field_id)
}

fn lower_store(
    ctx: &mut Context,
    name: String,
    value: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let class = ctx.current_class.unwrap();
    let field_id = ctx.field_id[&(class, name)];

    let lowered_value = lower_typed_expr(value, ctx);

    ir::Expr::FieldSet(field_id, lowered_value.into())
}

fn lower_instance_call(
    ctx: &mut Context,
    receiver: typed::Typed<typed::Expression>,
    selector: Selector,
    arguments: Vec<typed::Typed<typed::Expression>>,
) -> ir::Expr {
    let crate::sema::Type::Class(class_type) = receiver.r#type else {
        unreachable!()
    };
    let class_id = ctx.class_type_to_id[&class_type];

    let lowered_receiver = lower_typed_expr(receiver, ctx);

    let method_id = ctx.method_id[&(class_id, selector)];

    let lowered_arguments = lower_many(arguments, ctx);

    ir::Expr::InstanceCall(lowered_receiver.into(), method_id, lowered_arguments)
}

fn lower_class_call(
    ctx: &mut Context,
    class_name: String,
    selector: Selector,
    arguments: Vec<typed::Typed<typed::Expression>>,
) -> ir::Expr {
    let class_id = ctx.class_id[&class_name];
    let method_id = ctx.method_id[&(class_id, selector)];

    let lowered_arguments = lower_many(arguments, ctx);

    ir::Expr::ClassCall(class_id, method_id, lowered_arguments)
}

fn lower_instantiate(
    ctx: &mut Context,
    class_name: String,
    field_init: Vec<(String, typed::Typed<typed::Expression>)>,
) -> ir::Expr {
    let class_id = ctx.class_id[&class_name];

    let fields = field_init
        .into_iter()
        .map(|(_, value)| lower_typed_expr(value, ctx))
        .collect();

    ir::Expr::Instantiate(class_id, fields)
}

pub fn lower_program(typed::Program(tree): typed::Program) -> ir::Program {
    let mut classes = vec![];
    let mut methods = vec![];

    for item in tree {
        match item {
            typed::TopLevel::ClassDefinition(class) => classes.push(class),
            typed::TopLevel::MethodDefinition(method) => methods.push(method),
        }
    }

    let mut ctx = Context {
        locals: Default::default(),
        current_class: None,
        class_type_to_id: Default::default(),
        class_id: Default::default(),
        classes: Default::default(),
        method_id: Default::default(),
        methods: Default::default(),
        field_id: Default::default(),
        fields: Default::default(),
    };

    {
        for class in &classes {
            ctx.register_class(class);
        }
        for method in &methods {
            ctx.register_method(method);
        }
    }

    {
        for class in classes {
            ctx.add_class(class);
        }
        for method in methods {
            ctx.add_method(method);
        }
    }

    let classes = ctx.classes.into_values().collect();
    let methods = ctx.methods.into_values().collect();

    ir::Program::new(classes, methods)
}

#[cfg(test)]
mod test {
    use crate::lowering::lower_program;

    #[test]
    fn test_lowering() {
        let source = include_str!("../examples/linked-list.moo");
        let lexer = crate::lexer::Lexer::new(source);
        let mut parser = crate::parser::Parser::new(lexer);
        let program = parser.parse_program().unwrap();
        let (analyzed, _) = crate::sema::analyze_program(program).unwrap();
        let lowered = lower_program(analyzed);
        println!("{lowered:#?}")
    }
}
