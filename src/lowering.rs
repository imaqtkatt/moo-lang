use std::collections::BTreeMap;

use crate::{
    sema,
    shared::Selector,
    tree::{ir, typed},
};

#[derive(Debug)]
pub struct Context {
    //
    // mutable fields
    //
    next_local: usize,
    locals: BTreeMap<String, Vec<ir::Local>>,
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

    type_context: sema::TypeContext,
}

impl Context {
    fn add_local(&mut self, name: &str) -> ir::Local {
        let next_id = self.next_local;
        self.next_local += 1;

        let local = ir::Local(next_id);

        self.locals
            .entry(String::from(name))
            .or_default()
            .push(local);

        local
    }

    fn lookup_local(&self, name: &str) -> ir::Local {
        self.locals[name].last().copied().unwrap()
    }

    fn pop_local(&mut self, name: &str) {
        if let Some(scope) = self.locals.get_mut(name) {
            scope.pop();
        }
    }

    fn next_class_id(&mut self, class_name: &str) -> ir::ClassId {
        let next_id = self.class_id.len();
        let class_id = ir::ClassId(next_id);
        if self.class_id.insert(class_name.into(), class_id).is_some() {
            panic!("redefinition of {class_name}");
        };
        class_id
    }

    fn next_method_id(&mut self, selector: (ir::ClassId, Selector)) -> ir::MethodId {
        let next_id = self.method_id.len();
        let method_id = ir::MethodId(next_id);
        if self.method_id.insert(selector, method_id).is_some() {
            panic!("redefinition of method ..");
        }
        method_id
    }

    fn next_field_id(&mut self, field: (ir::ClassId, String)) -> ir::FieldId {
        let next_id = self.field_id.len();
        let field_id = ir::FieldId(next_id);
        if self.field_id.insert(field, field_id).is_some() {
            panic!("redefinition of field ..");
        }
        field_id
    }

    fn register_class(&mut self, class: &typed::ClassDefinition) {
        self.next_class_id(&class.class_name);
    }

    fn add_class(&mut self, class: typed::ClassDefinition) -> ir::ClassId {
        let class_id = self.class_id[&class.class_name];

        self.class_type_to_id.insert(class.class_type, class_id);

        let mut fields = vec![];
        for (offset, (name, ty)) in class.fields.into_iter().enumerate() {
            let field_id = self.next_field_id((class_id, name.clone()));

            let field = ir::Field {
                id: field_id,
                offset,
                name: name.clone(),
                ty,
            };
            fields.push(field_id);

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
            let locals = self.next_local;

            self.reset_context();

            (result, locals)
        };

        let ir_method = ir::Method {
            id: method_id,
            method_type: method.method_type.into(),
            receiver: class_id,
            selector: method.selector,
            parameters: method.parameter_types,
            return_type: method.return_type,
            locals,
            body: lowered_body,
        };

        self.methods.insert(method_id, ir_method);

        self.classes
            .get_mut(&class_id)
            .unwrap()
            .methods
            .push(method_id);

        method_id
    }

    fn reset_context(&mut self) {
        self.current_class = None;
        self.next_local = 0;
        self.locals.clear();
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
        typed::Expression::Pipe(initial, messages) => lower_pipe(ctx, initial, messages),
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
    ctx.pop_local(&bind);

    ir::Expr::Let(local, lowered_value, lowered_next)
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

    ir::Expr::If(lowered_condition, lowered_consequence, lowered_alternative)
}

fn lower_if_let_then_else(
    ctx: &mut Context,
    nullable: typed::Typed<typed::Expression>,
    refined: Option<String>,
    consequence: typed::Typed<typed::Expression>,
    alternative: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let lowered_nullable = lower_typed_expr(nullable, ctx);

    let check_not_null = ir::Expression::new(ir::Expr::NotNull(lowered_nullable.clone()));

    let nullable_local = ctx.add_local("tmp:nullable");
    let nullable_ref = ir::Expression::new(ir::Expr::Variable(nullable_local));

    let if_then_else = if let Some(refined) = refined {
        let local = ctx.add_local(&refined);

        let lowered_consequence = lower_typed_expr(consequence, ctx);

        let lowered_consequence = ir::Expression::new(ir::Expr::Let(
            local,
            nullable_ref.clone(),
            lowered_consequence,
        ));

        let lowered_alternative = lower_typed_expr(alternative, ctx);

        ir::Expr::If(check_not_null, lowered_consequence, lowered_alternative)
    } else {
        let lowered_consequence = lower_typed_expr(consequence, ctx);
        let lowered_alternative = lower_typed_expr(alternative, ctx);
        ir::Expr::If(check_not_null, lowered_consequence, lowered_alternative)
    };

    ir::Expr::Let(
        nullable_local,
        lowered_nullable,
        ir::Expression::new(if_then_else),
    )
}

fn lower_seq(
    ctx: &mut Context,
    a: typed::Typed<typed::Expression>,
    b: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let a = lower_typed_expr(a, ctx);
    let b = lower_typed_expr(b, ctx);

    ir::Expr::Seq(a, b)
}

fn lower_cascade(
    ctx: &mut Context,
    receiver: typed::Typed<typed::Expression>,
    messages: Vec<(Selector, Vec<typed::Typed<typed::Expression>>)>,
) -> ir::Expr {
    assert!(messages.len() >= 2);

    let crate::sema::Type::Class(class_type, _) = ctx.type_context.get(receiver.r#type) else {
        unreachable!()
    };
    let class_id = ctx.class_type_to_id[class_type];
    let lowered_receiver = lower_typed_expr(receiver, ctx);

    let local_receiver = ctx.add_local("tmp:receiver");
    let receiver = ir::Expression::new(ir::Expr::Variable(local_receiver));

    let mut lowered_calls = Vec::new();

    for (selector, arguments) in messages {
        let method_id = ctx.method_id[&(class_id, selector)];
        let lowered_arguments = lower_many(arguments, ctx);
        let lowered_call = ir::Expr::InstanceCall(receiver.clone(), method_id, lowered_arguments);
        lowered_calls.push(lowered_call);
    }

    let last = lowered_calls.pop().unwrap();

    let seq = lowered_calls.into_iter().rfold(last, |acc, next| {
        ir::Expr::Seq(ir::Expression::new(next), ir::Expression::new(acc))
    });

    ir::Expr::Let(local_receiver, lowered_receiver, ir::Expression::new(seq))
}

fn lower_pipe(
    ctx: &mut Context,
    initial: typed::Typed<typed::Expression>,
    calls: Vec<(Selector, Vec<typed::Typed<typed::Expression>>)>,
) -> ir::Expr {
    assert!(!calls.is_empty());

    let crate::sema::Type::Class(curr_class_type, _) = ctx.type_context.get(initial.r#type) else {
        unreachable!()
    };
    let mut curr_class_type = *curr_class_type;
    let mut curr_class_id = ctx.class_type_to_id[&curr_class_type];

    let mut lowered_calls = std::rc::Rc::unwrap_or_clone(lower_typed_expr(initial, ctx));

    for (selector, arguments) in calls {
        let method_id = ctx.method_id[&(curr_class_id, selector)];
        let method = ctx.methods.get(&method_id).unwrap();
        let method_return_type = method.return_type;

        let lowered_arguments = lower_many(arguments, ctx);

        lowered_calls = ir::Expr::InstanceCall(
            ir::Expression::new(lowered_calls),
            method_id,
            lowered_arguments,
        );

        let crate::sema::Type::Class(next_class_type, _) = ctx.type_context.get(method_return_type)
        else {
            unreachable!()
        };

        curr_class_type = *next_class_type;
        curr_class_id = ctx.class_type_to_id[&curr_class_type];
    }

    lowered_calls
}

fn lower_load(ctx: &mut Context, name: String) -> ir::Expr {
    let class = ctx.current_class.unwrap();
    let field_id = ctx.field_id[&(class, name)];
    let field_offset = ctx.fields[&(class, field_id)].offset;

    ir::Expr::FieldGet(
        ir::Expression::new(ir::Expr::SelfRef),
        field_id,
        field_offset,
    )
}

fn lower_store(
    ctx: &mut Context,
    name: String,
    value: typed::Typed<typed::Expression>,
) -> ir::Expr {
    let class = ctx.current_class.unwrap();
    let field_id = ctx.field_id[&(class, name)];
    let field_offset = ctx.fields[&(class, field_id)].offset;

    let lowered_value = lower_typed_expr(value, ctx);

    ir::Expr::FieldSet(
        ir::Expression::new(ir::Expr::SelfRef),
        field_id,
        field_offset,
        lowered_value,
    )
}

fn lower_instance_call(
    ctx: &mut Context,
    receiver: typed::Typed<typed::Expression>,
    selector: Selector,
    arguments: Vec<typed::Typed<typed::Expression>>,
) -> ir::Expr {
    // let class_type_id = receiver.r#type;
    // let class_type = todo!();
    let crate::sema::Type::Class(class_type, _) = ctx.type_context.get(receiver.r#type) else {
        unreachable!()
    };
    let class_id = ctx.class_type_to_id[class_type];

    let lowered_receiver = lower_typed_expr(receiver, ctx);

    let method_id = ctx.method_id[&(class_id, selector)];

    let lowered_arguments = lower_many(arguments, ctx);

    ir::Expr::InstanceCall(lowered_receiver, method_id, lowered_arguments)
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

pub fn lower_program(
    typed::Program(tree): typed::Program,
    type_context: sema::TypeContext,
) -> (ir::Program, sema::TypeContext) {
    let mut classes = vec![];
    let mut methods = vec![];

    for item in tree {
        match item {
            typed::TopLevel::ClassDefinition(class) => classes.push(class),
            typed::TopLevel::MethodDefinition(method) => methods.push(method),
        }
    }

    let mut ctx = Context {
        next_local: 0,
        type_context,
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
    let fields = ctx.fields.into_values().collect();

    (ir::Program::new(classes, methods, fields), ctx.type_context)
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
        let (analyzed, ctx) = crate::sema::analyze_program(program).unwrap();
        let (lowered, _) = lower_program(analyzed, ctx.type_context);
        println!("classes = {:?}", lowered.classes);
        println!("\n\n\nmethods = {:?}", lowered.methods);
        // println!("methods = {:?}", lowered.methods);
    }
}
