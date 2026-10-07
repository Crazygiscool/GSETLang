//! Inference tests over real Python lowered by the frontend.
//!
//! The point of these is the property the crate promises: where a type is
//! knowable it is filled in, and where it is not the result is [`Type::Unknown`]
//! rather than a guess.

use gset_frontend::frontend_by_name;
use gset_ir::{IntWidth, Item, Module, Type, VarDecl, Variance};
use gset_semantic::infer;

fn analyzed(source: &str) -> Module {
    let frontend = frontend_by_name("python").expect("the python frontend is compiled in");
    let lowered = frontend.lower("test.py", source);
    assert!(
        !lowered.diagnostics.has_errors(),
        "lowering reported errors: {:?}",
        lowered.diagnostics
    );
    let mut module = lowered.module.expect("a module was lowered");
    infer(&mut module);
    module
}

fn global(module: &Module, name: &str) -> Type {
    global_decl(module, name)
        .ty
        .clone()
        .unwrap_or(Type::UNKNOWN)
}

fn global_decl<'module>(module: &'module Module, name: &str) -> &'module VarDecl {
    module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Global(decl)
                if decl.pattern.single_binding().map(|binding| &**binding) == Some(name) =>
            {
                Some(decl)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no module-level binding named `{name}`"))
}

fn int() -> Type {
    Type::Int(IntWidth::I64)
}

#[test]
fn infers_scalar_literals() {
    let module = analyzed("x = 1\ny = 1.5\ns = \"hi\"\nb = True\nn = None\n");
    assert_eq!(global(&module, "x"), int());
    assert_eq!(global(&module, "y"), Type::Float);
    assert_eq!(global(&module, "s"), Type::Str);
    assert_eq!(global(&module, "b"), Type::Bool);
    assert_eq!(global(&module, "n"), Type::Null);
}

#[test]
fn infers_list_element_type() {
    let module = analyzed("xs = [1, 2, 3]\n");
    assert_eq!(
        global(&module, "xs"),
        Type::List {
            element: Box::new(int()),
            variance: Variance::Covariant,
        }
    );
}

#[test]
fn an_empty_list_has_an_unknown_element_type() {
    let module = analyzed("xs = []\n");
    assert!(matches!(
        global(&module, "xs"),
        Type::List { element, .. } if element.is_unknown()
    ));
}

#[test]
fn a_mixed_list_stays_unknown_rather_than_picking_a_side() {
    let module = analyzed("xs = [1, \"a\"]\n");
    assert!(matches!(
        global(&module, "xs"),
        Type::List { element, .. } if element.is_unknown()
    ));
}

#[test]
fn arithmetic_promotes_to_float() {
    let module = analyzed("x = 1 + 2.0\n");
    assert_eq!(global(&module, "x"), Type::Float);
}

#[test]
fn python_true_division_yields_a_float() {
    let module = analyzed("x = 1 / 2\n");
    assert_eq!(global(&module, "x"), Type::Float);
}

#[test]
fn floor_division_stays_integral() {
    let module = analyzed("x = 7 // 2\n");
    assert_eq!(global(&module, "x"), int());
}

#[test]
fn string_concatenation_is_a_string() {
    let module = analyzed("x = \"a\" + \"b\"\n");
    assert_eq!(global(&module, "x"), Type::Str);
}

#[test]
fn arithmetic_on_a_string_is_unknown_not_a_guess() {
    let module = analyzed("x = \"a\" * 2\n");
    assert!(global(&module, "x").is_unknown());
}

#[test]
fn comparison_is_bool() {
    let module = analyzed("x = 1 < 2\n");
    assert_eq!(global(&module, "x"), Type::Bool);
}

#[test]
fn a_call_to_an_annotated_function_uses_its_return_type() {
    let module = analyzed("def f(x: int) -> str:\n    return str(x)\n\ny = f(1)\n");
    assert_eq!(global(&module, "y"), Type::Str);
}

#[test]
fn a_local_binding_is_inferred_from_its_value() {
    let module = analyzed("def f() -> int:\n    x = 1\n    return x\n");
    let function = module.functions().next().expect("one function");
    assert_eq!(function.ret, Some(int()));
}

#[test]
fn an_unannotated_return_type_is_inferred_from_the_body() {
    let module = analyzed("def f():\n    return \"hello\"\n");
    let function = module.functions().next().expect("one function");
    assert_eq!(function.ret, Some(Type::Str));
}

#[test]
fn a_function_with_conflicting_returns_stays_unknown() {
    let module = analyzed("def f(c):\n    if c:\n        return 1\n    return \"a\"\n");
    let function = module.functions().next().expect("one function");
    assert_eq!(function.ret, None);
    assert!(function.signature().is_callable());
}

#[test]
fn len_is_always_an_integer() {
    let module = analyzed("xs = [1, 2]\nn = len(xs)\n");
    assert_eq!(global(&module, "n"), int());
}

#[test]
fn iteration_binds_the_element_type() {
    let module = analyzed(
        "def f():\n    total = 0\n    for x in [1, 2, 3]:\n        total = x\n    return total\n",
    );
    let function = module.functions().next().expect("one function");
    assert_eq!(function.ret, Some(int()));
}

#[test]
fn a_shadowed_local_does_not_leak_into_the_module_scope() {
    let module = analyzed("x = 1\ndef f():\n    x = \"local\"\n    return x\n");
    assert_eq!(global(&module, "x"), int());
    let function = module.functions().next().expect("one function");
    assert_eq!(function.ret, Some(Type::Str));
}

#[test]
fn a_conditional_with_a_null_branch_becomes_optional() {
    let module = analyzed("x = 1 if True else None\n");
    assert_eq!(global(&module, "x"), Type::Option(Box::new(int())));
}
