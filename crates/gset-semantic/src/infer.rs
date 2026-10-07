//! Conservative type inference over the IR.
//!
//! The pass walks a [`Module`] and fills in [`Expr::ty`] wherever a type can be
//! established from the source alone: literals, annotations, arithmetic, and
//! calls to functions and builtins whose result is known. It never guesses. A
//! value whose type cannot be established stays [`Type::UNKNOWN`] and the
//! uncertainty propagates through every operation that touches it, which is what
//! lets a backend choose a documented conservative strategy instead of emitting
//! confidently-wrong code.
//!
//! It also fills a declaration's type when the source omitted one and the
//! initialiser is inferable. That is the behaviour [`VarDecl::ty`] documents:
//! "inference in `gset-semantic` fills it in, or it stays absent and the backend
//! picks a conservative default". [`Function::ret`] is treated the same way,
//! because a backend emitting a Go signature needs a return type and Python
//! rarely states one.
//!
//! # Two passes, one direction
//!
//! Module-level signatures are collected before any body is walked, so a call to
//! a function written earlier resolves. The signature map is then updated as
//! each function's return type is inferred, and bodies are walked in source
//! order. A call to a function defined *later* therefore sees only its declared
//! annotation, not its inferred return type. That is a deliberate limitation:
//! resolving it needs a fixed point over the call graph, and a fixed point is
//! not worth the complexity until the corpus shows it is.

use std::collections::HashMap;

use gset_ir::{
    BinaryOp, Block, ComprehensionKind, Else, Expr, ExprKind, Function, IntWidth, Item, LangId,
    Literal, MatchCase, Module, Name, Pattern, PatternKind, Stmt, Type, UnaryOp, VarDecl, Variance,
};

/// The IR's representation of a Python `int`.
///
/// Matches the width `gset-frontend` assigns to an `int` annotation so an
/// inferred literal and a declared type compare equal. Backends currently map
/// every integer width to the same target type, so the choice is not yet
/// observable in output.
fn int() -> Type {
    Type::Int(IntWidth::I64)
}

/// Infers types across an entire module in place.
///
/// Returns a diagnostic bag. Conservative inference does not currently report
/// anything — an unknown type is a legitimate outcome, not an error — but the
/// signature leaves room for a later pass to report an inference cycle or an
/// unresolvable call without changing every caller.
pub fn infer(module: &mut Module) -> gset_ir::DiagnosticBag {
    let mut functions: HashMap<Name, Type> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some((function.name.clone(), signature_of(function))),
            _ => None,
        })
        .collect();

    let mut env = Env::new(module.lang);
    for item in &module.items {
        match item {
            Item::Function(function) => env.define(function.name.clone(), signature_of(function)),
            Item::Global(global) => {
                if let (Some(binding), Some(ty)) = (global.pattern.single_binding(), &global.ty) {
                    env.define(binding.clone(), ty.clone());
                }
            }
            _ => {}
        }
    }

    // Python binds a module-level name when the module finishes loading, so a
    // function written above the assignment still sees the type of the value
    // declared below it. Every module-level name is therefore inferred from its
    // value before any body is walked, which also lets one global refer to
    // another declared after it.
    for index in 0..module.items.len() {
        let Item::Global(global) = &mut module.items[index] else {
            continue;
        };
        let Some(binding) = global.pattern.single_binding() else {
            continue;
        };
        // Each value is read in a copy of the scope, so a name this pass has
        // not reached yet reads as unknown instead of as something wrong.
        let mut scope = env.clone();
        let value = global
            .value
            .as_mut()
            .map(|value| infer_expr(value, &functions, &mut scope));
        let ty = global.ty.clone().or(value).unwrap_or(Type::UNKNOWN);
        if global.ty.is_none() && ty.is_known() {
            global.ty = Some(ty.clone());
        }
        env.define(binding.clone(), ty);
    }

    for index in 0..module.items.len() {
        match &mut module.items[index] {
            Item::Function(function) => {
                infer_function(function, &functions, &mut env);
                let signature = signature_of(function);
                functions.insert(function.name.clone(), signature.clone());
                env.redefine(function.name.clone(), signature);
            }
            Item::Global(global) => infer_global(global, &functions, &mut env),
            Item::Stmt(statement) => {
                let mut returns = None;
                infer_stmt(statement, &functions, &mut env, &mut returns);
            }
            _ => {}
        }
    }

    gset_ir::DiagnosticBag::new()
}

/// The signature of a function as a callable type.
fn signature_of(function: &Function) -> Type {
    Type::Function {
        params: function
            .param_types
            .iter()
            .map(|ty| ty.clone().unwrap_or(Type::UNKNOWN))
            .collect(),
        ret: Box::new(function.ret.clone().unwrap_or(Type::UNKNOWN)),
        variadic: function.variadic,
        param_names: function
            .params
            .iter()
            .map(|pattern| pattern.names.first().cloned().unwrap_or_default())
            .collect(),
    }
}

/// A stack of lexical scopes.
///
/// The module scope is the bottom entry; every function, block and comprehension
/// pushes one more. A lookup searches from the innermost scope outward, which is
/// what lets a local shadow a module-level name instead of the two being
/// indistinguishable at emit time.
#[derive(Clone)]
struct Env {
    scopes: Vec<HashMap<Name, Type>>,
    lang: LangId,
}

impl Env {
    fn new(lang: LangId) -> Self {
        Env {
            scopes: vec![HashMap::new()],
            lang,
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    /// Binds a name in the innermost scope, replacing any existing binding.
    fn define(&mut self, name: Name, ty: Type) {
        self.scopes
            .last_mut()
            .expect("an environment always has a module scope")
            .insert(name, ty);
    }

    /// Binds a name in the innermost scope, joining with any type it already had
    /// *in that scope*.
    ///
    /// Reassignment is how Python changes a variable's type, so a second binding
    /// with an incompatible type must become `Unknown` rather than the new type.
    /// Only the innermost scope is consulted: an assignment inside a function
    /// creates a local that shadows a module-level name rather than widening it.
    fn redefine(&mut self, name: Name, ty: Type) {
        let scope = self
            .scopes
            .last_mut()
            .expect("an environment always has a module scope");
        let joined = match scope.get(&name) {
            Some(previous) => previous.join(&ty),
            None => ty,
        };
        scope.insert(name, joined);
    }

    fn lookup(&self, name: &str) -> Option<Type> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
}

fn infer_function(function: &mut Function, functions: &HashMap<Name, Type>, env: &mut Env) {
    env.push();
    for (index, pattern) in function.params.iter().enumerate() {
        let ty = function
            .param_types
            .get(index)
            .and_then(|ty| ty.clone())
            .unwrap_or(Type::UNKNOWN);
        bind_pattern(pattern, &ty, env);
    }
    let mut returns = None;
    infer_block(&mut function.body, functions, env, &mut returns);
    env.pop();

    if function.ret.is_none()
        && let Some(ty) = returns
        && ty.is_known()
        && !matches!(ty, Type::Void | Type::Never)
    {
        function.ret = Some(ty);
    }
}

fn infer_global(global: &mut VarDecl, functions: &HashMap<Name, Type>, env: &mut Env) {
    let value = global
        .value
        .as_mut()
        .map(|value| infer_expr(value, functions, env));
    let ty = global.ty.clone().or(value).unwrap_or(Type::UNKNOWN);
    if global.ty.is_none() && ty.is_known() {
        global.ty = Some(ty.clone());
    }
    bind_pattern(&global.pattern, &ty, env);
}

fn infer_block(
    block: &mut Block,
    functions: &HashMap<Name, Type>,
    env: &mut Env,
    returns: &mut Option<Type>,
) {
    env.push();
    for statement in &mut block.statements {
        infer_stmt(statement, functions, env, returns);
    }
    env.pop();
}

fn infer_stmt(
    statement: &mut Stmt,
    functions: &HashMap<Name, Type>,
    env: &mut Env,
    returns: &mut Option<Type>,
) {
    match statement {
        Stmt::Decl(decl) => {
            let value = decl
                .value
                .as_mut()
                .map(|value| infer_expr(value, functions, env));
            let ty = decl.ty.clone().or(value).unwrap_or(Type::UNKNOWN);
            if decl.ty.is_none() && ty.is_known() {
                decl.ty = Some(ty.clone());
            }
            bind_pattern(&decl.pattern, &ty, env);
        }
        Stmt::Expr(expr) => {
            infer_expr(expr, functions, env);
        }
        Stmt::Assign { target, value, .. } => {
            let ty = infer_expr(value, functions, env);
            bind_assignment(target, &ty, env);
        }
        Stmt::Return { value, .. } => match value {
            Some(value) => {
                let ty = infer_expr(value, functions, env);
                join_into(returns, ty);
            }
            None => join_into(returns, Type::Void),
        },
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            infer_expr(condition, functions, env);
            infer_block(then_branch, functions, env, returns);
            match else_branch.as_deref_mut() {
                Some(Else::Block(block)) => infer_block(block, functions, env, returns),
                Some(Else::If(nested)) => infer_stmt(nested, functions, env, returns),
                None => {}
            }
        }
        Stmt::While {
            condition,
            body,
            else_body,
            ..
        } => {
            infer_expr(condition, functions, env);
            infer_block(body, functions, env, returns);
            if let Some(else_body) = else_body {
                infer_block(else_body, functions, env, returns);
            }
        }
        Stmt::DoWhile {
            body, condition, ..
        } => {
            infer_block(body, functions, env, returns);
            infer_expr(condition, functions, env);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            env.push();
            if let Some(init) = init {
                infer_stmt(init, functions, env, returns);
            }
            if let Some(condition) = condition {
                infer_expr(condition, functions, env);
            }
            if let Some(update) = update {
                infer_expr(update, functions, env);
            }
            infer_block(body, functions, env, returns);
            env.pop();
        }
        Stmt::ForIn {
            pattern,
            iterable,
            body,
            else_body,
            ..
        } => {
            env.push();
            let iterable_ty = infer_expr(iterable, functions, env);
            bind_pattern(pattern, &element_type(&iterable_ty), env);
            infer_block(body, functions, env, returns);
            if let Some(else_body) = else_body {
                infer_block(else_body, functions, env, returns);
            }
            env.pop();
        }
        Stmt::Block(block) => infer_block(block, functions, env, returns),
        Stmt::Switch {
            scrutinee,
            cases,
            default,
            ..
        } => {
            let scrutinee_ty = scrutinee
                .as_mut()
                .map(|value| infer_expr(value, functions, env))
                .unwrap_or(Type::UNKNOWN);
            env.push();
            for MatchCase {
                pattern,
                guard,
                body,
                ..
            } in cases.iter_mut()
            {
                bind_pattern(pattern, &scrutinee_ty, env);
                if let Some(guard) = guard {
                    infer_expr(guard, functions, env);
                }
                infer_block(body, functions, env, returns);
            }
            if let Some(default) = default {
                infer_block(default, functions, env, returns);
            }
            env.pop();
        }
        Stmt::Throw { value, .. } => {
            if let Some(value) = value {
                infer_expr(value, functions, env);
            }
        }
        Stmt::Try {
            body,
            handlers,
            finally,
            ..
        } => {
            infer_block(body, functions, env, returns);
            for handler in handlers.iter_mut() {
                env.push();
                if let Some(binding) = &handler.binding {
                    bind_pattern(binding, &Type::UNKNOWN, env);
                }
                infer_block(&mut handler.body, functions, env, returns);
                env.pop();
            }
            if let Some(finally) = finally {
                infer_block(finally, functions, env, returns);
            }
        }
        Stmt::With {
            value,
            binding,
            body,
            ..
        } => {
            let ty = infer_expr(value, functions, env);
            env.push();
            if let Some(binding) = binding {
                bind_pattern(binding, &ty, env);
            }
            infer_block(body, functions, env, returns);
            env.pop();
        }
        Stmt::Defer { expr, .. } => {
            infer_expr(expr, functions, env);
        }
        Stmt::Assert {
            condition, message, ..
        } => {
            infer_expr(condition, functions, env);
            if let Some(message) = message {
                infer_expr(message, functions, env);
            }
        }
        Stmt::Delete { target, .. } => {
            infer_expr(target, functions, env);
        }
        Stmt::Yield { value, .. } => {
            if let Some(value) = value {
                infer_expr(value, functions, env);
            }
        }
        Stmt::LocalItem(item) => match item.as_mut() {
            Item::Function(function) => {
                infer_function(function, functions, env);
                env.redefine(function.name.clone(), signature_of(function));
            }
            Item::Global(global) => infer_global(global, functions, env),
            _ => {}
        },
        Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Error { .. } => {}
    }
}

fn join_into(accumulator: &mut Option<Type>, ty: Type) {
    let joined = match accumulator.take() {
        Some(previous) => previous.join(&ty),
        None => ty,
    };
    *accumulator = Some(joined);
}

/// Binds the names in `pattern` to (parts of) `ty`.
fn bind_pattern(pattern: &Pattern, ty: &Type, env: &mut Env) {
    match &pattern.kind {
        PatternKind::Bind => {
            if pattern.names.len() == 1 {
                env.define(pattern.names[0].clone(), ty.clone());
            } else {
                for name in &pattern.names {
                    env.define(name.clone(), Type::UNKNOWN);
                }
            }
        }
        PatternKind::Sequence => match ty {
            Type::Tuple(elements) if elements.len() == pattern.subpatterns.len() => {
                for (subpattern, element) in pattern.subpatterns.iter().zip(elements) {
                    bind_pattern(subpattern, element, env);
                }
            }
            Type::List { element, .. } => {
                for subpattern in &pattern.subpatterns {
                    bind_pattern(subpattern, element, env);
                }
            }
            _ => {
                for subpattern in &pattern.subpatterns {
                    bind_pattern(subpattern, &Type::UNKNOWN, env);
                }
            }
        },
        PatternKind::Mapping => {
            let value = map_value_type(ty);
            for subpattern in &pattern.subpatterns {
                bind_pattern(subpattern, &value, env);
            }
        }
        PatternKind::Ignore | PatternKind::Location(_) => {}
    }
}

/// Binds an assignment target, joining with a previous binding of the same name.
fn bind_assignment(pattern: &Pattern, ty: &Type, env: &mut Env) {
    match &pattern.kind {
        PatternKind::Bind if pattern.names.len() == 1 => {
            env.redefine(pattern.names[0].clone(), ty.clone());
        }
        PatternKind::Sequence => {
            for subpattern in &pattern.subpatterns {
                bind_assignment(subpattern, &Type::UNKNOWN, env);
            }
        }
        PatternKind::Mapping => {
            for subpattern in &pattern.subpatterns {
                bind_assignment(subpattern, &Type::UNKNOWN, env);
            }
        }
        PatternKind::Bind | PatternKind::Ignore | PatternKind::Location(_) => {}
    }
}

/// The type of the values in a mapping-like type.
fn map_value_type(ty: &Type) -> Type {
    match ty {
        Type::Map(value) => value.as_ref().clone(),
        Type::OrderedMap { value, .. } => value.as_ref().clone(),
        _ => Type::UNKNOWN,
    }
}

/// The type a `for`/comprehension binding sees for each element of `ty`.
fn element_type(ty: &Type) -> Type {
    match ty {
        Type::List { element, .. } | Type::Set(element) => element.as_ref().clone(),
        Type::Str | Type::StrLit => Type::Str,
        Type::Map(_) => Type::Str,
        Type::OrderedMap { key, .. } => key.as_ref().clone(),
        _ => Type::UNKNOWN,
    }
}

/// The result type of indexing `target` with `index`.
fn index_type(target: &Type, index: &Expr, index_ty: &Type) -> Type {
    match target {
        Type::List { element, .. } => element.as_ref().clone(),
        Type::Tuple(elements) => match integer_literal(index) {
            Some(position) => elements
                .get(position as usize)
                .cloned()
                .unwrap_or(Type::UNKNOWN),
            None => Type::UNKNOWN,
        },
        Type::Str | Type::StrLit => Type::Str,
        Type::Bytes => Type::Int(IntWidth::I8),
        Type::Map(value) => value.as_ref().clone(),
        Type::OrderedMap { value, .. } => value.as_ref().clone(),
        Type::Unknown(_) => Type::UNKNOWN,
        _ if index_ty.is_unknown() => Type::UNKNOWN,
        _ => Type::UNKNOWN,
    }
}

/// Parses an integer literal in an indexing position, for tuple indexing.
fn integer_literal(expr: &Expr) -> Option<u64> {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(text)) => text.parse().ok(),
        _ => None,
    }
}

fn infer_expr(expr: &mut Expr, functions: &HashMap<Name, Type>, env: &mut Env) -> Type {
    let ty = infer_kind(expr, functions, env);
    expr.ty = ty.clone();
    ty
}

fn infer_kind(expr: &mut Expr, functions: &HashMap<Name, Type>, env: &mut Env) -> Type {
    match &mut expr.kind {
        ExprKind::Literal(literal) => literal_type(literal),
        ExprKind::Path(path) => {
            if path.is_bare() {
                path.segments
                    .first()
                    .and_then(|name| env.lookup(name))
                    .unwrap_or(Type::UNKNOWN)
            } else {
                Type::UNKNOWN
            }
        }
        ExprKind::Unary { op, operand } => {
            let operand_ty = infer_expr(operand, functions, env);
            match op {
                UnaryOp::Not => Type::Bool,
                UnaryOp::Neg | UnaryOp::BitNot => operand_ty,
                UnaryOp::Ref
                | UnaryOp::MutRef
                | UnaryOp::Deref
                | UnaryOp::PreIncrement
                | UnaryOp::PostIncrement
                | UnaryOp::PreDecrement
                | UnaryOp::PostDecrement => operand_ty,
            }
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let left = infer_expr(lhs, functions, env);
            let right = infer_expr(rhs, functions, env);
            binary_type(*op, &left, &right, env.lang)
        }
        ExprKind::Compare { lhs, rhs, .. } => {
            infer_expr(lhs, functions, env);
            infer_expr(rhs, functions, env);
            Type::Bool
        }
        ExprKind::Logical { lhs, rhs, .. } => {
            let left = infer_expr(lhs, functions, env);
            let right = infer_expr(rhs, functions, env);
            left.join(&right)
        }
        ExprKind::Call {
            callee,
            args,
            named_args,
        } => {
            let callee_ty = infer_expr(callee, functions, env);
            for arg in args.iter_mut() {
                infer_expr(arg, functions, env);
            }
            for named in named_args.iter_mut() {
                infer_expr(&mut named.value, functions, env);
            }
            call_type(callee, &callee_ty, args, functions)
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            named_args,
        } => {
            let receiver_ty = infer_expr(receiver, functions, env);
            for arg in args.iter_mut() {
                infer_expr(arg, functions, env);
            }
            for named in named_args.iter_mut() {
                infer_expr(&mut named.value, functions, env);
            }
            method_type(&receiver_ty, method, args)
        }
        ExprKind::Index { target, index, .. } => {
            let target_ty = infer_expr(target, functions, env);
            let index_ty = infer_expr(index, functions, env);
            index_type(&target_ty, index, &index_ty)
        }
        ExprKind::Slice {
            target, start, end, ..
        } => {
            let target_ty = infer_expr(target, functions, env);
            if let Some(start) = start {
                infer_expr(start, functions, env);
            }
            if let Some(end) = end {
                infer_expr(end, functions, env);
            }
            match target_ty {
                Type::List { element, variance } => Type::List { element, variance },
                Type::Tuple(elements) => Type::List {
                    element: Box::new(join_all(elements)),
                    variance: Variance::Covariant,
                },
                Type::Str | Type::StrLit => Type::Str,
                _ => Type::UNKNOWN,
            }
        }
        ExprKind::Field { target, .. } => {
            infer_expr(target, functions, env);
            Type::UNKNOWN
        }
        ExprKind::StructLit { path, fields } => {
            for field in fields.iter_mut() {
                infer_expr(&mut field.value, functions, env);
            }
            match path.last() {
                Some(name) => Type::Named(name.clone()),
                None => Type::UNKNOWN,
            }
        }
        ExprKind::List { elements } => {
            let element = infer_elements(elements, functions, env);
            Type::List {
                element: Box::new(element),
                variance: Variance::Covariant,
            }
        }
        ExprKind::Tuple { elements } => {
            let types = elements
                .iter_mut()
                .map(|element| infer_expr(element, functions, env))
                .collect();
            Type::Tuple(types)
        }
        ExprKind::Set { elements } => {
            let element = infer_elements(elements, functions, env);
            Type::Set(Box::new(element))
        }
        ExprKind::Map { entries } => {
            let mut key: Option<Type> = None;
            let mut value: Option<Type> = None;
            for entry in entries.iter_mut() {
                let entry_key = if entry.name.is_empty() {
                    Type::UNKNOWN
                } else {
                    Type::Str
                };
                key = Some(match key {
                    Some(previous) => previous.join(&entry_key),
                    None => entry_key,
                });
                let entry_value = infer_expr(&mut entry.value, functions, env);
                value = Some(match value {
                    Some(previous) => previous.join(&entry_value),
                    None => entry_value,
                });
            }
            Type::OrderedMap {
                key: Box::new(key.unwrap_or(Type::UNKNOWN)),
                value: Box::new(value.unwrap_or(Type::UNKNOWN)),
                variance: Variance::Invariant,
            }
        }
        ExprKind::Lambda { params, body } => {
            env.push();
            for pattern in params.iter() {
                bind_pattern(pattern, &Type::UNKNOWN, env);
            }
            let ret = infer_expr(body, functions, env);
            env.pop();
            Type::Function {
                params: vec![Type::UNKNOWN; params.len()],
                ret: Box::new(ret),
                variadic: false,
                param_names: params
                    .iter()
                    .map(|pattern| pattern.names.first().cloned().unwrap_or_default())
                    .collect(),
            }
        }
        ExprKind::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            infer_expr(condition, functions, env);
            let then_ty = infer_expr(then_branch, functions, env);
            let else_ty = infer_expr(else_branch, functions, env);
            conditional_type(&then_ty, &else_ty)
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(start) = start {
                infer_expr(start, functions, env);
            }
            if let Some(end) = end {
                infer_expr(end, functions, env);
            }
            Type::List {
                element: Box::new(int()),
                variance: Variance::Covariant,
            }
        }
        ExprKind::Cast { expr, to } => {
            infer_expr(expr, functions, env);
            to.clone()
        }
        ExprKind::TypeAssert { expr, to } => {
            infer_expr(expr, functions, env);
            to.clone()
        }
        ExprKind::Await { expr } => infer_expr(expr, functions, env),
        ExprKind::Assign { value, .. } => infer_expr(value, functions, env),
        ExprKind::Comprehension {
            kind,
            element,
            key,
            value,
            clauses,
            condition,
        } => {
            env.push();
            for clause in clauses.iter_mut() {
                let iterable_ty = infer_expr(&mut clause.iterable, functions, env);
                bind_pattern(&clause.pattern, &element_type(&iterable_ty), env);
            }
            if let Some(condition) = condition {
                infer_expr(condition, functions, env);
            }
            let result = match kind {
                ComprehensionKind::List | ComprehensionKind::Generator => {
                    let element_ty = infer_expr(element, functions, env);
                    Type::List {
                        element: Box::new(element_ty),
                        variance: Variance::Covariant,
                    }
                }
                ComprehensionKind::Set => {
                    let element_ty = infer_expr(element, functions, env);
                    Type::Set(Box::new(element_ty))
                }
                ComprehensionKind::Map => {
                    let key_ty = key
                        .as_mut()
                        .map(|key| infer_expr(key, functions, env))
                        .unwrap_or(Type::UNKNOWN);
                    let value_ty = value
                        .as_mut()
                        .map(|value| infer_expr(value, functions, env))
                        .unwrap_or(Type::UNKNOWN);
                    Type::OrderedMap {
                        key: Box::new(key_ty),
                        value: Box::new(value_ty),
                        variance: Variance::Invariant,
                    }
                }
            };
            env.pop();
            result
        }
        ExprKind::Let {
            pattern,
            value,
            body,
        } => {
            let value_ty = infer_expr(value, functions, env);
            env.push();
            bind_pattern(pattern, &value_ty, env);
            let body_ty = infer_expr(body, functions, env);
            env.pop();
            body_ty
        }
        ExprKind::Unpack { expr } => {
            infer_expr(expr, functions, env);
            Type::UNKNOWN
        }
        ExprKind::Format { arguments, .. } => {
            for argument in arguments.iter_mut() {
                infer_expr(argument, functions, env);
            }
            Type::Str
        }
        ExprKind::Error(_) => Type::UNKNOWN,
    }
}

/// Infers every element and joins their types.
fn infer_elements(elements: &mut [Expr], functions: &HashMap<Name, Type>, env: &mut Env) -> Type {
    let mut element: Option<Type> = None;
    for value in elements.iter_mut() {
        let ty = infer_expr(value, functions, env);
        element = Some(match element {
            Some(previous) => previous.join(&ty),
            None => ty,
        });
    }
    element.unwrap_or(Type::UNKNOWN)
}

fn join_all(types: Vec<Type>) -> Type {
    let mut types = types.into_iter();
    let Some(first) = types.next() else {
        return Type::UNKNOWN;
    };
    types.fold(first, |acc, ty| acc.join(&ty))
}

fn literal_type(literal: &Literal) -> Type {
    match literal {
        Literal::Int(_) => int(),
        Literal::Float(_) => Type::Float,
        Literal::Str(_) => Type::Str,
        Literal::Bytes(_) => Type::Bytes,
        Literal::Char(_) => Type::Char,
        Literal::Bool(_) => Type::Bool,
        Literal::Null => Type::Null,
    }
}

fn binary_type(op: BinaryOp, left: &Type, right: &Type, lang: LangId) -> Type {
    if left.is_unknown() || right.is_unknown() {
        return Type::UNKNOWN;
    }
    match op {
        BinaryOp::Add => {
            if matches!(left, Type::Str | Type::StrLit) && matches!(right, Type::Str | Type::StrLit)
            {
                Type::Str
            } else if let (
                Type::List {
                    element: a,
                    variance,
                },
                Type::List { element: b, .. },
            ) = (left, right)
            {
                Type::List {
                    element: Box::new(a.join(b)),
                    variance: *variance,
                }
            } else {
                numeric_result(left, right)
            }
        }
        // True division differs by source language: Python's `/` always yields a
        // float, while a C-family `/` on integers yields an integer. The IR has a
        // single `/`, so inference reads the module's language to decide. Where
        // the language is not known to promote, it stays conservative.
        BinaryOp::Div => match numeric_result(left, right) {
            Type::Int(_) if lang == LangId::PYTHON => Type::Float,
            result => result,
        },
        BinaryOp::Sub | BinaryOp::Mul | BinaryOp::FloorDiv | BinaryOp::Rem | BinaryOp::Pow => {
            numeric_result(left, right)
        }
        BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight => match (left, right) {
            (Type::Int(_), Type::Int(_)) => int(),
            _ => Type::UNKNOWN,
        },
    }
}

fn numeric_result(left: &Type, right: &Type) -> Type {
    match (left, right) {
        (Type::Int(_), Type::Int(_)) => int(),
        (Type::Int(_), Type::Float) | (Type::Float, Type::Int(_)) | (Type::Float, Type::Float) => {
            Type::Float
        }
        _ => Type::UNKNOWN,
    }
}

fn conditional_type(then_ty: &Type, else_ty: &Type) -> Type {
    match (then_ty, else_ty) {
        (Type::Null, other) | (other, Type::Null) if other.is_known() && !other.is_null() => {
            Type::Option(Box::new(other.clone()))
        }
        _ => then_ty.join(else_ty),
    }
}

fn call_type(
    callee: &Expr,
    callee_ty: &Type,
    args: &[Expr],
    functions: &HashMap<Name, Type>,
) -> Type {
    if let ExprKind::Path(path) = &callee.kind
        && path.is_bare()
        && let Some(name) = path.segments.first()
    {
        if let Some(ty) = builtin_call_type(name, args) {
            return ty;
        }
        if let Some(Type::Function { ret, .. }) = functions.get(name.as_ref()) {
            return ret.as_ref().clone();
        }
    }
    if let Type::Function { ret, .. } = callee_ty {
        ret.as_ref().clone()
    } else {
        Type::UNKNOWN
    }
}

fn builtin_call_type(name: &str, args: &[Expr]) -> Option<Type> {
    let first = args.first();
    let result = match name {
        "len" | "ord" | "id" | "hash" => int(),
        "str" | "repr" | "ascii" | "format" | "input" | "chr" | "hex" | "oct" | "bin" => Type::Str,
        "int" => int(),
        "float" => Type::Float,
        "bool" => Type::Bool,
        "bytes" => Type::Bytes,
        "list" | "tuple" | "sorted" | "reversed" | "enumerate" | "zip" => Type::List {
            element: Box::new(Type::UNKNOWN),
            variance: Variance::Covariant,
        },
        "set" | "frozenset" => Type::Set(Box::new(Type::UNKNOWN)),
        "dict" => Type::OrderedMap {
            key: Box::new(Type::UNKNOWN),
            value: Box::new(Type::UNKNOWN),
            variance: Variance::Invariant,
        },
        "range" => Type::List {
            element: Box::new(int()),
            variance: Variance::Covariant,
        },
        "print" => Type::Void,
        "isinstance" | "issubclass" | "callable" | "hasattr" | "all" | "any" => Type::Bool,
        "abs" => first.map(|arg| arg.ty.clone()).unwrap_or(Type::UNKNOWN),
        "sum" => first
            .map(|arg| element_type(&arg.ty))
            .unwrap_or(Type::UNKNOWN),
        "min" | "max" => match first {
            Some(arg) if args.len() == 1 => element_type(&arg.ty),
            Some(arg) => arg.ty.join(&args[1].ty),
            None => Type::UNKNOWN,
        },
        _ => return None,
    };
    Some(result)
}

fn method_type(receiver: &Type, method: &str, _args: &[Expr]) -> Type {
    let text = matches!(receiver, Type::Str | Type::StrLit);
    let sequence = receiver_is_sequence(receiver);
    match method {
        "append" | "extend" | "insert" | "remove" | "clear" | "sort" | "reverse" | "add"
        | "discard" | "update" | "close" | "write" | "writelines" => Type::Void,
        "pop" => element_type(receiver),
        "copy" => receiver.clone(),
        "keys" => Type::List {
            element: Box::new(map_key_type(receiver)),
            variance: Variance::Covariant,
        },
        "values" => Type::List {
            element: Box::new(map_value_type(receiver)),
            variance: Variance::Covariant,
        },
        "items" => Type::List {
            element: Box::new(Type::Tuple(vec![
                map_key_type(receiver),
                map_value_type(receiver),
            ])),
            variance: Variance::Covariant,
        },
        "get" => map_value_type(receiver),
        "index" | "count" | "find" | "rfind" | "startswith" | "endswith" | "isdigit"
        | "isalpha" | "isspace" | "isnumeric" | "isupper" | "islower" | "isidentifier" => {
            if text {
                match method {
                    "startswith" | "endswith" | "isdigit" | "isalpha" | "isspace" | "isnumeric"
                    | "isupper" | "islower" | "isidentifier" => Type::Bool,
                    _ => int(),
                }
            } else if sequence && matches!(method, "index" | "count") {
                int()
            } else {
                Type::UNKNOWN
            }
        }
        "split" | "splitlines" => Type::List {
            element: Box::new(Type::Str),
            variance: Variance::Covariant,
        },
        "join" => Type::Str,
        "strip" | "lstrip" | "rstrip" | "lower" | "upper" | "title" | "capitalize" | "replace"
        | "zfill" | "format" | "casefold" | "encode" => Type::Str,
        "decode" => Type::Str,
        "union" | "intersection" | "difference" | "symmetric_difference" => {
            Type::Set(Box::new(element_type(receiver)))
        }
        "read" | "readline" => Type::Str,
        "readlines" => Type::List {
            element: Box::new(Type::Str),
            variance: Variance::Covariant,
        },
        "to_string" | "clone" => receiver.clone(),
        _ => Type::UNKNOWN,
    }
}

fn receiver_is_sequence(receiver: &Type) -> bool {
    matches!(
        receiver,
        Type::List { .. } | Type::Tuple(_) | Type::Str | Type::StrLit
    )
}

fn map_key_type(ty: &Type) -> Type {
    match ty {
        Type::Map(_) => Type::Str,
        Type::OrderedMap { key, .. } => key.as_ref().clone(),
        _ => Type::UNKNOWN,
    }
}
