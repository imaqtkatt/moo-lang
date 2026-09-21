use std::io::Read;

mod builtins;
mod interp_ir;
mod lexer;
mod lowering;
mod parser;
mod sema;
mod shared;
mod tree;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();

    let file_path = args.get(1).expect("file path");
    let mut file = std::fs::File::open(file_path)?;

    let mut source_buf = String::new();
    {
        file.read_to_string(&mut source_buf)?;
    }

    let lexer = lexer::Lexer::new(&source_buf);

    let mut parser = parser::Parser::new(lexer);
    let program = match parser.parse_program() {
        Ok(value) => value,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(0);
        }
    };

    println!("===");
    println!("{program:?}");
    println!("===");

    let (program, ctx) = match sema::analyze_program(program) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(0);
        }
    };

    let (program, tc) = lowering::lower_program(program, ctx.type_context);
    println!("{tc:?}");

    let value = interp_ir::eval_ir_program(program);
    println!("{value:?}");

    Ok(())
}
