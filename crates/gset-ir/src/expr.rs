//! Expressions.
//!
//! An [`Expr`] is a kind, a type and a span. The type is present so a backend
//! never has to guess what it is holding, and the span so a diagnostic can point
//! at the source that produced it.
//!
//! # Why `ExprKind::Error` exists
//!
//! Lowering recovers from constructs it does not understand by producing an
//! [`ExprKind::Error`] of type [`Type::Unknown`] rather than unwinding. One
//! unrecognised construct therefore costs one diagnostic instead of the whole
//! translation, and the rest of the function still emits. This is the concrete
//! form of the "degradation, not failure" rule in the crate docs.
//!
//! # Short-circuit logic is not a binary operator
//!
//! [`ExprKind::Logical`] is separate from [`ExprKind::Binary`] on purpose.
//! `a && b` evaluates `b` conditionally in every source and target language.
//! Folding it into `Binary` means a backend has to re-derive short-circuit
//! semantics from the operator, and one that forgets emits eager evaluation.
//! The Go emitter did not model this at all, which is how `&&` reached Ruby as
//! syntax that language does not have.

use crate::span::Span;
use crate::types::{Name, Type, name};

/// A unary operator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnaryOp {
    /// Arithmetic negation, `-x`.
    Neg,
    /// Logical not, `not x`.
    Not,
    /// Bitwise complement, `~x`.
    BitNot,
    /// Address-of, `&x`.
    Ref,
    /// Mutable address-of, `&mut x`.
    MutRef,
    /// Dereference, `*x`.
    Deref,
    /// Pre-increment, `++x`.
    PreIncrement,
    /// Post-increment, `x++`.
    PostIncrement,
    /// Pre-decrement, `--x`.
    PreDecrement,
    /// Post-decrement, `x--`.
    PostDecrement,
}

/// A binary arithmetic, bitwise or shift operator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `**`
    Pow,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
}

impl BinaryOp {
    /// Reports whether this operator may be evaluated as text.
    ///
    /// Exponentiation is the motivating case: Python spells it `**`, Ruby and
    /// C-family languages spell it `Math.pow` or a runtime call, and SQL has no
    /// equivalent. A backend that lacks an operator cannot simply print it.
    pub fn may_need_expansion(self) -> bool {
        matches!(self, BinaryOp::Pow)
    }
}

/// A comparison operator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComparisonOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `is`
    Is,
    /// `is not`
    IsNot,
    /// `in`
    In,
    /// `not in`
    NotIn,
}

/// A short-circuiting logical operator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogicalOp {
    /// `and`, `&&`
    And,
    /// `or`, `||`
    Or,
}

/// The form a destructuring pattern takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PatternKind {
    /// A single name.
    Bind,
    /// Destructure a sequence, such as Python's `a, b = pair`.
    Sequence,
    /// Destructure a mapping, such as a Python dict or a JS object.
    Mapping,
    /// Ignore the value entirely.
    Ignore,
}

/// A target of assignment or binding.
///
/// Modelled as a distinct node rather than as an [`Expr`] because assignment
/// targets are restricted: an arbitrary expression is not a valid lvalue, and
/// representing one as `Expr` invited exactly the class of bug where a backend
/// emitted `f(x) = 1`.
#[derive(Clone, PartialEq, Debug)]
pub struct Pattern {
    /// What form this pattern takes.
    pub kind: PatternKind,
    /// The names bound, in source order.
    pub names: Vec<Name>,
    /// Nested patterns for `Sequence` and `Mapping`.
    pub subpatterns: Vec<Pattern>,
    /// The span of the whole pattern.
    pub span: Span,
}

impl Pattern {
    /// Creates a simple binding to one name.
    pub fn bind(name: Name, span: Span) -> Self {
        Pattern {
            kind: PatternKind::Bind,
            names: vec![name],
            subpatterns: Vec::new(),
            span,
        }
    }

    /// Creates a pattern that binds nothing.
    pub fn ignore(span: Span) -> Self {
        Pattern {
            kind: PatternKind::Ignore,
            names: Vec::new(),
            subpatterns: Vec::new(),
            span,
        }
    }

    /// Creates a destructuring pattern.
    pub fn sequence(subpatterns: Vec<Pattern>, span: Span) -> Self {
        Pattern {
            kind: PatternKind::Sequence,
            names: Vec::new(),
            subpatterns,
            span,
        }
    }

    /// Creates a mapping destructuring pattern.
    pub fn mapping(subpatterns: Vec<Pattern>, span: Span) -> Self {
        Pattern {
            kind: PatternKind::Mapping,
            names: Vec::new(),
            subpatterns,
            span,
        }
    }
}

/// A literal value.
#[derive(Clone, PartialEq, Debug)]
pub enum Literal {
    /// An integer literal, kept as text so a literal too large for any
    /// supported width survives intact.
    ///
    /// Text rather than `i64` because the Go implementation formatted every
    /// numeric literal with `%g` and `%q`, silently mangling large integers and
    /// producing quoted floats.
    Int(String),
    /// A floating-point literal, kept as text for the same reason.
    Float(String),
    /// A string literal, kept as text rather than a parsed string so escape
    /// sequences are the frontend's problem and are never re-encoded by a
    /// backend.
    Str(String),
    /// A byte-string literal.
    Bytes(String),
    /// A character literal.
    Char(String),
    /// A boolean.
    Bool(bool),
    /// The absent value.
    Null,
}

/// A dotted path such as `os.path.join`.
#[derive(Clone, PartialEq, Debug)]
pub struct Path {
    /// The segments, outermost first.
    pub segments: Vec<Name>,
}

impl Path {
    /// Creates a path from segments.
    pub fn new(segments: Vec<Name>) -> Self {
        Path { segments }
    }

    /// Creates a single-segment path.
    pub fn single(segment: Name) -> Self {
        Path {
            segments: vec![segment],
        }
    }

    /// Reports whether this is a bare name with no qualification.
    pub fn is_bare(&self) -> bool {
        self.segments.len() == 1
    }

    /// Returns the final segment.
    pub fn last(&self) -> Option<&Name> {
        self.segments.last()
    }

    /// Returns the path with the final segment removed.
    pub fn parent(&self) -> Option<Path> {
        self.segments
            .len()
            .checked_sub(1)
            .filter(|len| *len > 0)
            .map(|len| Path {
                segments: self.segments[..len].to_vec(),
            })
    }
}

/// One named argument in a call.
#[derive(Clone, PartialEq, Debug)]
pub struct NamedArg {
    /// The name the source used.
    pub name: Name,
    /// The value.
    pub value: Expr,
}

/// The form a comprehension takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComprehensionKind {
    /// List, Python's `[f(x) for x in xs]`.
    List,
    /// Set, Python's `{f(x) for x in xs}`.
    Set,
    /// Mapping, Python's `{k: v for k, v in items}`.
    Map,
    /// Generator, Python's `(f(x) for x in xs)`.
    Generator,
}

/// One generator clause of a comprehension: bind a pattern, then iterate.
#[derive(Clone, PartialEq, Debug)]
pub struct GeneratorClause {
    /// The binding target.
    pub pattern: Pattern,
    /// The sequence to iterate.
    pub iterable: Expr,
    /// Where the clause was written.
    pub span: Span,
}

/// The result of lowering something that could not be represented.
///
/// Carrying this instead of aborting is what lets a frontend report more than
/// one problem per run, and lets a backend emit what it understood.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompileError {
    /// Why lowering failed here.
    pub reason: String,
}

impl CompileError {
    /// Creates a compile error.
    pub fn new(reason: impl Into<String>) -> Self {
        CompileError {
            reason: reason.into(),
        }
    }
}

/// The kinds of expression.
#[derive(Clone, PartialEq, Debug)]
pub enum ExprKind {
    /// A literal value.
    Literal(Literal),

    /// A reference to a name, possibly module-qualified.
    Path(Path),

    /// A unary operation.
    Unary {
        /// Which operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<Expr>,
    },

    /// A binary arithmetic, bitwise or shift operation.
    Binary {
        /// Which operator.
        op: BinaryOp,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },

    /// A comparison. Separate from [`ExprKind::Binary`] because comparison
    /// operators include membership tests that have no arithmetic equivalent.
    Compare {
        /// Which operator.
        op: ComparisonOp,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },

    /// A short-circuiting logical operation.
    ///
    /// Never represented as [`ExprKind::Binary`], so a backend cannot lose
    /// short-circuit semantics by treating it as an ordinary operator.
    Logical {
        /// Which operator.
        op: LogicalOp,
        /// The left operand, always evaluated.
        lhs: Box<Expr>,
        /// The right operand, evaluated only when needed.
        rhs: Box<Expr>,
    },

    /// A call to a callable.
    Call {
        /// The callee.
        callee: Box<Expr>,
        /// Positional arguments.
        args: Vec<Expr>,
        /// Keyword arguments.
        named_args: Vec<NamedArg>,
    },

    /// A method call on a receiver.
    MethodCall {
        /// The receiver.
        receiver: Box<Expr>,
        /// The method name.
        method: Name,
        /// Positional arguments.
        args: Vec<Expr>,
        /// Keyword arguments.
        named_args: Vec<NamedArg>,
    },

    /// Element access.
    Index {
        /// The sequence being indexed.
        target: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },

    /// A slice, with either bound optional.
    Slice {
        /// The sequence being sliced.
        target: Box<Expr>,
        /// Inclusive start, if given.
        start: Option<Box<Expr>>,
        /// Exclusive end, if given.
        end: Option<Box<Expr>>,
        /// Exclusive end, if the source wrote an inclusive range.
        inclusive: bool,
    },

    /// Field access on a value.
    Field {
        /// The base value.
        target: Box<Expr>,
        /// The field name.
        field: Name,
    },

    /// Construction of a named type.
    StructLit {
        /// The type being constructed.
        path: Path,
        /// Fields, in source order. A backend may need to reorder these to
        /// match the target's constructor or keyword rules.
        fields: Vec<NamedArg>,
    },

    /// A sequence literal.
    List {
        /// The elements.
        elements: Vec<Expr>,
    },

    /// A tuple literal.
    Tuple {
        /// The elements.
        elements: Vec<Expr>,
    },

    /// A set literal.
    Set {
        /// The elements.
        elements: Vec<Expr>,
    },

    /// A mapping literal.
    Map {
        /// The entries, in source order.
        entries: Vec<NamedArg>,
    },

    /// An anonymous function.
    Lambda {
        /// Parameter patterns, in declaration order.
        params: Vec<Pattern>,
        /// The body.
        body: Box<Expr>,
    },

    /// A conditional expression, such as Python's `a if c else b`.
    ///
    /// Not a statement, because the source allows it in expression position in
    /// several languages. Folding it into a statement loses that.
    Conditional {
        /// The condition.
        condition: Box<Expr>,
        /// The value when the condition holds.
        then_branch: Box<Expr>,
        /// The value otherwise.
        else_branch: Box<Expr>,
    },

    /// A range of values.
    Range {
        /// The first value, if given.
        start: Option<Box<Expr>>,
        /// The last value, if given.
        end: Option<Box<Expr>>,
        /// Whether the end is included.
        inclusive: bool,
    },

    /// An explicit type conversion.
    Cast {
        /// The value being converted.
        expr: Box<Expr>,
        /// The type to convert to.
        to: Type,
    },

    /// A type assertion that is also a narrowing test, such as JS `as` or a
    /// Python `isinstance` call in narrowing position.
    TypeAssert {
        /// The value.
        expr: Box<Expr>,
        /// The type asserted.
        to: Type,
    },

    /// Awaiting an asynchronous value.
    Await {
        /// The awaited expression.
        expr: Box<Expr>,
    },

    /// An assignment.
    Assign {
        /// The assignment target.
        target: Box<Pattern>,
        /// The existing value, for a compound assignment.
        ///
        /// A backend that does not support the operator expands this into a
        /// plain assignment by rewriting it.
        previous: Option<Box<Expr>>,
        /// Which operator, for a compound assignment. `None` is plain `=`.
        op: Option<BinaryOp>,
        /// The assigned value.
        value: Box<Expr>,
    },

    /// A comprehension.
    ///
    /// Modelled as a node rather than desugared into a loop, because the source
    /// semantics differ: Python comprehensions have their own scope, and a
    /// backend for a loop-based language that expands this naively leaks
    /// bindings. The Go emitter had no node for this at all and emitted Python
    /// comprehension syntax into Go verbatim.
    Comprehension {
        /// Which form.
        kind: ComprehensionKind,
        /// The produced value for a list, set or generator.
        element: Box<Expr>,
        /// The produced key for a mapping.
        key: Option<Box<Expr>>,
        /// The produced value for a mapping.
        value: Option<Box<Expr>>,
        /// The generator clauses, in order.
        clauses: Vec<GeneratorClause>,
        /// An optional filter applied after every clause.
        condition: Option<Box<Expr>>,
    },

    /// An interpolated string, such as Python's f-string or JS template
    /// literal.
    ///
    /// The literal parts and the expressions are kept separate so a backend can
    /// emit concatenation, an interpolation function, or a template literal
    /// without parsing the source text again.
    Format {
        /// The literal segments. Always one more than `arguments`, so the
        /// segments and arguments interleave unambiguously.
        segments: Vec<String>,
        /// The interpolated expressions, in order.
        arguments: Vec<Expr>,
        /// How the source wanted the value rendered.
        format: Option<String>,
    },

    /// Unpacking in an expression position, such as Python's `*rest`.
    Unpack {
        /// The value being unpacked.
        expr: Box<Expr>,
    },

    /// Something lowering could not represent.
    ///
    /// Always of type [`Type::Unknown`], so it propagates honestly instead of
    /// letting a guess creep in.
    Error(CompileError),
}

/// An expression: a kind, a type and a span.
///
/// `ty` is mutable through `&mut` so `gset-semantic` can fill it in after
/// lowering, rather than the IR needing a side table keyed by node identity.
#[derive(Clone, PartialEq, Debug)]
pub struct Expr {
    /// What the expression is.
    pub kind: ExprKind,
    /// Its type. [`Type::UNKNOWN`] until a frontend or inference supplies one.
    pub ty: Type,
    /// Where it came from.
    pub span: Span,
}

impl Expr {
    /// Creates an expression of unknown type.
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Expr {
            kind,
            ty: Type::UNKNOWN,
            span,
        }
    }

    /// Creates an expression with a known type.
    pub fn typed(kind: ExprKind, ty: Type, span: Span) -> Self {
        Expr { kind, ty, span }
    }

    /// Creates a reference to a single name.
    pub fn path(segment: impl AsRef<str>, span: Span) -> Self {
        Expr::new(ExprKind::Path(Path::single(name(segment))), span)
    }

    /// Creates an integer literal.
    pub fn int(value: impl Into<String>, span: Span) -> Self {
        Expr::new(ExprKind::Literal(Literal::Int(value.into())), span)
    }

    /// Creates a string literal.
    pub fn string(value: impl Into<String>, span: Span) -> Self {
        Expr::new(ExprKind::Literal(Literal::Str(value.into())), span)
    }

    /// Creates a boolean literal.
    pub fn bool(value: bool, span: Span) -> Self {
        Expr::new(ExprKind::Literal(Literal::Bool(value)), span)
    }

    /// Creates the absent value.
    pub fn null(span: Span) -> Self {
        Expr::new(ExprKind::Literal(Literal::Null), span)
    }

    /// Creates an error expression, carrying the failure instead of unwinding.
    pub fn error(reason: impl Into<String>, span: Span) -> Self {
        Expr::new(ExprKind::Error(CompileError::new(reason)), span)
    }

    /// Reports whether this expression is an error placeholder.
    pub fn is_error(&self) -> bool {
        matches!(self.kind, ExprKind::Error(_))
    }

    /// Reports whether this expression is a literal.
    pub fn is_literal(&self) -> bool {
        matches!(self.kind, ExprKind::Literal(_))
    }

    /// Returns the type, or [`Type::UNKNOWN`] if it has none.
    ///
    /// Convenience for backends, which almost always need the type and should
    /// not have to match on the kind first.
    pub fn ty(&self) -> &Type {
        &self.ty
    }

    /// Returns the immediate child expressions, in evaluation-relevant order.
    ///
    /// Used by inference to walk an expression tree, and by backends that need
    /// to find nested nodes such as string interpolation arguments.
    pub fn children(&self) -> Vec<&Expr> {
        match &self.kind {
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::Error(_) => Vec::new(),
            ExprKind::Unary { operand, .. } => vec![operand.as_ref()],
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::Compare { lhs, rhs, .. }
            | ExprKind::Logical { lhs, rhs, .. } => vec![lhs.as_ref(), rhs.as_ref()],
            ExprKind::Call {
                callee,
                args,
                named_args,
            } => {
                let mut out = vec![callee.as_ref()];
                out.extend(args.iter());
                out.extend(named_args.iter().map(|a| &a.value));
                out
            }
            ExprKind::MethodCall {
                receiver,
                args,
                named_args,
                ..
            } => {
                let mut out = vec![receiver.as_ref()];
                out.extend(args.iter());
                out.extend(named_args.iter().map(|a| &a.value));
                out
            }
            ExprKind::Index { target, index } => vec![target.as_ref(), index.as_ref()],
            ExprKind::Slice {
                target, start, end, ..
            } => {
                let mut out = vec![target.as_ref()];
                out.extend(start.as_ref().map(|s| s.as_ref()));
                out.extend(end.as_ref().map(|e| e.as_ref()));
                out
            }
            ExprKind::Field { target, .. } => vec![target.as_ref()],
            ExprKind::StructLit { fields, .. } => fields.iter().map(|f| &f.value).collect(),
            ExprKind::List { elements }
            | ExprKind::Tuple { elements }
            | ExprKind::Set { elements } => elements.iter().collect(),
            ExprKind::Map { entries } => entries.iter().map(|e| &e.value).collect(),
            ExprKind::Lambda { body, .. } => vec![body.as_ref()],
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => vec![
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ],
            ExprKind::Range { start, end, .. } => {
                let mut out = Vec::new();
                out.extend(start.as_ref().map(|s| s.as_ref()));
                out.extend(end.as_ref().map(|e| e.as_ref()));
                out
            }
            ExprKind::Cast { expr, .. }
            | ExprKind::TypeAssert { expr, .. }
            | ExprKind::Await { expr }
            | ExprKind::Unpack { expr } => vec![expr.as_ref()],
            ExprKind::Assign {
                previous, value, ..
            } => {
                let mut out = Vec::new();
                out.extend(previous.as_ref().map(|p| p.as_ref()));
                out.push(value.as_ref());
                out
            }
            ExprKind::Comprehension {
                kind: _,
                element,
                key,
                value,
                clauses,
                condition,
            } => {
                let mut out = vec![element.as_ref()];
                out.extend(key.as_ref().map(|k| k.as_ref()));
                out.extend(value.as_ref().map(|v| v.as_ref()));
                out.extend(clauses.iter().map(|c| &c.iterable));
                out.extend(condition.as_ref().map(|c| c.as_ref()));
                out
            }
            ExprKind::Format { arguments, .. } => arguments.iter().collect(),
        }
    }

    /// Applies `f` to this expression and every descendant, in pre-order.
    ///
    /// Walks the tree without allocating, so inference can use it for a fixpoint
    /// without building an intermediate `Vec` per node. A node is visited before
    /// its children, which is the order a rewrite pass needs.
    pub fn walk(&mut self, f: &mut impl FnMut(&mut Expr)) {
        f(self);
        match &mut self.kind {
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::Error(_) => {}
            ExprKind::Unary { operand, .. } => operand.walk(f),
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::Compare { lhs, rhs, .. }
            | ExprKind::Logical { lhs, rhs, .. } => {
                lhs.walk(f);
                rhs.walk(f);
            }
            ExprKind::Call {
                callee,
                args,
                named_args,
            }
            | ExprKind::MethodCall {
                receiver: callee,
                args,
                named_args,
                ..
            } => {
                callee.walk(f);
                for arg in args {
                    arg.walk(f);
                }
                for arg in named_args {
                    arg.value.walk(f);
                }
            }
            ExprKind::Index { target, index } => {
                target.walk(f);
                index.walk(f);
            }
            ExprKind::Slice {
                target, start, end, ..
            } => {
                target.walk(f);
                if let Some(start) = start {
                    start.walk(f);
                }
                if let Some(end) = end {
                    end.walk(f);
                }
            }
            ExprKind::Field { target, .. } => target.walk(f),
            ExprKind::StructLit { fields, .. } | ExprKind::Map { entries: fields } => {
                for field in fields {
                    field.value.walk(f);
                }
            }
            ExprKind::List { elements }
            | ExprKind::Tuple { elements }
            | ExprKind::Set { elements } => {
                for element in elements {
                    element.walk(f);
                }
            }
            ExprKind::Lambda { body, .. } => body.walk(f),
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.walk(f);
                then_branch.walk(f);
                else_branch.walk(f);
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(start) = start {
                    start.walk(f);
                }
                if let Some(end) = end {
                    end.walk(f);
                }
            }
            ExprKind::Cast { expr, .. }
            | ExprKind::TypeAssert { expr, .. }
            | ExprKind::Await { expr }
            | ExprKind::Unpack { expr } => expr.walk(f),
            ExprKind::Assign {
                previous, value, ..
            } => {
                if let Some(previous) = previous {
                    previous.walk(f);
                }
                value.walk(f);
            }
            ExprKind::Comprehension {
                kind: _,
                element,
                key,
                value,
                clauses,
                condition,
            } => {
                element.walk(f);
                if let Some(key) = key {
                    key.walk(f);
                }
                if let Some(value) = value {
                    value.walk(f);
                }
                for clause in clauses {
                    clause.iterable.walk(f);
                }
                if let Some(condition) = condition {
                    condition.walk(f);
                }
            }
            ExprKind::Format { arguments, .. } => {
                for argument in arguments {
                    argument.walk(f);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SourceId;

    fn span() -> Span {
        Span::point(SourceId::from_raw(0), 0)
    }

    #[test]
    fn new_expressions_have_unknown_type() {
        let expr = Expr::path("x", span());
        assert!(expr.ty().is_unknown());
        assert!(expr.ty().is_unknown());
    }

    #[test]
    fn typed_expressions_keep_their_type() {
        let expr = Expr::typed(
            ExprKind::Literal(Literal::Int("1".into())),
            Type::Bool,
            span(),
        );
        assert_eq!(expr.ty(), &Type::Bool);
    }

    #[test]
    fn error_expressions_are_unknown_and_identifiable() {
        let expr = Expr::error("no idea", span());
        assert!(expr.is_error());
        // An error must not masquerade as a real value, or a backend could emit
        // it as though it meant something.
        assert!(expr.ty().is_unknown());
        assert!(!Expr::int("1", span()).is_error());
    }

    #[test]
    fn literals_are_recognised() {
        assert!(Expr::int("42", span()).is_literal());
        assert!(Expr::string("hi", span()).is_literal());
        assert!(Expr::bool(true, span()).is_literal());
        assert!(Expr::null(span()).is_literal());
        assert!(!Expr::path("x", span()).is_literal());
    }

    #[test]
    fn paths_report_their_arity() {
        let bare = Path::single(name("x"));
        assert!(bare.is_bare());
        assert_eq!(&**bare.last().unwrap(), "x");
        assert!(bare.parent().is_none());

        let qualified = Path::new(vec![name("os"), name("path"), name("join")]);
        assert!(!qualified.is_bare());
        assert_eq!(&**qualified.last().unwrap(), "join");
        let parent = qualified.parent().unwrap();
        assert_eq!(parent.segments.len(), 2);
        assert_eq!(&**parent.last().unwrap(), "path");
    }

    #[test]
    fn exponentiation_is_flagged_for_expansion() {
        // A backend without a power operator cannot print one, so the operator
        // has to be identifiable as such.
        assert!(BinaryOp::Pow.may_need_expansion());
        assert!(!BinaryOp::Add.may_need_expansion());
        assert!(!BinaryOp::Mul.may_need_expansion());
    }

    #[test]
    fn binary_children_are_in_evaluation_order() {
        let expr = Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Sub,
                lhs: Box::new(Expr::int("1", span())),
                rhs: Box::new(Expr::int("2", span())),
            },
            span(),
        );
        let children = expr.children();
        assert_eq!(children.len(), 2);
        assert!(matches!(children[0].kind, ExprKind::Literal(Literal::Int(ref v)) if v == "1"));
        assert!(matches!(children[1].kind, ExprKind::Literal(Literal::Int(ref v)) if v == "2"));
    }

    #[test]
    fn logical_expressions_are_not_binary_expressions() {
        // Short-circuit semantics live in the variant, not in the operator, so a
        // backend cannot accidentally evaluate eagerly.
        let expr = Expr::new(
            ExprKind::Logical {
                op: LogicalOp::And,
                lhs: Box::new(Expr::bool(true, span())),
                rhs: Box::new(Expr::bool(false, span())),
            },
            span(),
        );
        assert!(matches!(expr.kind, ExprKind::Logical { .. }));
        assert!(!matches!(expr.kind, ExprKind::Binary { .. }));
        assert_eq!(expr.children().len(), 2);
    }

    #[test]
    fn call_children_include_positional_and_named_arguments() {
        let expr = Expr::new(
            ExprKind::Call {
                callee: Box::new(Expr::path("f", span())),
                args: vec![Expr::int("1", span())],
                named_args: vec![NamedArg {
                    name: name("sep"),
                    value: Expr::string(",", span()),
                }],
            },
            span(),
        );
        let children = expr.children();
        assert_eq!(children.len(), 3);
        assert!(matches!(children[0].kind, ExprKind::Path(_)));
    }

    #[test]
    fn optional_slice_bounds_are_skipped_not_guessed() {
        let open_ended = Expr::new(
            ExprKind::Slice {
                target: Box::new(Expr::path("xs", span())),
                start: Some(Box::new(Expr::int("1", span()))),
                end: None,
                inclusive: false,
            },
            span(),
        );
        // A missing bound must stay missing. Substituting 0 or len() here would
        // change the meaning of the slice.
        assert_eq!(open_ended.children().len(), 2);

        let open_start = Expr::new(
            ExprKind::Slice {
                target: Box::new(Expr::path("xs", span())),
                start: None,
                end: Some(Box::new(Expr::int("3", span()))),
                inclusive: false,
            },
            span(),
        );
        assert_eq!(open_start.children().len(), 2);
    }

    #[test]
    fn comprehension_is_a_node_and_keeps_all_its_parts() {
        let expr = Expr::new(
            ExprKind::Comprehension {
                kind: ComprehensionKind::List,
                element: Box::new(Expr::path("x", span())),
                key: None,
                value: None,
                clauses: vec![GeneratorClause {
                    pattern: Pattern::bind(name("x"), span()),
                    iterable: Expr::path("xs", span()),
                    span: span(),
                }],
                condition: Some(Box::new(Expr::bool(true, span()))),
            },
            span(),
        );
        // element + iterable + condition
        assert_eq!(expr.children().len(), 3);
        assert!(matches!(expr.kind, ExprKind::Comprehension { .. }));
    }

    #[test]
    fn mapping_comprehension_keeps_key_and_value_separate() {
        let expr = Expr::new(
            ExprKind::Comprehension {
                kind: ComprehensionKind::Map,
                element: Box::new(Expr::path("unused", span())),
                key: Some(Box::new(Expr::path("k", span()))),
                value: Some(Box::new(Expr::path("v", span()))),
                clauses: vec![GeneratorClause {
                    pattern: Pattern::bind(name("k"), span()),
                    iterable: Expr::path("items", span()),
                    span: span(),
                }],
                condition: None,
            },
            span(),
        );
        // element + key + value + iterable
        assert_eq!(expr.children().len(), 4);
    }

    #[test]
    fn assignment_records_the_previous_value_for_compound_operators() {
        // A backend that lacks the operator needs the old value to expand
        // `x += 1` into `x = x + 1`.
        let compound = Expr::new(
            ExprKind::Assign {
                target: Box::new(Pattern::bind(name("x"), span())),
                previous: Some(Box::new(Expr::path("x", span()))),
                op: Some(BinaryOp::Add),
                value: Box::new(Expr::int("1", span())),
            },
            span(),
        );
        match &compound.kind {
            ExprKind::Assign { previous, op, .. } => {
                assert!(previous.is_some());
                assert_eq!(*op, Some(BinaryOp::Add));
            }
            other => panic!("expected assign, got {other:?}"),
        }

        let plain = Expr::new(
            ExprKind::Assign {
                target: Box::new(Pattern::bind(name("x"), span())),
                previous: None,
                op: None,
                value: Box::new(Expr::int("1", span())),
            },
            span(),
        );
        assert_eq!(plain.children().len(), 1);
    }

    #[test]
    fn format_parts_outnumber_arguments() {
        let expr = Expr::new(
            ExprKind::Format {
                segments: vec!["a".into(), "b".into(), "c".into()],
                arguments: vec![Expr::path("x", span()), Expr::path("y", span())],
                format: Some(">10".into()),
            },
            span(),
        );
        match &expr.kind {
            ExprKind::Format {
                segments,
                arguments,
                ..
            } => assert_eq!(segments.len(), arguments.len() + 1),
            other => panic!("expected format, got {other:?}"),
        }
    }

    #[test]
    fn destructuring_patterns_nest() {
        let pattern = Pattern::sequence(
            vec![
                Pattern::bind(name("a"), span()),
                Pattern::bind(name("b"), span()),
            ],
            span(),
        );
        assert_eq!(pattern.kind, PatternKind::Sequence);
        assert_eq!(pattern.subpatterns.len(), 2);
        assert!(pattern.subpatterns[0].kind == PatternKind::Bind);
    }

    #[test]
    fn walk_visits_every_descendant_exactly_once() {
        let mut expr = Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::new(
                    ExprKind::List {
                        elements: vec![Expr::int("1", span()), Expr::int("2", span())],
                    },
                    span(),
                )),
                rhs: Box::new(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Pattern::bind(name("n"), span())],
                        body: Box::new(Expr::new(
                            ExprKind::Await {
                                expr: Box::new(Expr::path("f", span())),
                            },
                            span(),
                        )),
                    },
                    span(),
                )),
            },
            span(),
        );

        let mut literals = 0;
        expr.walk(&mut |node| {
            if let ExprKind::Literal(Literal::Int(_)) = &node.kind {
                literals += 1;
            }
        });
        // Two list elements only. Pre-order with no duplicates.
        assert_eq!(literals, 2);
    }

    #[test]
    fn walk_can_rewrite_the_tree_in_place() {
        let mut expr = Expr::new(
            ExprKind::List {
                elements: vec![Expr::int("1", span()), Expr::int("2", span())],
            },
            span(),
        );
        expr.walk(&mut |node| {
            if let ExprKind::Literal(Literal::Int(value)) = &mut node.kind {
                let doubled: i64 = value.parse::<i64>().unwrap() * 2;
                *value = doubled.to_string();
            }
        });
        match &expr.kind {
            ExprKind::List { elements } => {
                assert!(
                    matches!(&elements[0].kind, ExprKind::Literal(Literal::Int(v)) if v == "2")
                );
                assert!(
                    matches!(&elements[1].kind, ExprKind::Literal(Literal::Int(v)) if v == "4")
                );
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn literal_text_is_preserved_verbatim() {
        // The Go emitter formatted literals with %g and %q, mangling large
        // integers and quoting floats. Text is kept so that cannot recur.
        let big = Expr::int("123456789012345678901234567890", span());
        match &big.kind {
            ExprKind::Literal(Literal::Int(text)) => {
                assert_eq!(text, "123456789012345678901234567890")
            }
            other => panic!("expected int literal, got {other:?}"),
        }
    }
}
