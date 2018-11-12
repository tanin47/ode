use std::cell::Cell;
use std::env;
use std::fs::File;
use std::path::Path;
use std::io::prelude::*;
use std::collections::HashMap;

extern crate inkwell;

use inkwell::OptimizationLevel;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, Symbol};
use inkwell::module::Module;
use inkwell::values::{FunctionValue, BasicValue, IntValue};
use inkwell::basic_block::BasicBlock;
use inkwell::targets::{InitializationConfig, Target, TargetMachine, RelocMode, CodeModel, FileType};

mod lilit;
mod ast;
mod semantics;
mod scope;

fn gen_mod<'a>(
    module: &'a semantics::Mod<'a>,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) -> Module {
    let llvm_module = context.create_module("main");
    scope.enter();
    for unit in &module.units {
        gen_mod_unit(&unit, &llvm_module, &context, &builder, &funcs, scope);
    }
    scope.leave();
    return llvm_module;
}

fn gen_mod_unit<'a>(
    unit: &'a semantics::ModUnit<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) {
    match unit {
        semantics::ModUnit::Func { func, syntax: _ } => {
            gen_func(&func, &module, &context, &builder, &funcs, scope);
        },
        _ => (),
    }
}

fn gen_func<'a>(
    func: &'a semantics::Func<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) {
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[], false);

    let function = module.add_function(&*func.syntax.id.name, &fn_type, None);
    func.llvm_ref.set(Some(function));

    scope.enter();

    for (index, expr) in func.exprs.iter().enumerate() {
        let basic_block = context.append_basic_block(&function, &format!("block_{}", index));
        if index > 0 {
            builder.build_unconditional_branch(&basic_block);
        }

        builder.position_at_end(&basic_block);

        let ret = gen_expr(&expr, &module, &context, &builder, &funcs, scope);

        if index == (func.exprs.len() - 1) {
            builder.build_return(Some(&ret));
        }
    }

    scope.leave()
}

fn gen_expr<'a>(
    expr: &'a semantics::Expr<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) -> IntValue {
    match expr {
        semantics::Expr::Invoke { invoke, syntax: _ } => {
            gen_invoke(&invoke, &module, &context, &builder, &funcs, scope)
        },
        semantics::Expr::Num { num, syntax: _ } => {
            gen_num(&num, &module, &context, &builder, scope)
        },
        semantics::Expr::Assignment { assignment, syntax: _ } => {
            gen_assignment(&assignment, &module, &context, &builder, &funcs, scope)
        },
        semantics::Expr::Var { var, syntax: _ } => {
            gen_var(&var, &module, &context, &builder, &funcs, scope)
        },
    }
}

fn gen_var<'a>(
    var: &'a semantics::Var<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) -> IntValue {
    let value = builder.build_load(&scope.read(&var.id.syntax.name).unwrap().llvm_ref.get().unwrap(), "deref");
    value.into_int_value()
}

fn gen_assignment<'a>(
    assignment: &'a semantics::Assignment<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) -> IntValue {
    let i32_type = context.i32_type();
    let ptr = builder.build_alloca(i32_type, &assignment.var.syntax.id.name);

    let expr = gen_expr(&assignment.expr, &module, &context, &builder, &funcs, scope);

    builder.build_store(&ptr, &expr);
    assignment.var.llvm_ref.set(Some(ptr));
    scope.declare(&assignment.var.syntax.id.name, &assignment.var);
    expr
}

fn gen_num<'a>(
    num: &'a semantics::Num<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    scope: &mut scope::Scope<'a>,
) -> IntValue {
    let i32_type = context.i32_type();
    i32_type.const_int(num.value as u64, false)
}

fn gen_invoke<'a>(
    invoke: &'a semantics::Invoke<'a>,
    module: &Module,
    context: &Context,
    builder: &Builder,
    funcs: &HashMap<String, &'a semantics::Func<'a>>,
    scope: &mut scope::Scope<'a>,
) -> IntValue {
    builder.build_call(&invoke.func_opt.get().unwrap().llvm_ref.get().unwrap(), &[], &invoke.syntax.id.name, false).left().unwrap().into_int_value()
}


// fn build_next_mod(
//     next_module_opt: &Option<Box<ast::Mod>>,
//     module: &Module,
//     context: &Context,
//     builder: &Builder
// ) {
//     if let Some(ref next_module) = next_module_opt {
//         add_func(&*(*next_module).func, &module, &context, &builder);
//         build_next_mod(&next_module.next_opt, &module, &context, &builder);
//     }
// }

// fn add_func(
//     func: &ast::Func,
//     module: &Module,
//     context: &Context,
//     builder: &Builder
// ) {
//     let i32_type = context.i32_type();
//     let fn_type = i32_type.fn_type(&[], false);

//     let function = module.add_function(&*func.name, &fn_type, None);
//     let basic_block = context.append_basic_block(&function, "entry");

//     builder.position_at_end(&basic_block);
//     let ret = i32_type.const_int((*func.expr).value as u64, false);
//     builder.build_return(Some(&ret));
// }

fn build_invoke<'a>(invoke: &'a ast::Invoke) -> semantics::Invoke<'a> {
    semantics::Invoke {
        func_opt: Cell::new(None),
        syntax: &invoke,
    }
}

fn build_num<'a>(num: &'a ast::Num) -> semantics::Num<'a> {
    semantics::Num {
        value: (*num).value,
        syntax: &num,
    }
}

fn build_id<'a>(id: &'a ast::Id) -> semantics::Id<'a> {
    semantics::Id {
        syntax: &id
    }
}

fn build_var<'a>(var: &'a ast::Var) -> semantics::Var<'a> {
    semantics::Var {
        llvm_ref: Cell::new(None),
        id: Box::new(build_id(&var.id)),
        syntax: &var
    }
}

fn build_assignment<'a>(assignment: &'a ast::Assignment) -> semantics::Assignment<'a> {
    semantics::Assignment {
        var: Box::new(build_var(&assignment.var)),
        expr: Box::new(build_expr(&assignment.expr)),
        syntax: &assignment,
    }
}

fn build_expr<'a>(expr: &'a ast::Expr) -> semantics::Expr<'a> {
    match expr {
        ast::Expr::Invoke(i) => semantics::Expr::Invoke {
            invoke: Box::new(build_invoke(&i)),
            syntax: &expr,
        },
        ast::Expr::Num(n) => semantics::Expr::Num {
            num: Box::new(build_num(&n)),
            syntax: &expr,
        },
        ast::Expr::Assignment(a) => semantics::Expr::Assignment {
            assignment: Box::new(build_assignment(&a)),
            syntax: &expr,
        },
        ast::Expr::Var(v) => semantics::Expr::Var {
            var: Box::new(build_var(&v)),
            syntax: &expr,
        },
    }
}

fn build_func<'a>(func: &'a ast::Func) -> semantics::Func<'a> {
    let mut vec = Vec::new();

    for expr in &(*func).exprs {
       vec.push(build_expr(&expr))
    }

    semantics::Func { llvm_ref: Cell::new(None), exprs: vec, syntax: &func }

}

fn build_class<'a>(class: &'a ast::Class) -> semantics::Class<'a> {
    semantics::Class { extends: vec![], methods: vec![], syntax: &class }
}

fn build_mod_unit<'a>(unit: &'a ast::ModUnit) -> semantics::ModUnit<'a> {
    match unit {
      ast::ModUnit::Func(func) => semantics::ModUnit::Func {
          func: Box::new(build_func(&func)),
          syntax: &unit,
      },
      ast::ModUnit::Class(class) => semantics::ModUnit::Class {
          class: Box::new(build_class(&class)),
          syntax: &unit,
      },
    }
}


fn build_mod<'a>(m: &'a ast::Mod) -> semantics::Mod<'a> {
    let mut vec = Vec::new();

    for unit in &(*m).units {
       vec.push(build_mod_unit(&unit))
    }

    semantics::Mod { units: vec, syntax: &m }
}

fn register_funcs<'a>(root: &'a semantics::Mod<'a>, funcs: &mut HashMap<String, &'a semantics::Func<'a>>) {
    for unit in &(*root).units {
        match unit {
            semantics::ModUnit::Func { func, syntax: _ } => {
                funcs.insert(func.syntax.id.name.to_string(), func);
            },
            _ => (),
        }
    }
}

fn hydrate_funcs<'a>(root: &'a semantics::Mod<'a>, funcs: &HashMap<String, &'a semantics::Func<'a>>) {
    for unit in &(*root).units {
        match unit {
            semantics::ModUnit::Func { func, syntax: _ } => {
                for expr in &func.exprs {
                    match expr {
                        semantics::Expr::Invoke { invoke, syntax: _ } => {
                            invoke.func_opt.set(funcs.get(&invoke.syntax.id.name).map(|v| *v));
                        },
                        _ => (),
                    }
                }
            },
            _ => (),
        }
    }
}

fn main() {
    println!("Lilit 0.0.1\n");
    let args: Vec<String> = env::args().collect();
    let mut file = File::open(&args[1]).expect("file not found");

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("something went wrong reading the file");

    let tree = lilit::ModParser::new().parse(&contents);

    println!("---- Code ----");
    println!("{}\n", contents);


    println!("{:?}\n", tree);

    // The first pass makes a hashtable for function and class.

    if let Ok(ref _ok_tree) = tree {
        let mut root = build_mod(_ok_tree);
        println!("{:?}\n", root);

        let mut funcs = HashMap::new();
        register_funcs(&root, &mut funcs);
        println!("{:?}\n", funcs);

        hydrate_funcs(&root, &funcs);

        println!("{:?}\n", root);

        Target::initialize_native(&InitializationConfig::default()).unwrap();

        let context = Context::create();
        let builder = context.create_builder();

        let mut scope = scope::Scope { levels: Vec::new() };
        scope.enter();
        let module = gen_mod(&root, &context, &builder, &funcs, &mut scope);
        scope.leave();

        let triple = TargetMachine::get_default_triple().to_string();
        let target = Target::from_triple(&triple).unwrap();
        let target_machine = target.create_target_machine(&triple, "generic", "", OptimizationLevel::Default, RelocMode::Default, CodeModel::Default).unwrap();

        let path =  Path::new("./output.o\0");
        let result = target_machine.write_to_file(&module, FileType::Object, &path);
        println!("---- LLVM IR ----");
        module.print_to_stderr();

    //     // This is an object file. In order to run it as a binary,
    //     // we need to link it using `cc output.o -o output`.
    //     // Now you can run `./output`.
    }
}

