//! Target identity and the capability matrix.
//!
//! # Why capabilities are declared up front
//!
//! The Go emitter decided support *during* emission, inside 42 `switch`
//! statements, and the default arm was often `return nil`. That is how `enum`,
//! `struct`, `trait`, `interface`, a type alias and `export` all vanished from
//! generated output with no warning: no case matched, and nothing recorded that
//! it had not.
//!
//! Here a backend answers [`Backend::capability`] before the driver emits
//! anything, so an unsupported construct is a decision the driver makes with a
//! span in hand — a typed diagnostic or an explicit marker — rather than a
//! branch a backend author forgot to write.

/// A target language a backend can emit.
///
/// A newtype over a raw id rather than an enum of units so adding a target is
/// not a breaking change for a backend that only knows the existing ones. The
/// associated constants are the ones that exist today.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TargetId(u16);

impl TargetId {
    /// Go.
    pub const GO: TargetId = TargetId(0);
    /// Python.
    pub const PYTHON: TargetId = TargetId(1);
    /// JavaScript.
    pub const JAVASCRIPT: TargetId = TargetId(2);
    /// Java.
    pub const JAVA: TargetId = TargetId(3);
    /// Ruby.
    pub const RUBY: TargetId = TargetId(4);

    /// Wraps a raw identifier.
    pub const fn from_raw(raw: u16) -> TargetId {
        TargetId(raw)
    }

    /// The raw identifier.
    pub const fn as_raw(self) -> u16 {
        self.0
    }

    /// Resolves a target from its conventional name.
    ///
    /// Accepts the names a user types, which are not always the same as the
    /// canonical id: `js` and `javascript`, `golang` and `go`.
    pub fn from_name(name: &str) -> Option<TargetId> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "go" | "golang" => TargetId::GO,
            "py" | "python" | "python3" => TargetId::PYTHON,
            "js" | "javascript" | "node" => TargetId::JAVASCRIPT,
            "java" => TargetId::JAVA,
            "rb" | "ruby" => TargetId::RUBY,
            _ => return None,
        })
    }

    /// The canonical name.
    pub fn name(self) -> &'static str {
        match self {
            TargetId::GO => "go",
            TargetId::PYTHON => "python",
            TargetId::JAVASCRIPT => "javascript",
            TargetId::JAVA => "java",
            TargetId::RUBY => "ruby",
            _ => "unknown",
        }
    }

    /// The file extension generated files should carry.
    pub fn extension(self) -> &'static str {
        match self {
            TargetId::GO => "go",
            TargetId::PYTHON => "py",
            TargetId::JAVASCRIPT => "js",
            TargetId::JAVA => "java",
            TargetId::RUBY => "rb",
            _ => "txt",
        }
    }

    /// Every target, in naming order.
    pub const ALL: [TargetId; 5] = [
        TargetId::GO,
        TargetId::PYTHON,
        TargetId::JAVASCRIPT,
        TargetId::JAVA,
        TargetId::RUBY,
    ];
}

impl std::fmt::Display for TargetId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

/// A construct a backend may or may not support.
///
/// Coarse on purpose: one variant per family of source construct whose
/// translation strategy genuinely differs between targets. A finer set would
/// drift from what backends actually branch on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Capability {
    /// A top-level import.
    Imports,
    /// A class with methods.
    Classes,
    /// A record or data class.
    Records,
    /// An enumeration.
    Enums,
    /// An interface or trait.
    Interfaces,
    /// A type alias.
    TypeAliases,
    /// A comprehension over one or more clauses.
    Comprehensions,
    /// A generator function or `yield`.
    Generators,
    /// `try` / `catch` / `finally`.
    Exceptions,
    /// `with`, `defer`, or another scoped-resource construct.
    ContextManagers,
    /// A decorator on a function or class.
    Decorators,
    /// `async` / `await`.
    Async,
    /// An interpolated string such as an f-string.
    FStrings,
    /// A nested function or closure.
    Closures,
    /// A `match` or `switch` with patterns.
    PatternMatching,
    /// A slice.
    Slices,
    /// Call arguments given by keyword.
    KeywordArguments,
    /// Multiple assignment from one expression, such as `a, b = f()`.
    MultipleAssignment,
    /// An operator the target evaluates with a runtime call rather than a
    /// symbol, such as exponentiation.
    OperatorCall,
    /// A loop `else`, which runs when the loop ends without `break`.
    LoopElse,
}

impl Capability {
    /// A human-readable name, for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Capability::Imports => "imports",
            Capability::Classes => "classes",
            Capability::Records => "records",
            Capability::Enums => "enums",
            Capability::Interfaces => "interfaces",
            Capability::TypeAliases => "type aliases",
            Capability::Comprehensions => "comprehensions",
            Capability::Generators => "generators",
            Capability::Exceptions => "exceptions",
            Capability::ContextManagers => "context managers",
            Capability::Decorators => "decorators",
            Capability::Async => "async",
            Capability::FStrings => "f-strings",
            Capability::Closures => "closures",
            Capability::PatternMatching => "pattern matching",
            Capability::Slices => "slices",
            Capability::KeywordArguments => "keyword arguments",
            Capability::MultipleAssignment => "multiple assignment",
            Capability::OperatorCall => "operators needing a runtime call",
            Capability::LoopElse => "a loop `else`",
        }
    }
}

/// How well a backend can express a [`Capability`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Support {
    /// The target has a direct equivalent.
    Native,
    /// The target has no direct equivalent but the construct can be rewritten
    /// faithfully without changing the program.
    Desugared,
    /// The target cannot express the construct. The driver must report it.
    Unsupported,
}

impl Support {
    /// Whether a construct may be emitted at all.
    pub fn is_supported(self) -> bool {
        !matches!(self, Support::Unsupported)
    }
}
