use LilitFile;
use analyse::scope::Scope;
use parse::tree::{CompilationUnitItem, Class, Method};
use analyse::def::{class, method};
use index::tree::Root;

pub mod def;
pub mod expr;
pub mod scope;
pub mod tpe;

pub fn apply<'def>(
    files: &mut [&mut LilitFile<'def>],
    root: &Root<'def>,
) {
    for file in files {
        apply_file(file, root);
    }
}

pub fn apply_file<'def>(
    file: &mut LilitFile<'def>,
    root: &Root<'def>,
) {
    let mut scope = Scope { levels: vec![] };
    scope.enter_root(root);
   // Add all import statements to scope

    for item in &mut file.unit.items {
        match item {
            CompilationUnitItem::Class(c) => class::apply(c, &mut scope),
            CompilationUnitItem::Method(m) => method::apply(m, None, &mut scope),
        }
    }
    scope.leave();
}

#[cfg(test)]
mod tests {
    use std::ops::{Deref, DerefMut};

    use index::build;
    use parse;
    use parse::tree::{CompilationUnit, Type, CompilationUnitItem, Method, Invoke, Expr, Class};
    use test_common::span2;
    use analyse::apply;
    use std::cell::Cell;

    #[test]
    fn test_full() {
        let content = r#"
class Native__String
end

class String(underlying: Native__String)
end

class Void
end

def native__printf(text: String): Void

end

def main: Void
  native__printf("hello")
end
        "#;
        let mut file = unwrap!(Ok, parse::apply(content.trim(), ""));
        let root = build(&[file.deref()]);

        apply(&mut [file.deref_mut()], &root);

        println!("{:#?}", file.unit);
    }

    #[test]
    fn test_simple() {
        let content = r#"
class Number
end

def test(): Number
end

def main(): Number
  test()
end
        "#;
        let mut file = unwrap!(Ok, parse::apply(content.trim(), ""));
        let root = build(&[file.deref()]);

        apply(&mut [file.deref_mut()], &root);

        assert_eq!(
            file.unit,
            CompilationUnit {
                items: vec![
                    CompilationUnitItem::Class(Class {
                        name: span2(1, 7, "Number", file.deref()),
                        params: vec![],
                        methods: vec![],
                        llvm: Cell::new(None),
                        llvm_native: Cell::new(None)
                    }),
                    CompilationUnitItem::Method(Method {
                        name: span2(4, 5, "test", file.deref()),
                        params: vec![],
                        exprs: vec![],
                        return_type: Type { span: Some(span2(4, 13, "Number", file.deref())), class_def: Some(root.find_class("Number")) },
                        parent_class: None,
                        llvm: Cell::new(None)
                    }),
                    CompilationUnitItem::Method(Method {
                        name: span2(7, 5, "main", file.deref()),
                        params: vec![],
                        exprs: vec![
                            Expr::Invoke(Box::new(Invoke {
                                invoker_opt: None,
                                name: span2(8, 3, "test", file.deref()),
                                args: vec![],
                                method_def: Some(root.find_method("test")),
                            }))
                        ],
                        return_type: Type { span: Some(span2(7, 13, "Number", file.deref())), class_def: Some(root.find_class("Number")) },
                        parent_class: None,
                        llvm: Cell::new(None)
                    }),
                ]
            }
        )
    }
}
