//! Statements.
//!
//! A [`Stmt`] is a node in a [`Block`]. The set is deliberately a superset of
//! what any one language needs: a frontend lowers only what its source has, and
//! the rest of the variants are unreachable for that frontend.
//!
//! That superset is the point. The Go implementation had a `statement()` method
//! whose `switch` had no case for `enum`, `struct`, `trait`, `interface`, type
//! alias or `export`, so those constructs returned `nil` and vanished from the
//! output with no warning. Here, an unhandled construct is a compiler error in
//! the backend rather than a silent omission.
//!
//! # Blocks are always present
//!
//! Control-flow bodies are [`Block`]s rather than single statements. Every
//! language in scope can express a nested block, and flattening them early is
//! what makes a backend lose track of which construct owns which statement — the
//! bug recorded in `tests/baseline/` where a trailing top-level statement was
//! absorbed into the preceding function body.
//!
//! # Labels are explicit
//!
//! [`Stmt::Break`] and [`Stmt::Continue`] carry an optional label. Modelling
//! break without one is how a nested loop silently breaks the wrong loop.

use crate::expr::{Expr, Pattern};
use crate::span::Span;
use crate::types::{Name, Type};

/// A sequence of statements.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Block {
    /// The statements, in execution order.
    pub statements: Vec<Stmt>,
    /// The span covering the whole block, including its delimiters.
    pub span: Span,
}

impl Block {
    /// Creates an empty block.
    pub fn empty(span: Span) -> Self {
        Block {
            statements: Vec::new(),
            span,
        }
    }

    /// Creates a block from statements.
    pub fn new(statements: Vec<Stmt>, span: Span) -> Self {
        Block { statements, span }
    }

    /// Appends a statement.
    pub fn push(&mut self, statement: Stmt) {
        self.statements.push(statement);
    }

    /// Reports whether the block has no statements.
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    /// Returns the number of statements.
    pub fn len(&self) -> usize {
        self.statements.len()
    }

    /// Applies `f` to every statement in this block, not to nested blocks.
    pub fn walk_shallow(&mut self, f: &mut impl FnMut(&mut Stmt)) {
        for statement in &mut self.statements {
            f(statement);
        }
    }

    /// Visits every statement and expression in this block and all nested
    /// blocks, in pre-order.
    ///
    /// The tree is visited parent-first, so a rewrite that deletes a statement
    /// does not then walk into it.
    pub fn walk(&mut self, f_stmt: &mut impl FnMut(&mut Stmt), f_expr: &mut impl FnMut(&mut Expr)) {
        for statement in &mut self.statements {
            f_stmt(statement);
            statement.walk_into(f_stmt, f_expr);
        }
    }
}

/// The `else` of an `if`.
#[derive(Clone, PartialEq, Debug)]
pub enum Else {
    /// A plain `else` block.
    Block(Block),
    /// An `else if`, which is an `if` nested in the `else` position.
    ///
    /// Kept as a distinct variant rather than as an `if` with an empty `else`
    /// block, so a backend can flatten the chain correctly. An `if`/`else if`
    /// chain must not produce `} else { if ... }` in a target with no such
    /// construct.
    If(Box<Stmt>),
}

/// One arm of a `match` or `switch`.
#[derive(Clone, PartialEq, Debug)]
pub struct MatchCase {
    /// What is matched against.
    pub pattern: Pattern,
    /// An additional condition, such as Rust's `if` guard or Python's `if`.
    pub guard: Option<Expr>,
    /// The arm body.
    pub body: Block,
    /// Where the arm was written.
    pub span: Span,
}

impl MatchCase {
    /// Creates an unguarded arm.
    pub fn new(pattern: Pattern, body: Block, span: Span) -> Self {
        MatchCase {
            pattern,
            guard: None,
            body,
            span,
        }
    }

    /// Attaches a guard.
    pub fn with_guard(mut self, guard: Expr) -> Self {
        self.guard = Some(guard);
        self
    }
}

/// One `catch` clause.
#[derive(Clone, PartialEq, Debug)]
pub struct CatchClause {
    /// The bound name, if the source binds one.
    pub binding: Option<Pattern>,
    /// The exception types caught, as written in the source.
    ///
    /// Left untranslated: which exception hierarchy is available is a target
    /// question, and `gset-deps` is what resolves these to something the target
    /// has.
    pub types: Vec<Name>,
    /// The handler body.
    pub body: Block,
    /// Where the clause was written.
    pub span: Span,
}

/// One arm of an `import`.
#[derive(Clone, PartialEq, Debug)]
pub struct ImportedName {
    /// The name as the exporting module knows it.
    pub original: Name,
    /// The name it is bound to locally, if different.
    pub alias: Option<Name>,
}

impl ImportedName {
    /// Creates an unaliased imported name.
    pub fn new(original: Name) -> Self {
        ImportedName {
            original,
            alias: None,
        }
    }

    /// Binds this name to a different local name.
    pub fn as_alias(mut self, alias: Name) -> Self {
        self.alias = Some(alias);
        self
    }

    /// Returns the name actually bound locally.
    pub fn local(&self) -> &Name {
        self.alias.as_ref().unwrap_or(&self.original)
    }
}

/// How an import brings names into scope.
#[derive(Clone, PartialEq, Debug)]
pub enum ImportKind {
    /// Imports the module itself, such as Python's `import os`.
    Module,
    /// Imports specific names, such as `from os import path`.
    Named(Vec<ImportedName>),
    /// Imports the module's default export.
    Default(ImportedName),
    /// Imports everything, such as `from os import *`.
    Star,
}

/// Where a module's source actually comes from.
///
/// Present in the IR rather than computed in `gset-deps` so backends can answer
/// "can I translate this?" without depending on `gset-deps`, which sits above
/// them in the dependency graph. `gset-deps` fills this in; a frontend leaves it
/// `None`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ModuleSource {
    /// Source on disk inside the project, and therefore transpilable.
    LocalPath {
        /// Path relative to the project root.
        relative: String,
    },
    /// Pinned by a lockfile. Transpilable only if the source is fetched.
    LockedRemote {
        /// The pinned version.
        version: String,
        /// Whether source was actually obtained.
        source_available: bool,
    },
    /// Provided by the target runtime's standard library.
    ///
    /// Not transpilable, and not meant to be: it belongs to whatever the target
    /// ships.
    Sdk {
        /// The runtime that provides it, such as `python` or `jdk`.
        runtime: String,
    },
    /// A compiled artefact with no source to translate.
    ///
    /// The honest ceiling for "dependencies in other places". The import is
    /// kept and a backend emits a passthrough plus a diagnostic naming the
    /// target-side package manager that must supply it.
    OpaqueForeign {
        /// What the artefact is, such as `wheel` or `jar`.
        artifact: String,
        /// The package name to require on the target side.
        requirement: String,
    },
}

impl ModuleSource {
    /// Reports whether this module's source is available to translate.
    pub fn is_transpilable(&self) -> bool {
        match self {
            ModuleSource::LocalPath { .. } => true,
            ModuleSource::LockedRemote {
                source_available, ..
            } => *source_available,
            ModuleSource::Sdk { .. } | ModuleSource::OpaqueForeign { .. } => false,
        }
    }
}

/// An import declaration.
///
/// Imports are [`Item`]s and a backend is required to consume them. In Go,
/// `ast.Program.Imports` was written by the parser and read by no emitter, so no
/// top-level import ever reached the output.
#[derive(Clone, PartialEq, Debug)]
pub struct Import {
    /// The module path being imported.
    pub path: crate::expr::Path,
    /// How names are brought into scope.
    pub kind: ImportKind,
    /// The local alias for a whole-module import, such as `import numpy as np`.
    pub alias: Option<Name>,
    /// Where the module's source comes from, once `gset-deps` has resolved it.
    pub resolution: Option<ModuleSource>,
    /// Where the import was written.
    pub span: Span,
}

impl Import {
    /// Creates an unresolved whole-module import.
    pub fn module(path: crate::expr::Path, span: Span) -> Self {
        Import {
            path,
            kind: ImportKind::Module,
            alias: None,
            resolution: None,
            span,
        }
    }

    /// Creates an unresolved import of specific names.
    pub fn named(path: crate::expr::Path, names: Vec<ImportedName>, span: Span) -> Self {
        Import {
            path,
            kind: ImportKind::Named(names),
            alias: None,
            resolution: None,
            span,
        }
    }

    /// Attaches an alias.
    pub fn with_alias(mut self, alias: Name) -> Self {
        self.alias = Some(alias);
        self
    }

    /// Attaches a resolution result.
    pub fn with_resolution(mut self, resolution: ModuleSource) -> Self {
        self.resolution = Some(resolution);
        self
    }

    /// Reports whether the module's source is known and translatable.
    ///
    /// `None` when unresolved, which a backend must treat as "not translatable
    /// yet" rather than assuming the happy path.
    pub fn is_transpilable(&self) -> Option<bool> {
        self.resolution.as_ref().map(ModuleSource::is_transpilable)
    }
}

/// A declaration of a named constant or variable.
#[derive(Clone, PartialEq, Debug)]
pub struct VarDecl {
    /// The bound pattern, which may destructure.
    pub pattern: Pattern,
    /// The declared type, if the source states one.
    ///
    /// `None` for a dynamic declaration. A backend must not invent one here;
    /// inference in `gset-semantic` fills it in, or it stays absent and the
    /// backend picks a conservative default.
    pub ty: Option<Type>,
    /// The initialiser, if any.
    pub value: Option<Expr>,
    /// Whether the binding can be reassigned.
    pub mutable: bool,
    /// Whether the declaration is exported.
    pub exported: bool,
    /// Where the declaration was written.
    pub span: Span,
}

/// A function declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct Function {
    /// The function name.
    pub name: Name,
    /// Parameter patterns, in declaration order.
    pub params: Vec<Pattern>,
    /// Declared parameter types, where the source states them. `None` entries are
    /// parameters the source left untyped, which is most of them in Python.
    pub param_types: Vec<Option<Type>>,
    /// The return type, if the source states one.
    pub ret: Option<Type>,
    /// The body.
    pub body: Block,
    /// Whether the last parameter absorbs extra arguments.
    pub variadic: bool,
    /// Whether the function is asynchronous.
    pub is_async: bool,
    /// Whether the function is exported.
    pub exported: bool,
    /// The declared type parameters, for a generic function.
    pub generics: Vec<Name>,
    /// Where the declaration was written.
    pub span: Span,
}

impl Function {
    /// Returns the number of declared parameters.
    pub fn param_count(&self) -> usize {
        self.params.len()
    }

    /// Returns the function's signature as a [`Type::Function`].
    ///
    /// Deliberately produces [`Type::UNKNOWN`] where the source states no type,
    /// rather than a default. A caller that needs a concrete type has to run
    /// inference, which is where guessing would otherwise creep in.
    pub fn signature(&self) -> Type {
        Type::Function {
            params: self
                .param_types
                .iter()
                .map(|t| t.clone().unwrap_or(Type::UNKNOWN))
                .collect(),
            ret: Box::new(self.ret.clone().unwrap_or(Type::Void)),
            variadic: self.variadic,
            param_names: self
                .params
                .iter()
                .map(|p| p.names.first().cloned().unwrap_or_default())
                .collect(),
        }
    }
}

/// A class declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct Class {
    /// The class name.
    pub name: Name,
    /// Base class, as written.
    pub extends: Option<crate::expr::Path>,
    /// Implemented interfaces or traits, as written.
    pub implements: Vec<crate::expr::Path>,
    /// Fields.
    pub fields: Vec<VarDecl>,
    /// Methods.
    pub methods: Vec<Function>,
    /// Constructors, where the language distinguishes them.
    pub constructors: Vec<Function>,
    /// Whether the class is exported.
    pub exported: bool,
    /// Declared type parameters.
    pub generics: Vec<Name>,
    /// Where the declaration was written.
    pub span: Span,
}

/// A field of a record-like type.
#[derive(Clone, PartialEq, Debug)]
pub struct Field {
    /// The field name.
    pub name: Name,
    /// The field type, if the source states one.
    pub ty: Option<Type>,
    /// The default value, if any.
    pub default: Option<Expr>,
    /// Where the field was written.
    pub span: Span,
}

/// A record-like declaration: a struct, a record, or a data class.
#[derive(Clone, PartialEq, Debug)]
pub struct Record {
    /// The type name.
    pub name: Name,
    /// The fields, in declaration order. Order matters: a backend may need to
    /// match a positional constructor.
    pub fields: Vec<Field>,
    /// Whether the declaration is exported.
    pub exported: bool,
    /// Declared type parameters.
    pub generics: Vec<Name>,
    /// Where the declaration was written.
    pub span: Span,
}

/// One variant of an enumeration.
#[derive(Clone, PartialEq, Debug)]
pub struct Variant {
    /// The variant name.
    pub name: Name,
    /// The payload, for an enumeration with associated data.
    pub fields: Vec<Field>,
    /// The explicit value, for a C-style enumeration.
    pub value: Option<Expr>,
    /// Where the variant was written.
    pub span: Span,
}

/// An enumeration declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct Enum {
    /// The enumeration name.
    pub name: Name,
    /// The variants, in declaration order.
    pub variants: Vec<Variant>,
    /// Whether the declaration is exported.
    pub exported: bool,
    /// Where the declaration was written.
    pub span: Span,
}

/// A method of an interface or trait.
#[derive(Clone, PartialEq, Debug)]
pub struct MethodSignature {
    /// The method name.
    pub name: Name,
    /// Parameter names, in order.
    pub params: Vec<Name>,
    /// Declared parameter types.
    pub param_types: Vec<Option<Type>>,
    /// The return type, if stated.
    pub ret: Option<Type>,
    /// Whether the last parameter absorbs extra arguments.
    pub variadic: bool,
    /// Where the signature was written.
    pub span: Span,
}

/// An interface or trait declaration.
///
/// One type for both concepts. They differ in what a consumer may do with them,
/// not in their shape, and splitting them would double the IR for a difference
/// the backend resolves from its own language's conventions.
#[derive(Clone, PartialEq, Debug)]
pub struct Interface {
    /// The interface name.
    pub name: Name,
    /// Base interfaces or traits.
    pub extends: Vec<crate::expr::Path>,
    /// Required methods.
    pub methods: Vec<MethodSignature>,
    /// Whether implementations must provide these. A `default` method in a trait
    /// is optional in one sense; a target may not have the concept at all.
    pub requires_all: bool,
    /// Whether the declaration is exported.
    pub exported: bool,
    /// Where the declaration was written.
    pub span: Span,
}

/// A type alias.
#[derive(Clone, PartialEq, Debug)]
pub struct TypeAlias {
    /// The alias name.
    pub name: Name,
    /// The aliased type.
    pub target: Type,
    /// Declared type parameters.
    pub generics: Vec<Name>,
    /// Whether the alias is exported.
    pub exported: bool,
    /// Where the alias was written.
    pub span: Span,
}

/// A module-level statement.
///
/// Note what is *not* here: declarations. A binding, a function, a class and an
/// import are all [`Item`]s. That split is what lets `gset-semantic` reason about
/// scope, and it is what the Go implementation lacked when a trailing statement
/// ended up inside a function body.
#[derive(Clone, PartialEq, Debug)]
pub enum Stmt {
    /// A declaration binding.
    Decl(VarDecl),

    /// An expression evaluated for its effect.
    Expr(Expr),

    /// An empty statement, or a source `pass`.
    ///
    /// Carries a span like every other statement. A source `pass` occupies real
    /// source text, and a diagnostic about why it was emitted has to point
    /// somewhere.
    Empty {
        /// Where it was written.
        span: Span,
    },

    /// An assignment to an existing binding.
    Assign {
        /// The target.
        target: Pattern,
        /// The assigned value.
        value: Expr,
        /// Where it was written.
        span: Span,
    },

    /// A `return`.
    ///
    /// The value is absent for a bare `return`, which is not the same as
    /// `return None`.
    Return {
        /// The returned value, if any.
        value: Option<Expr>,
        /// Where it was written.
        span: Span,
    },

    /// A `break`.
    Break {
        /// The label of the loop being exited, if named.
        label: Option<Name>,
        /// Where it was written.
        span: Span,
    },

    /// A `continue`.
    Continue {
        /// The label of the loop being continued, if named.
        label: Option<Name>,
        /// Where it was written.
        span: Span,
    },

    /// An `if`.
    If {
        /// The condition.
        condition: Expr,
        /// The consequent.
        then_branch: Block,
        /// The alternative, if any.
        else_branch: Option<Box<Else>>,
        /// Where it was written.
        span: Span,
    },

    /// A `while`.
    While {
        /// The condition, evaluated before each iteration.
        condition: Expr,
        /// The body.
        body: Block,
        /// Where it was written.
        span: Span,
    },

    /// A `do`/`while`, which evaluates its body before the condition.
    DoWhile {
        /// The body.
        body: Block,
        /// The condition, evaluated after each iteration.
        condition: Expr,
        /// Where it was written.
        span: Span,
    },

    /// A C-style `for`.
    For {
        /// The initialiser, run once.
        init: Option<Box<Stmt>>,
        /// The condition, evaluated before each iteration. Absent means an
        /// infinite loop.
        condition: Option<Expr>,
        /// The update, run after each iteration.
        update: Option<Expr>,
        /// The body.
        body: Block,
        /// A label, for `break`/`continue` targeting this loop.
        label: Option<Name>,
        /// Where it was written.
        span: Span,
    },

    /// Iteration over a sequence.
    ForIn {
        /// The binding target.
        pattern: Pattern,
        /// The sequence being iterated.
        iterable: Expr,
        /// The body.
        body: Block,
        /// Whether the loop may modify the sequence while iterating.
        is_parallel: bool,
        /// A label, for `break`/`continue` targeting this loop.
        label: Option<Name>,
        /// Where it was written.
        span: Span,
    },

    /// A nested block.
    Block(Block),

    /// A `match` or `switch`.
    Switch {
        /// The value being matched, absent for a tag-style `switch`.
        scrutinee: Option<Expr>,
        /// The arms, in source order.
        cases: Vec<MatchCase>,
        /// The fallback arm.
        default: Option<Block>,
        /// Where it was written.
        span: Span,
    },

    /// A `throw` or `raise`.
    Throw {
        /// The thrown value.
        value: Expr,
        /// Where it was written.
        span: Span,
    },

    /// A `try`.
    Try {
        /// The protected block.
        body: Block,
        /// The catch clauses, in order.
        handlers: Vec<CatchClause>,
        /// The `finally` block, if any.
        finally: Option<Block>,
        /// Where it was written.
        span: Span,
    },

    /// A `with` or resource-management block.
    With {
        /// The managed resource.
        value: Expr,
        /// The body.
        body: Block,
        /// Where it was written.
        span: Span,
    },

    /// A `defer`, registering cleanup to run on scope exit.
    Defer {
        /// The deferred expression.
        expr: Expr,
        /// Where it was written.
        span: Span,
    },

    /// An assertion.
    Assert {
        /// The condition checked.
        condition: Expr,
        /// The message, if the source gives one.
        message: Option<Expr>,
        /// Where it was written.
        span: Span,
    },

    /// Deletion of a binding or element.
    Delete {
        /// The target being deleted.
        target: Expr,
        /// Where it was written.
        span: Span,
    },

    /// A `yield` or `await` in a generator position.
    Yield {
        /// The yielded value, if any.
        value: Option<Expr>,
        /// Where it was written.
        span: Span,
    },

    /// Something that could not be lowered.
    ///
    /// Always carries a type of [`Type::UNKNOWN`] where one would apply, so the
    /// failure propagates instead of being papered over.
    Error {
        /// Why lowering failed.
        reason: String,
        /// Where the unlowered construct was.
        span: Span,
    },
}

impl Stmt {
    /// Creates an error statement.
    pub fn error(reason: impl Into<String>, span: Span) -> Self {
        Stmt::Error {
            reason: reason.into(),
            span,
        }
    }

    /// Reports whether this statement is an error placeholder.
    pub fn is_error(&self) -> bool {
        matches!(self, Stmt::Error { .. })
    }

    /// Returns the span, whatever the variant.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Decl(decl) => decl.span,
            Stmt::Expr(expr) => expr.span,
            Stmt::Empty { span } => *span,
            Stmt::Assign { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span, .. }
            | Stmt::Continue { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::DoWhile { span, .. }
            | Stmt::For { span, .. }
            | Stmt::ForIn { span, .. }
            | Stmt::Switch { span, .. }
            | Stmt::Throw { span, .. }
            | Stmt::Try { span, .. }
            | Stmt::With { span, .. }
            | Stmt::Defer { span, .. }
            | Stmt::Assert { span, .. }
            | Stmt::Delete { span, .. }
            | Stmt::Yield { span, .. } => *span,
            Stmt::Block(block) => block.span,
            Stmt::Error { span, .. } => *span,
        }
    }

    /// Visits every statement and expression inside this one, pre-order.
    ///
    /// Does **not** visit the statement itself, so a rewrite can act on a node
    /// and then recurse into whatever it produced.
    ///
    /// Expressions are visited through [`Expr::walk`], which recurses into the
    /// expression tree itself. Nesting the two traversals is what lets inference
    /// treat a whole function body as one thing to walk.
    pub fn walk_into(
        &mut self,
        f_stmt: &mut impl FnMut(&mut Stmt),
        f_expr: &mut impl FnMut(&mut Expr),
    ) {
        match self {
            Stmt::Decl(decl) => {
                if let Some(value) = &mut decl.value {
                    value.walk(f_expr);
                }
            }
            Stmt::Expr(expr) => expr.walk(f_expr),
            Stmt::Empty { .. } | Stmt::Error { .. } => {}
            Stmt::Assign { value, .. } => value.walk(f_expr),
            Stmt::Return { value, .. } | Stmt::Yield { value, .. } => {
                if let Some(value) = value {
                    value.walk(f_expr);
                }
            }
            Stmt::Throw { value, .. } | Stmt::Defer { expr: value, .. } => value.walk(f_expr),
            Stmt::Delete { target, .. } => target.walk(f_expr),
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                condition.walk(f_expr);
                then_branch.walk(f_stmt, f_expr);
                if let Some(alternative) = else_branch {
                    match alternative.as_mut() {
                        Else::Block(block) => block.walk(f_stmt, f_expr),
                        Else::If(statement) => {
                            f_stmt(statement);
                            statement.walk_into(f_stmt, f_expr);
                        }
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                condition.walk(f_expr);
                body.walk(f_stmt, f_expr);
            }
            Stmt::DoWhile {
                body, condition, ..
            } => {
                body.walk(f_stmt, f_expr);
                condition.walk(f_expr);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                if let Some(init) = init {
                    f_stmt(init);
                    init.walk_into(f_stmt, f_expr);
                }
                if let Some(condition) = condition {
                    condition.walk(f_expr);
                }
                if let Some(update) = update {
                    update.walk(f_expr);
                }
                body.walk(f_stmt, f_expr);
            }
            Stmt::ForIn { iterable, body, .. } => {
                iterable.walk(f_expr);
                body.walk(f_stmt, f_expr);
            }
            Stmt::Block(block) => block.walk(f_stmt, f_expr),
            Stmt::Switch {
                scrutinee,
                cases,
                default,
                ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    scrutinee.walk(f_expr);
                }
                for case in cases {
                    if let Some(guard) = &mut case.guard {
                        guard.walk(f_expr);
                    }
                    case.body.walk(f_stmt, f_expr);
                }
                if let Some(default) = default {
                    default.walk(f_stmt, f_expr);
                }
            }
            Stmt::Try {
                body,
                handlers,
                finally,
                ..
            } => {
                body.walk(f_stmt, f_expr);
                for handler in handlers {
                    handler.body.walk(f_stmt, f_expr);
                }
                if let Some(finally) = finally {
                    finally.walk(f_stmt, f_expr);
                }
            }
            Stmt::With { value, body, .. } => {
                value.walk(f_expr);
                body.walk(f_stmt, f_expr);
            }
            Stmt::Assert {
                condition, message, ..
            } => {
                condition.walk(f_expr);
                if let Some(message) = message {
                    message.walk(f_expr);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::span::SourceId;
    use crate::types::name;

    fn span() -> Span {
        Span::point(SourceId::from_raw(0), 0)
    }

    fn int(value: &str) -> Expr {
        Expr::int(value, span())
    }

    #[test]
    fn an_empty_block_has_no_statements() {
        let block = Block::empty(span());
        assert!(block.is_empty());
        assert_eq!(block.len(), 0);
    }

    #[test]
    fn every_statement_reports_its_span() {
        // A backend that needs to attribute a diagnostic to a statement must not
        // have to match on every variant.
        let statements = vec![
            Stmt::Expr(int("1")),
            Stmt::Decl(VarDecl {
                pattern: Pattern::bind(name("x"), span()),
                ty: None,
                value: Some(int("1")),
                mutable: true,
                exported: false,
                span: span(),
            }),
            Stmt::Empty { span: span() },
            Stmt::Return {
                value: Some(int("1")),
                span: span(),
            },
            Stmt::Break {
                label: Some(name("outer")),
                span: span(),
            },
            Stmt::Continue {
                label: None,
                span: span(),
            },
            Stmt::Delete {
                target: int("1"),
                span: span(),
            },
            Stmt::error("nope", span()),
            Stmt::Block(Block::empty(span())),
        ];
        for statement in &statements {
            assert_eq!(statement.span(), span(), "variant {statement:?}");
        }
    }

    #[test]
    fn a_bare_return_is_distinct_from_returning_nothing() {
        // `return` and `return None` are different programs. The variant has to
        // be able to tell them apart or a backend will invent a value.
        let bare = Stmt::Return {
            value: None,
            span: span(),
        };
        let explicit = Stmt::Return {
            value: Some(Expr::null(span())),
            span: span(),
        };
        assert!(matches!(bare, Stmt::Return { value: None, .. }));
        assert!(matches!(explicit, Stmt::Return { value: Some(_), .. }));
    }

    #[test]
    fn a_declaration_after_a_function_is_not_absorbed_by_it() {
        // The structural bug in tests/baseline was a top-level statement ending
        // up inside the preceding function body. Decl and Function being
        // different node kinds is what makes that impossible to represent.
        let function = Function {
            name: name("pick"),
            params: vec![Pattern::bind(name("n"), span())],
            param_types: vec![None],
            ret: None,
            body: Block::new(
                vec![Stmt::Return {
                    value: Some(int("1")),
                    span: span(),
                }],
                span(),
            ),
            variadic: false,
            is_async: false,
            exported: false,
            generics: Vec::new(),
            span: span(),
        };
        // The function is an Item, so it cannot appear in a Block at all: there
        // is no Stmt variant that holds a Function. What can be checked here is
        // that a module-level declaration and a function body coexist without
        // the two being confused for one another.
        let mut module_stmts = Block::new(
            vec![
                Stmt::Decl(VarDecl {
                    pattern: Pattern::bind(name("x"), span()),
                    ty: None,
                    value: None,
                    mutable: false,
                    exported: false,
                    span: span(),
                }),
                Stmt::Block(function.body.clone()),
            ],
            span(),
        );

        // Walking the module block must reach the return statement inside the
        // function body, which only holds if nested blocks are traversed.
        let mut returns = 0;
        module_stmts.walk(&mut |_| {}, &mut |expr| {
            if matches!(expr.kind, crate::expr::ExprKind::Call { .. }) {
                returns += 1;
            }
        });
        assert_eq!(returns, 0, "the literal is not a call");
        assert_eq!(module_stmts.len(), 2);
        assert_eq!(module_stmts.statements[1].span(), function.span);
    }

    #[test]
    fn labels_are_carried_by_break_and_continue() {
        // Without a label a nested loop breaks the wrong one.
        let labelled = Stmt::Break {
            label: Some(name("outer")),
            span: span(),
        };
        match &labelled {
            Stmt::Break { label, .. } => assert_eq!(label.as_deref(), Some("outer")),
            other => panic!("expected break, got {other:?}"),
        }
    }

    #[test]
    fn loop_labels_reach_break_and_continue() {
        let loop_stmt = Stmt::For {
            init: None,
            condition: Some(int("1")),
            update: None,
            body: Block::new(
                vec![Stmt::Break {
                    label: Some(name("outer")),
                    span: span(),
                }],
                span(),
            ),
            label: Some(name("outer")),
            span: span(),
        };
        match &loop_stmt {
            Stmt::For { label, body, .. } => {
                assert_eq!(label.as_deref(), Some("outer"));
                match &body.statements[0] {
                    Stmt::Break { label, .. } => assert_eq!(label.as_deref(), Some("outer")),
                    other => panic!("expected break, got {other:?}"),
                }
            }
            other => panic!("expected for, got {other:?}"),
        }
    }

    #[test]
    fn walk_reaches_nested_statements() {
        let mut block = Block::new(
            vec![
                Stmt::If {
                    condition: int("1"),
                    then_branch: Block::new(
                        vec![Stmt::Expr(int("2")), Stmt::Expr(int("3"))],
                        span(),
                    ),
                    else_branch: Some(Box::new(Else::Block(Block::new(
                        vec![Stmt::Expr(int("4"))],
                        span(),
                    )))),
                    span: span(),
                },
                Stmt::While {
                    condition: int("5"),
                    body: Block::new(vec![Stmt::Expr(int("6"))], span()),
                    span: span(),
                },
            ],
            span(),
        );

        let mut visited = 0;
        block.walk(&mut |_| visited += 1, &mut |_| {});
        // The two top-level statements, the two in the if body, the one in the
        // else block, and the one in the while body.
        assert_eq!(visited, 6);
    }

    #[test]
    fn walk_visits_expressions_parent_before_children() {
        // An `if` has to be seen before its condition and body, otherwise a
        // rewrite that widens a variable's type would see the body first and
        // bake in the old type.
        let mut block = Block::new(
            vec![Stmt::If {
                condition: int("1"),
                then_branch: Block::new(vec![Stmt::Expr(int("2"))], span()),
                else_branch: None,
                span: span(),
            }],
            span(),
        );

        // Both closures need to append to the same log, so share it.
        let log = std::cell::RefCell::new(Vec::new());
        block.walk(&mut |_| log.borrow_mut().push("stmt"), &mut |_| {
            log.borrow_mut().push("expr")
        });

        // The if comes first, then its condition, then the statement in its
        // body, then that statement's expression.
        assert_eq!(
            *log.borrow(),
            ["stmt", "expr", "stmt", "expr"],
            "expected pre-order statement/expression interleaving"
        );
    }

    #[test]
    fn walk_does_not_descend_into_a_removed_statement() {
        // The rewrite contract: a pass that deletes a statement must not then
        // walk into it, or it would rewrite nodes that are about to be dropped.
        let mut block = Block::new(
            vec![Stmt::If {
                condition: int("1"),
                then_branch: Block::new(vec![Stmt::Expr(int("2"))], span()),
                else_branch: None,
                span: span(),
            }],
            span(),
        );

        block.walk(
            &mut |statement| {
                if matches!(statement, Stmt::If { .. }) {
                    *statement = Stmt::Empty { span: span() };
                }
            },
            &mut |_| {},
        );
        assert_eq!(
            block.statements.len(),
            1,
            "the statement was replaced, not removed"
        );
        assert!(
            matches!(block.statements[0], Stmt::Empty { .. }),
            "the replacement must be in place"
        );
    }

    #[test]
    fn walk_follows_an_else_if_chain() {
        let mut chain = Block::new(
            vec![Stmt::If {
                condition: int("1"),
                then_branch: Block::empty(span()),
                else_branch: Some(Box::new(Else::If(Box::new(Stmt::If {
                    condition: int("2"),
                    then_branch: Block::empty(span()),
                    else_branch: Some(Box::new(Else::Block(Block::new(
                        vec![Stmt::Expr(int("3"))],
                        span(),
                    )))),
                    span: span(),
                })))),
                span: span(),
            }],
            span(),
        );

        let mut expressions = 0;
        chain.walk(&mut |_| {}, &mut |_| expressions += 1);
        // Both conditions plus the statement in the final else block.
        assert_eq!(expressions, 3);
    }

    #[test]
    fn walk_shallow_does_not_descend() {
        let mut block = Block::new(
            vec![Stmt::Block(Block::new(vec![Stmt::Expr(int("1"))], span()))],
            span(),
        );
        let mut visited = 0;
        block.walk_shallow(&mut |_| visited += 1);
        assert_eq!(visited, 1);
    }

    #[test]
    fn else_if_is_distinct_from_a_nested_if() {
        // An if/else-if chain must not emit } else { if ... } in a target that
        // has no such construct.
        let chain = Stmt::If {
            condition: int("1"),
            then_branch: Block::empty(span()),
            else_branch: Some(Box::new(Else::If(Box::new(Stmt::If {
                condition: int("2"),
                then_branch: Block::empty(span()),
                else_branch: None,
                span: span(),
            })))),
            span: span(),
        };
        match &chain {
            Stmt::If { else_branch, .. } => {
                assert!(matches!(else_branch.as_deref(), Some(Else::If(_))));
            }
            other => panic!("expected if, got {other:?}"),
        }
    }

    #[test]
    fn imports_know_whether_they_are_transpilable() {
        let unresolved = Import::module(crate::expr::Path::single(name("os")), span());
        // Unresolved is not "probably fine". A backend must not assume the
        // happy path for something nobody has looked at.
        assert_eq!(unresolved.is_transpilable(), None);

        let local = unresolved.clone().with_resolution(ModuleSource::LocalPath {
            relative: "src/os.py".into(),
        });
        assert_eq!(local.is_transpilable(), Some(true));

        let remote = unresolved
            .clone()
            .with_resolution(ModuleSource::LockedRemote {
                version: "1.2.3".into(),
                source_available: false,
            });
        assert_eq!(remote.is_transpilable(), Some(false));

        let sdk = unresolved.clone().with_resolution(ModuleSource::Sdk {
            runtime: "python".into(),
        });
        assert_eq!(sdk.is_transpilable(), Some(false));

        let opaque = unresolved.with_resolution(ModuleSource::OpaqueForeign {
            artifact: "wheel".into(),
            requirement: "numpy".into(),
        });
        assert_eq!(opaque.is_transpilable(), Some(false));
    }

    #[test]
    fn sdk_and_opaque_are_but_not_transpilable_for_different_reasons() {
        // Both answer false, but a backend must not treat them the same way: one
        // is the target's own runtime, the other needs a package manager.
        let sdk = ModuleSource::Sdk {
            runtime: "python".into(),
        };
        let opaque = ModuleSource::OpaqueForeign {
            artifact: "wheel".into(),
            requirement: "numpy".into(),
        };
        assert!(!sdk.is_transpilable());
        assert!(!opaque.is_transpilable());
        assert!(matches!(sdk, ModuleSource::Sdk { .. }));
        assert!(matches!(opaque, ModuleSource::OpaqueForeign { .. }));
    }

    #[test]
    fn an_imported_name_knows_its_local_binding() {
        let plain = ImportedName::new(name("join"));
        assert_eq!(&**plain.local(), "join");

        let aliased = ImportedName::new(name("join")).as_alias(name("j"));
        assert_eq!(&**aliased.local(), "j");
        assert_eq!(&*aliased.original, "join");
    }

    #[test]
    fn function_signature_keeps_unknown_where_the_source_is_silent() {
        // Python states no parameter or return types, so the signature is mostly
        // Unknown. Filling in a default here would be guessing before inference
        // has even run.
        let function = Function {
            name: name("add"),
            params: vec![
                Pattern::bind(name("a"), span()),
                Pattern::bind(name("b"), span()),
            ],
            param_types: vec![None, None],
            ret: None,
            body: Block::empty(span()),
            variadic: false,
            is_async: false,
            exported: false,
            generics: Vec::new(),
            span: span(),
        };
        match function.signature() {
            Type::Function { params, ret, .. } => {
                assert_eq!(params.len(), 2);
                assert!(params.iter().all(Type::is_unknown));
                // Absent return type means "returns nothing", not "unknown".
                // infer() refines this; signature() must not pretend to know.
                assert_eq!(*ret, Type::Void);
            }
            other => panic!("expected function type, got {other:?}"),
        }
    }

    #[test]
    fn function_signature_uses_declared_types_when_present() {
        let function = Function {
            name: name("add"),
            params: vec![Pattern::bind(name("a"), span())],
            param_types: vec![Some(Type::Bool)],
            ret: Some(Type::Bool),
            body: Block::empty(span()),
            variadic: false,
            is_async: false,
            exported: false,
            generics: Vec::new(),
            span: span(),
        };
        match function.signature() {
            Type::Function { params, ret, .. } => {
                assert_eq!(params, vec![Type::Bool]);
                assert_eq!(*ret, Type::Bool);
            }
            other => panic!("expected function type, got {other:?}"),
        }
    }

    #[test]
    fn a_match_case_can_carry_a_guard() {
        let plain = MatchCase::new(
            Pattern::bind(name("x"), span()),
            Block::empty(span()),
            span(),
        );
        assert!(plain.guard.is_none());

        let guarded = plain.with_guard(int("1"));
        assert!(guarded.guard.is_some());
    }

    #[test]
    fn interface_and_trait_share_one_node() {
        // One type for both. They differ in what a consumer may do with them,
        // not in shape.
        let contract = Interface {
            name: name("Comparable"),
            extends: Vec::new(),
            methods: vec![MethodSignature {
                name: name("compare"),
                params: vec![name("other")],
                param_types: vec![None],
                ret: None,
                variadic: false,
                span: span(),
            }],
            requires_all: true,
            exported: true,
            span: span(),
        };
        assert_eq!(contract.methods.len(), 1);
        assert!(contract.requires_all);
    }

    #[test]
    fn error_statements_are_identifiable() {
        let bad = Stmt::error("unlowered", span());
        assert!(bad.is_error());
        assert!(!Stmt::Empty { span: span() }.is_error());
        assert!(!Stmt::Expr(int("1")).is_error());
    }
}
