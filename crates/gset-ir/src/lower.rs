//! Validated construction of a [`Module`] from a frontend.
//!
//! A tree-sitter grammar validates syntax and nothing else. It will happily
//! hand back a `return` at module scope or a `break` outside any loop, and it
//! cannot tell a frontend that it forgot to record an import. Those are the
//! mistakes this module exists to catch, in one place, before a backend is
//! handed a tree it will faithfully render into the wrong language.
//!
//! The builder never panics and never silently repairs. Problems become
//! [`Diagnostic`]s, which is what lets a frontend keep going after one bad
//! statement instead of aborting the whole file.
//!
//! ```
//! use gset_ir::{Builder, Item, LangId, Span, Stmt};
//!
//! let span = Span::synthetic();
//! let mut builder = Builder::new(LangId::PYTHON, span);
//!
//! // `break` at module scope is an error, not something to pass through.
//! builder.stmt(Stmt::Break { label: None, span });
//!
//! let result = builder.finish();
//! assert!(result.diagnostics.has_errors());
//! assert!(result.module.is_none());
//! ```

use std::collections::HashSet;

use crate::diagnostic::DiagnosticBag;
use crate::expr::Expr;
use crate::item::{Item, LangId, Module};
use crate::span::Span;
use crate::stmt::{Block, Stmt, VarDecl};
use crate::types::Name;

/// What kind of control flow a statement sits inside.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Flow {
    /// How many enclosing loops a `break` or `continue` could target.
    loops: u32,
    /// Whether the statement is inside a function body.
    ///
    /// A bare `return` at module scope is an error; a `return` inside a lambda
    /// is not, so this cannot be collapsed into a boolean at the top level.
    in_function: bool,
}

impl Flow {
    /// Module scope: no enclosing loop, and no enclosing function.
    const MODULE: Flow = Flow {
        loops: 0,
        in_function: false,
    };

    /// Whether the statement may return.
    fn may_return(self) -> bool {
        self.in_function
    }

    /// Whether the statement may break or continue.
    fn may_break(self) -> bool {
        self.loops > 0
    }
}

/// A module plus whatever went wrong building it.
#[derive(Clone, Debug)]
pub struct Lowered {
    /// The module, present only if nothing failed.
    ///
    /// Returning `None` on error is deliberate: a backend that runs on a broken
    /// module emits confidently wrong code, which is the failure mode this
    /// whole rewrite is trying to eliminate.
    pub module: Option<Module>,
    /// Everything that went wrong, in discovery order.
    pub diagnostics: DiagnosticBag,
}

/// Builds and validates a [`Module`].
///
/// Validation runs on insertion rather than at [`Builder::finish`], so the
/// builder has the enclosing loop and function context in hand when it needs to
/// judge a `break`.
#[derive(Clone, Debug)]
pub struct Builder {
    lang: LangId,
    span: Span,
    items: Vec<Item>,
    /// Names already taken at module scope.
    ///
    /// Tracked as plain strings because the diagnostic only needs the name, and
    /// a `HashSet<Arc<str>>` would hash the same bytes for no gain.
    declared: HashSet<String>,
    diagnostics: DiagnosticBag,
    /// Expressions seen with no inferred type.
    untyped: usize,
}

impl Builder {
    /// Starts a module for `lang`.
    pub fn new(lang: LangId, span: Span) -> Builder {
        Builder {
            lang,
            span,
            items: Vec::new(),
            declared: HashSet::new(),
            diagnostics: DiagnosticBag::new(),
            untyped: 0,
        }
    }

    /// Treats later warnings as errors.
    ///
    /// Warnings here are advisory, mostly about shadowing and unreachable code,
    /// so a caller that wants them to stop a build has to ask for it.
    pub fn strict(mut self, strict: bool) -> Builder {
        self.diagnostics.set_warnings_as_errors(strict);
        self
    }

    /// Records a diagnostic raised by the frontend rather than by a check here.
    pub fn report(&mut self, diagnostic: crate::diagnostic::Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Whether anything has failed so far.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }

    /// Adds a module-level declaration.
    ///
    /// Validates that it has a name, and that the name is not already taken.
    pub fn item(&mut self, item: Item) {
        let span = item.span();
        match &item {
            Item::Import(import) => self.check_import(import, span),
            Item::Function(function) => {
                self.declare(&function.name, span, "function");
                self.check_function(function, span);
            }
            Item::Class(class) => {
                self.declare(&class.name, span, "class");
                for field in &class.fields {
                    let mut seen = HashSet::new();
                    for bound in field.pattern.bound_names() {
                        if !seen.insert(bound.to_string()) {
                            self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                                field.pattern.span,
                                format!(
                                    "field `{}` of class `{}` is declared twice",
                                    bound, class.name
                                ),
                            ));
                        }
                    }
                }
                for method in class.methods.iter().chain(class.constructors.iter()) {
                    self.check_function(method, method.span);
                }
            }
            Item::Record(record) => {
                self.declare(&record.name, span, "record");
            }
            Item::Enum(enumeration) => {
                self.declare(&enumeration.name, span, "enum");
            }
            Item::Interface(interface) => {
                self.declare(&interface.name, span, "interface");
            }
            Item::TypeAlias(alias) => {
                self.declare(&alias.name, span, "type alias");
            }
            Item::Global(global) => {
                self.check_global(global);
            }
            Item::Stmt(statement) => {
                // Valid against module scope, not against a function body.
                self.check_stmt(statement, Flow::MODULE);
            }
        }
        self.items.push(item);
    }

    /// Adds a statement at module scope.
    pub fn stmt(&mut self, statement: Stmt) {
        self.check_stmt(&statement, Flow::MODULE);
        self.items.push(Item::Stmt(statement));
    }

    /// Adds a module-level binding.
    pub fn global(&mut self, global: VarDecl) {
        self.item(Item::Global(global));
    }

    /// Adds an item, or records a diagnostic if `item` is `Err`.
    ///
    /// This is the shape a frontend wants: tree-sitter lowering returns
    /// `Result`s and the recovery path is to keep the diagnostic.
    pub fn item_from(&mut self, result: Result<Item, crate::diagnostic::Diagnostic>) {
        match result {
            Ok(item) => self.item(item),
            Err(diagnostic) => self.diagnostics.push(diagnostic),
        }
    }

    /// Finishes the module.
    ///
    /// Yields `None` for the module if anything was an error, since a partial
    /// module would let a backend emit code that is wrong in ways no later
    /// stage can detect.
    pub fn finish(mut self) -> Lowered {
        if self.untyped > 0 {
            let count = self.untyped;
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::warning(
                    self.span,
                    format!(
                        "{count} expression(s) have no inferred type; backends will pick a default"
                    ),
                )
                .with_code("gset-untyped"),
            );
        }
        let module = if self.diagnostics.has_errors() {
            None
        } else {
            Some(Module {
                items: self.items,
                lang: self.lang,
                span: self.span,
            })
        };
        Lowered {
            module,
            diagnostics: self.diagnostics,
        }
    }

    /// Claims `name` at module scope, reporting a collision if it is taken.
    ///
    /// Shadowing is only a warning because Python and JavaScript both allow it
    /// deliberately. A strict build turns it into an error.
    fn declare(&mut self, name: &Name, span: Span, kind: &str) {
        if name.is_empty() {
            self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                span,
                format!("{kind} declaration has an empty name"),
            ));
            return;
        }
        if !self.declared.insert(name.to_string()) {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::warning(
                    span,
                    format!("{kind} `{name}` shadows an earlier declaration"),
                )
                .with_code("gset-redefinition"),
            );
        }
    }

    /// Checks a module-level binding.
    ///
    /// A destructuring pattern declares several names, so each is claimed. A
    /// hole such as `a, _ = pair()` is deliberately allowed: it is ordinary in
    /// Python and every target language has a natural spelling for it. Only a
    /// name bound twice by the *same* pattern is rejected, because no target
    /// can honour it.
    fn check_global(&mut self, global: &VarDecl) {
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        for name in global.pattern.bound_names() {
            if !seen.insert(name.to_string()) {
                // Reported once. Claiming the repeat as well would add a
                // shadowing warning for the same mistake.
                self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                    global.pattern.span,
                    format!("`{name}` is bound more than once by one pattern"),
                ));
                continue;
            }
            unique.push(name);
        }
        for name in &unique {
            self.declare(name, global.pattern.span, "variable");
        }
        if let Some(value) = &global.value {
            self.check_expr(value);
        }
    }

    /// Checks an import.
    fn check_import(&mut self, import: &crate::item::Import, span: Span) {
        if import.path.segments.is_empty() {
            self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                span,
                "import has an empty path",
            ));
        }
        match &import.kind {
            crate::item::ImportKind::Named(names) if names.is_empty() => {
                self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                    span,
                    "named import lists no names",
                ));
            }
            crate::item::ImportKind::Named(names) => {
                for imported in names {
                    if imported.local().is_empty() {
                        self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                            span,
                            "imported name is empty",
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    /// Checks a function's signature and body.
    fn check_function(&mut self, function: &crate::item::Function, span: Span) {
        if function.params.len() != function.param_types.len() {
            // Not fatal: a frontend may legitimately leave a type undeliberated.
            self.diagnostics
                .push(crate::diagnostic::Diagnostic::warning(
                    span,
                    format!(
                        "function `{}` has {} parameters but {} type annotations",
                        function.name,
                        function.params.len(),
                        function.param_types.len()
                    ),
                ));
        }
        if function.params.len() != function.body.statements.len()
            && function.param_types.iter().all(Option::is_none)
            && function.variadic
        {
            // A variadic function may legitimately have a body whose statement
            // count differs; nothing to check.
        }

        let mut seen = HashSet::new();
        for param in &function.params {
            for bound in param.bound_names() {
                if !seen.insert(bound.to_string()) {
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        param.span,
                        format!("parameter `{bound}` is bound more than once"),
                    ));
                }
            }
        }

        self.check_block(
            &function.body,
            Flow {
                in_function: true,
                ..Flow::MODULE
            },
        );
    }

    /// Checks every statement in `block` under `flow`.
    fn check_block(&mut self, block: &Block, flow: Flow) {
        for statement in &block.statements {
            self.check_stmt(statement, flow);
        }
        // Unreachable statements are worth reporting but must not stop a build.
        if let Some(index) = first_unreachable(block) {
            let statement = &block.statements[index];
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::warning(statement.span(), "unreachable code")
                    .with_code("gset-unreachable"),
            );
        }
    }

    /// Checks a single statement under `flow`.
    fn check_stmt(&mut self, statement: &Stmt, flow: Flow) {
        match statement {
            Stmt::Break { label, span } => {
                if !flow.may_break() {
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        *span,
                        match label {
                            Some(label) => format!("`break {label}` is not inside a loop"),
                            None => "`break` is not inside a loop".to_string(),
                        },
                    ));
                }
            }
            Stmt::Continue { label, span } => {
                if !flow.may_break() {
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        *span,
                        match label {
                            Some(label) => format!("`continue {label}` is not inside a loop"),
                            None => "`continue` is not inside a loop".to_string(),
                        },
                    ));
                }
            }
            Stmt::Return { value, span } => {
                if !flow.may_return() {
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        *span,
                        "`return` is not inside a function",
                    ));
                }
                if let Some(value) = value {
                    self.check_expr(value);
                }
            }
            Stmt::Throw { value, .. }
            | Stmt::Defer { expr: value, .. }
            | Stmt::Delete { target: value, .. }
            | Stmt::Expr(value)
            | Stmt::Assign { value, .. } => self.check_expr(value),
            Stmt::Decl(decl) => {
                if let Some(value) = &decl.value {
                    self.check_expr(value);
                }
            }
            Stmt::Yield { value, .. } => {
                if let Some(value) = value {
                    self.check_expr(value);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expr(condition);
                // The condition narrows types inside the branches but does not
                // create a loop, so the loop count is unchanged.
                self.check_block(then_branch, flow);
                if let Some(alternative) = else_branch {
                    match alternative.as_ref() {
                        crate::stmt::Else::Block(block) => {
                            self.check_block(block, flow);
                        }
                        crate::stmt::Else::If(nested) => self.check_stmt(nested, flow),
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                body, condition, ..
            } => {
                self.check_expr(condition);
                let inner = Flow {
                    loops: flow.loops + 1,
                    ..flow
                };
                self.check_block(body, inner);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                if let Some(init) = init {
                    self.check_stmt(init, flow);
                }
                if let Some(condition) = condition {
                    self.check_expr(condition);
                }
                if let Some(update) = update {
                    self.check_expr(update);
                }
                let inner = Flow {
                    loops: flow.loops + 1,
                    ..flow
                };
                self.check_block(body, inner);
            }
            Stmt::ForIn { iterable, body, .. } => {
                self.check_expr(iterable);
                let inner = Flow {
                    loops: flow.loops + 1,
                    ..flow
                };
                self.check_block(body, inner);
            }
            Stmt::Switch {
                scrutinee,
                cases,
                default,
                ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    self.check_expr(scrutinee);
                }
                // A switch case is not a loop: `break` inside one exits the
                // switch, which every target language spells differently, so
                // backends must decide rather than inherit a guess here.
                for case in cases {
                    if let Some(guard) = &case.guard {
                        self.check_expr(guard);
                    }
                    self.check_block(&case.body, flow);
                }
                if let Some(default) = default {
                    self.check_block(default, flow);
                }
            }
            Stmt::Try {
                body,
                handlers,
                finally,
                ..
            } => {
                self.check_block(body, flow);
                for handler in handlers {
                    self.check_block(&handler.body, flow);
                }
                if let Some(finally) = finally {
                    self.check_block(finally, flow);
                }
            }
            Stmt::With { value, body, .. } => {
                self.check_expr(value);
                self.check_block(body, flow);
            }
            Stmt::Assert {
                condition, message, ..
            } => {
                self.check_expr(condition);
                if let Some(message) = message {
                    self.check_expr(message);
                }
            }
            Stmt::Block(block) => self.check_block(block, flow),
            Stmt::Empty { .. } | Stmt::Error { .. } => {}
        }
    }

    /// Notes that `expr` has no inferred type yet.
    ///
    /// Counted rather than reported per node. A frontend lowering a dynamic
    /// language leaves [`Type::Unknown`] on nearly every expression, so a
    /// warning each would bury the diagnostics that matter.
    fn check_expr(&mut self, expr: &Expr) {
        if expr.ty.is_unknown() {
            self.untyped += 1;
        }
    }
}

/// The index of the first statement that can never run, if there is one.
///
/// Only the terminators every source language agrees on count: `return`,
/// `throw`, `break`, and `continue`. A `raise` that some languages spell
/// `panic` and others `exit` is still a terminator, but the frontend has to say
/// which it meant rather than leaving it to a guess.
fn first_unreachable(block: &Block) -> Option<usize> {
    let terminator = block.statements.iter().position(|statement| {
        matches!(
            statement,
            Stmt::Return { .. } | Stmt::Throw { .. } | Stmt::Break { .. } | Stmt::Continue { .. }
        )
    })?;
    (terminator + 1 < block.statements.len()).then_some(terminator + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{Expr, Path, Pattern};
    use crate::item::{Class, Function, Import, Record};
    use crate::span::SourceId;
    use crate::types::{Type, name};

    fn span() -> Span {
        Span::point(SourceId::from_raw(0), 0)
    }

    fn int(value: &str) -> Expr {
        Expr::int(value, span())
    }

    fn function(name_text: &str, body: Block) -> Function {
        Function {
            name: name(name_text),
            params: vec![],
            param_types: vec![],
            ret: None,
            body,
            variadic: false,
            is_async: false,
            exported: false,
            generics: Vec::new(),
            span: span(),
        }
    }

    fn messages(bag: &DiagnosticBag) -> Vec<String> {
        bag.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn an_empty_module_lowers_cleanly() {
        let result = Builder::new(LangId::PYTHON, span()).finish();
        assert!(result.diagnostics.is_empty());
        assert!(result.module.is_some());
    }

    #[test]
    fn break_outside_a_loop_is_an_error() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.stmt(Stmt::Break {
            label: None,
            span: span(),
        });
        let result = builder.finish();
        assert!(result.module.is_none());
        assert_eq!(
            messages(&result.diagnostics),
            ["`break` is not inside a loop"]
        );
    }

    #[test]
    fn break_inside_a_loop_is_accepted() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::While {
                    condition: int("1"),
                    body: Block::new(
                        vec![Stmt::Break {
                            label: None,
                            span: span(),
                        }],
                        span(),
                    ),
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(builder.finish().module.is_some());
    }

    #[test]
    fn continue_outside_a_loop_names_its_label() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.stmt(Stmt::Continue {
            label: Some(name("outer")),
            span: span(),
        });
        let result = builder.finish();
        assert_eq!(
            messages(&result.diagnostics),
            ["`continue outer` is not inside a loop"]
        );
    }

    #[test]
    fn return_at_module_scope_is_an_error_but_inside_a_function_is_not() {
        let mut outside = Builder::new(LangId::PYTHON, span());
        outside.stmt(Stmt::Return {
            value: None,
            span: span(),
        });
        assert_eq!(
            messages(&outside.finish().diagnostics),
            ["`return` is not inside a function"]
        );

        let mut inside = Builder::new(LangId::PYTHON, span());
        inside.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::Return {
                    value: None,
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(inside.finish().module.is_some());
    }

    #[test]
    fn a_loop_inside_a_function_allows_both() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::For {
                    init: None,
                    condition: None,
                    update: None,
                    body: Block::new(
                        vec![
                            Stmt::Break {
                                label: None,
                                span: span(),
                            },
                            Stmt::Return {
                                value: None,
                                span: span(),
                            },
                        ],
                        span(),
                    ),
                    label: None,
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(builder.finish().module.is_some());
    }

    #[test]
    fn loop_depth_nests() {
        // A `break` inside an `if` inside a `while` is still inside a loop.
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::While {
                    condition: int("1"),
                    body: Block::new(
                        vec![Stmt::If {
                            condition: int("1"),
                            then_branch: Block::new(
                                vec![Stmt::Break {
                                    label: None,
                                    span: span(),
                                }],
                                span(),
                            ),
                            else_branch: None,
                            span: span(),
                        }],
                        span(),
                    ),
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(builder.finish().module.is_some());
    }

    #[test]
    fn a_switch_case_is_not_a_loop() {
        // `break` inside a switch exits the switch, so it must not be excused
        // by the case body. Backends spell that differently in every language.
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::Switch {
                    scrutinee: Some(int("1")),
                    cases: vec![crate::stmt::MatchCase {
                        pattern: Pattern::bind(name("x"), span()),
                        guard: None,
                        body: Block::new(
                            vec![Stmt::Break {
                                label: None,
                                span: span(),
                            }],
                            span(),
                        ),
                        span: span(),
                    }],
                    default: None,
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(builder.finish().module.is_none());
    }

    #[test]
    fn an_else_if_branch_keeps_the_enclosing_loop() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::While {
                    condition: int("1"),
                    body: Block::new(
                        vec![Stmt::If {
                            condition: int("1"),
                            then_branch: Block::empty(span()),
                            else_branch: Some(Box::new(crate::stmt::Else::If(Box::new(
                                Stmt::Break {
                                    label: None,
                                    span: span(),
                                },
                            )))),
                            span: span(),
                        }],
                        span(),
                    ),
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(builder.finish().module.is_some());
    }

    #[test]
    fn a_duplicate_declaration_is_a_warning_unless_strict() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function("f", Block::empty(span()))));
        builder.item(Item::Function(function("f", Block::empty(span()))));
        let lenient = builder.finish();
        assert!(lenient.module.is_some(), "shadowing is legal in Python");
        assert_eq!(lenient.diagnostics.len(), 1);
        assert_eq!(
            lenient.diagnostics.iter().next().unwrap().code,
            Some("gset-redefinition")
        );

        let mut strict = Builder::new(LangId::PYTHON, span()).strict(true);
        strict.item(Item::Function(function("f", Block::empty(span()))));
        strict.item(Item::Function(function("f", Block::empty(span()))));
        assert!(strict.finish().module.is_none());
    }

    #[test]
    fn a_class_and_a_function_of_the_same_name_collide() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function("Thing", Block::empty(span()))));
        builder.item(Item::Class(Class {
            name: name("Thing"),
            extends: None,
            implements: vec![],
            fields: vec![],
            methods: vec![],
            constructors: vec![],
            generics: vec![],
            exported: false,
            span: span(),
        }));
        assert_eq!(builder.finish().diagnostics.len(), 1);
    }

    #[test]
    fn a_destructuring_global_claims_every_name_it_binds() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.global(VarDecl {
            pattern: Pattern::sequence(
                vec![
                    Pattern::bind(name("a"), span()),
                    Pattern::bind(name("b"), span()),
                ],
                span(),
            ),
            ty: None,
            value: None,
            mutable: false,
            exported: false,
            span: span(),
        });
        builder.global(VarDecl {
            pattern: Pattern::bind(name("b"), span()),
            ty: None,
            value: None,
            mutable: false,
            exported: false,
            span: span(),
        });
        let result = builder.finish();
        assert_eq!(result.diagnostics.len(), 1);
        assert!(
            result
                .diagnostics
                .iter()
                .next()
                .unwrap()
                .message
                .contains("`b`")
        );
    }

    #[test]
    fn a_hole_in_a_destructuring_pattern_is_allowed() {
        // `_, _ = f()` is ordinary Python and every target language has a
        // natural spelling for it. Rejecting it would reject correct source.
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.global(VarDecl {
            pattern: Pattern::sequence(
                vec![Pattern::bind(name("a"), span()), Pattern::ignore(span())],
                span(),
            ),
            ty: None,
            value: None,
            mutable: false,
            exported: false,
            span: span(),
        });
        assert!(builder.finish().module.is_some());
    }

    #[test]
    fn one_pattern_binding_a_name_twice_is_rejected() {
        // No target language can honour `a, a = f()`, so this is an error
        // rather than a shadowing.
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.global(VarDecl {
            pattern: Pattern::sequence(
                vec![
                    Pattern::bind(name("a"), span()),
                    Pattern::bind(name("a"), span()),
                ],
                span(),
            ),
            ty: None,
            value: None,
            mutable: false,
            exported: false,
            span: span(),
        });
        let result = builder.finish();
        assert!(result.module.is_none());
        assert_eq!(
            messages(&result.diagnostics),
            ["`a` is bound more than once by one pattern"]
        );
    }

    #[test]
    fn an_empty_import_path_is_rejected() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Import(Import::module(Path::new(vec![]), span())));
        assert_eq!(
            messages(&builder.finish().diagnostics),
            ["import has an empty path"]
        );
    }

    #[test]
    fn unreachable_code_is_a_warning_not_an_error() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![
                    Stmt::Return {
                        value: None,
                        span: span(),
                    },
                    Stmt::Expr(int("1")),
                ],
                span(),
            ),
        )));
        let result = builder.finish();
        assert!(result.module.is_some(), "unreachable code is legal");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some("gset-unreachable")),
            "expected an unreachable-code warning, got {:?}",
            messages(&result.diagnostics)
        );
    }

    #[test]
    fn a_return_at_the_end_is_not_unreachable() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function(
            "f",
            Block::new(
                vec![Stmt::Return {
                    value: None,
                    span: span(),
                }],
                span(),
            ),
        )));
        assert!(builder.finish().diagnostics.is_empty());
    }

    #[test]
    fn one_error_suppresses_the_module_but_keeps_every_diagnostic() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.stmt(Stmt::Break {
            label: None,
            span: span(),
        });
        builder.stmt(Stmt::Continue {
            label: None,
            span: span(),
        });
        let result = builder.finish();
        assert!(result.module.is_none());
        assert_eq!(result.diagnostics.error_count(), 2);
    }

    #[test]
    fn a_record_declares_its_name() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Record(Record {
            name: name("Point"),
            fields: vec![],
            generics: vec![],
            exported: false,
            span: span(),
        }));
        builder.item(Item::Record(Record {
            name: name("Point"),
            fields: vec![],
            generics: vec![],
            exported: false,
            span: span(),
        }));
        assert_eq!(builder.finish().diagnostics.len(), 1);
    }

    #[test]
    fn an_empty_function_name_is_an_error() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.item(Item::Function(function("", Block::empty(span()))));
        assert_eq!(
            messages(&builder.finish().diagnostics),
            ["function declaration has an empty name"]
        );
    }

    #[test]
    fn a_parameter_bound_twice_is_an_error() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        let mut duplicate = function("f", Block::empty(span()));
        duplicate.params = vec![
            Pattern::bind(name("x"), span()),
            Pattern::bind(name("x"), span()),
        ];
        duplicate.param_types = vec![None, None];
        builder.item(Item::Function(duplicate));
        let result = builder.finish();
        assert!(result.module.is_none());
        assert_eq!(
            messages(&result.diagnostics),
            ["parameter `x` is bound more than once"]
        );
    }

    #[test]
    fn a_mismatched_parameter_and_type_count_is_a_warning() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        let mut mismatched = function("f", Block::empty(span()));
        mismatched.params = vec![Pattern::bind(name("x"), span())];
        mismatched.param_types = vec![];
        builder.item(Item::Function(mismatched));
        let result = builder.finish();
        assert!(result.module.is_some());
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn unknown_types_are_summarised_once_not_reported_per_node() {
        // A dynamic frontend leaves Unknown on nearly every expression. One
        // summary is useful; one warning per node buries everything else.
        let mut builder = Builder::new(LangId::PYTHON, span());
        for _ in 0..5 {
            builder.stmt(Stmt::Expr(int("1")));
        }
        let result = builder.finish();
        let summary: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == Some("gset-untyped"))
            .collect();
        assert_eq!(summary.len(), 1, "expected one summary");
        assert!(summary[0].message.starts_with("5 expression(s)"));
    }

    #[test]
    fn a_fully_typed_module_reports_no_unknown_types() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        let mut typed = Expr::int("1", span());
        typed.ty = Type::Int(crate::types::IntWidth::I32);
        builder.stmt(Stmt::Expr(typed));
        assert!(builder.finish().diagnostics.is_empty());
    }

    #[test]
    fn item_from_keeps_a_frontend_error_instead_of_aborting() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        let failure: Result<Item, crate::diagnostic::Diagnostic> = Err(
            crate::diagnostic::Diagnostic::error(span(), "unsupported syntax"),
        );
        builder.item_from(failure);
        builder.item_from(Ok(Item::Function(function("f", Block::empty(span())))));
        let result = builder.finish();
        assert!(result.module.is_none());
        assert_eq!(messages(&result.diagnostics), ["unsupported syntax"]);
    }

    #[test]
    fn a_frontend_diagnostic_is_preserved_alongside_the_checks() {
        let mut builder = Builder::new(LangId::PYTHON, span());
        builder.report(crate::diagnostic::Diagnostic::info(
            span(),
            "from the frontend",
        ));
        builder.stmt(Stmt::Break {
            label: None,
            span: span(),
        });
        let result = builder.finish();
        assert_eq!(messages(&result.diagnostics).len(), 2);
    }
}
