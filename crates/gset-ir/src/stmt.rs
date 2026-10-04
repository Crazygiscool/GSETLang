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
use crate::item::Item;
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

/// A module-level statement.
///
/// Note what is *not* here: declarations. A binding, a function, a class and an
/// import are all [`crate::item::Item`]s. That split is what lets `gset-semantic` reason about
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
        /// The block that runs when the loop ends without `break`.
        ///
        /// Python's loop `else`. Kept rather than dropped because it is live
        /// code: a backend with no equivalent has to synthesise a flag, and it
        /// cannot do that if the frontend already threw the block away.
        else_body: Option<Block>,
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
        /// The block that runs when the loop ends without `break`. See
        /// [`Stmt::While`].
        else_body: Option<Block>,
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
        ///
        /// `None` is a bare re-throw: Python's `raise`, C#'s `throw;`, which
        /// propagates the active exception rather than naming a new one.
        value: Option<Expr>,
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
    ///
    /// One [`Stmt::With`] per resource rather than one per block, because that
    /// is what the source means: Python's `with a() as x, b() as y` acquires
    /// `a`, then `b`, and releases them in reverse. A single node with a list of
    /// resources could not say which body statement each binding is live for.
    With {
        /// The managed resource.
        value: Expr,
        /// The name the resource is bound to, as Python's `as` clause states it.
        ///
        /// Required in practice rather than decorative: `with open(p) as f`
        /// leaves the body referring to `f`, so a node that dropped the binding
        /// would produce a body referencing a name that does not exist.
        binding: Option<Pattern>,
        /// The body.
        body: Block,
        /// Whether this resource is the last one in its `with` statement.
        ///
        /// A backend emitting nested `defer`s needs to know it has reached the
        /// end, because the release order inverts at the end.
        is_last: bool,
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

    /// A declaration that appears inside a body, such as a closure or a local
    /// class.
    ///
    /// A separate variant rather than an [`Item`] pushed into a block, because
    /// the distinction between "this is a module declaration" and "this is a
    /// local declaration" is exactly what a scope pass needs, and a `Block` of
    /// `Item`s would make every statement site answer for imports too.
    ///
    /// Boxed because an [`Item`] can itself contain a `Stmt`, and the two
    /// variants would otherwise be infinitely large.
    LocalItem(Box<Item>),

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
            Stmt::LocalItem(item) => item.span(),
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
            Stmt::Throw { value, .. } => {
                if let Some(value) = value {
                    value.walk(f_expr);
                }
            }
            Stmt::Defer { expr, .. } => expr.walk(f_expr),
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
                condition,
                body,
                else_body,
                ..
            } => {
                condition.walk(f_expr);
                body.walk(f_stmt, f_expr);
                if let Some(else_body) = else_body {
                    else_body.walk(f_stmt, f_expr);
                }
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
            Stmt::ForIn {
                iterable,
                body,
                else_body,
                ..
            } => {
                iterable.walk(f_expr);
                body.walk(f_stmt, f_expr);
                if let Some(else_body) = else_body {
                    else_body.walk(f_stmt, f_expr);
                }
            }
            Stmt::Block(block) => block.walk(f_stmt, f_expr),
            Stmt::LocalItem(item) => {
                if let Item::Function(function) = item.as_mut() {
                    function.body.walk(f_stmt, f_expr);
                }
            }
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
                    else_body: None,
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
    fn error_statements_are_identifiable() {
        let bad = Stmt::error("unlowered", span());
        assert!(bad.is_error());
        assert!(!Stmt::Empty { span: span() }.is_error());
        assert!(!Stmt::Expr(int("1")).is_error());
    }
}
