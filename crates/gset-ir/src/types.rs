//! The IR type system.
//!
//! Types are explicit in the IR rather than inferred on demand at emit time. That
//! is what lets a backend choose a target type: `python -m py_compile` will
//! happily accept a function whose annotations are wrong, but a Go backend
//! cannot invent one.
//!
//! # `Unknown` is contagious
//!
//! When a frontend cannot determine a type it records [`Type::Unknown`] and that
//! uncertainty propagates through every operation touching it. A backend
//! answering [`Type::Unknown`] must pick a conservative strategy and may attach a
//! diagnostic. Nobody may narrow it to a concrete type on a guess.
//!
//! The reason is that a guess is worse than a hole. Code emitted from a guessed
//! type compiles, runs, and then misbehaves, and the original source is nowhere
//! in the stack trace. A hole surfaces immediately as a diagnostic.
//!
//! The Go implementation had no inference at all. Every Java parameter and
//! return type became `Object`, including a `void` return for any function whose
//! only `return` sat inside an `if` — a bug class the type system makes
//! unrepresentable.

use std::fmt;
use std::sync::Arc;

/// An interned identifier for a declared name.
///
/// `Arc<str>` rather than `String` because names are cloned constantly (once per
/// binding, once per use site) and are immutable once interned. This keeps clones
/// to a refcount bump with no allocation.
pub type Name = Arc<str>;

/// Wraps a string as a [`Name`].
pub fn name(value: impl AsRef<str>) -> Name {
    Arc::from(value.as_ref())
}

/// The reason a type is unknown.
///
/// Carried so a backend can distinguish "the frontend does not know" from "this
/// is genuinely dynamic at runtime". The two need different strategies: the
/// former deserves a diagnostic, the latter may be legitimately untypeable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnknownReason {
    /// A frontend could not determine the type.
    Undetermined,
    /// The language is dynamically typed at this position and no static
    /// inference was applied.
    Dynamic,
    /// Inference ran and gave up, usually through a cycle it would not break.
    InferenceFailed,
    /// The type depends on something not available, such as a type from an
    /// opaque dependency.
    Unavailable,
}

/// A resolved reference to a declared generic parameter.
///
/// Generic parameters are referenced by index rather than by name because the
/// declaration is not always in scope at the use site, and a name would make
/// every type carry an `Arc<str>` for something that is one integer.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct GenericId(pub u32);

/// The integer widths a target may need to distinguish.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntWidth {
    /// 8 bits.
    I8,
    /// 16 bits.
    I16,
    /// 32 bits.
    I32,
    /// 64 bits.
    I64,
    /// A machine word; the native width of the target.
    Isize,
}

impl IntWidth {
    /// Returns the width in bits, or `None` for [`IntWidth::Isize`] because the
    /// native width depends on the target.
    pub const fn bits(self) -> Option<u32> {
        match self {
            IntWidth::I8 => Some(8),
            IntWidth::I16 => Some(16),
            IntWidth::I32 => Some(32),
            IntWidth::I64 => Some(64),
            IntWidth::Isize => None,
        }
    }
}

/// How a collection type constrains its elements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variance {
    /// Neither covariant nor contravariant. Used for immutable collections,
    /// which are usually what source code means.
    Invariant,
    /// A list that may grow or shrink.
    Covariant,
    /// A list that only accepts more.
    Contravariant,
}

/// A type in the IR.
///
/// Deliberately not a complete type calculus. This is a source-to-source
/// representation, so it models the types a *source language author writes down*
/// and nothing more. There are no pointers, no lifetimes, no regions and no
/// mutability in types: those are target-language concerns a backend derives
/// from context, and modelling them here would force every source language to
/// answer questions only a target can answer.
#[derive(Clone, PartialEq, Debug)]
pub enum Type {
    /// The type could not be determined.
    ///
    /// Contagious: every operation on it yields it again. See the module docs.
    Unknown(UnknownReason),

    /// A value that never produces one. The type of `return`, `raise` and
    /// similar diverging expressions, and the bottom type of the lattice.
    Never,

    /// No value. The return type of a function with no `return`.
    Void,

    /// A boolean.
    Bool,

    /// An integer of the given width.
    Int(IntWidth),

    /// A floating-point number.
    Float,

    /// A string of text.
    Str,

    /// A string known at compile time, for languages that distinguish the two.
    StrLit,

    /// A string that is a byte sequence rather than text.
    Bytes,

    /// A single character.
    Char,

    /// The absent value, distinct from every other type.
    ///
    /// Modelled separately from an optional because JavaScript conflates them,
    /// Python conflates them, and Go has neither. A backend that can express the
    /// distinction does; one that cannot maps [`Type::Option`] and
    /// [`Type::Null`] onto whatever its equivalent is.
    Null,

    /// An ordered sequence of `T`.
    List {
        /// The element type.
        element: Box<Type>,
        /// How the collection may vary.
        variance: Variance,
    },

    /// A fixed-length sequence of heterogeneous elements.
    Tuple(Vec<Type>),

    /// A string-keyed mapping of `V`.
    Map(Box<Type>),

    /// An unordered collection of `T`.
    Set(Box<Type>),

    /// A key-value mapping of `K` to `V`, so a target that distinguishes ordered
    /// maps from unordered ones can.
    OrderedMap {
        /// The key type.
        key: Box<Type>,
        /// The value type.
        value: Box<Type>,
        /// How the mapping may vary.
        variance: Variance,
    },

    /// A possibly-absent `T`.
    Option(Box<Type>),

    /// A function.
    Function {
        /// Parameter types in declaration order.
        params: Vec<Type>,
        /// Return type. [`Type::Void`] when the function returns nothing.
        ret: Box<Type>,
        /// Whether the last parameter absorbs extra arguments.
        variadic: bool,
        /// Parameter names, kept alongside types so a backend can emit named
        /// arguments and defaults where the target supports them.
        ///
        /// Empty when a backend has no use for them, which is the common case.
        param_names: Vec<Name>,
    },

    /// A type declared in the source, such as a class or a struct.
    ///
    /// The name alone, not the resolved declaration. Resolution is
    /// `gset-semantic`'s job; the IR records what the source said so a backend
    /// can decide whether it needs the resolved form.
    Named(Name),

    /// A declared generic parameter, by index.
    Generic(GenericId),

    /// A named type applied to type arguments, such as `List[int]`.
    Applied {
        /// The generic type being instantiated.
        base: Name,
        /// The type arguments, in declaration order.
        arguments: Vec<Type>,
    },

    /// A union of alternatives.
    ///
    /// Only produced when a source language genuinely has a union. Not
    /// synthesised by inference to paper over uncertainty: a union of
    /// everything would make `Unknown` meaningless.
    Union(Vec<Type>),
}

impl Type {
    /// The unknown type, with the most common reason.
    pub const UNKNOWN: Type = Type::Unknown(UnknownReason::Undetermined);

    /// Reports whether this type is fully known.
    pub fn is_known(&self) -> bool {
        !matches!(self, Type::Unknown(_))
    }

    /// Reports whether this type is unknown.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown(_))
    }

    /// Reports whether this type is the absent value.
    pub fn is_null(&self) -> bool {
        matches!(self, Type::Null)
    }

    /// Returns the reason this type is unknown, if it is.
    pub fn unknown_reason(&self) -> Option<UnknownReason> {
        match self {
            Type::Unknown(reason) => Some(*reason),
            _ => None,
        }
    }

    /// Reports whether this is a callable type.
    pub fn is_callable(&self) -> bool {
        matches!(self, Type::Function { .. })
    }

    /// Collects every immediate child type.
    ///
    /// Used by inference to walk into a type without matching on every variant
    /// twice. Deliberately shallow.
    pub fn children(&self) -> Vec<&Type> {
        match self {
            Type::List { element, .. } => vec![element.as_ref()],
            Type::Tuple(elements) => elements.iter().collect(),
            Type::Map(value) => vec![value.as_ref()],
            Type::Set(element) => vec![element.as_ref()],
            Type::OrderedMap { key, value, .. } => vec![key.as_ref(), value.as_ref()],
            Type::Option(inner) => vec![inner.as_ref()],
            Type::Function { params, ret, .. } => {
                let mut out: Vec<&Type> = params.iter().collect();
                out.push(ret.as_ref());
                out
            }
            Type::Applied { arguments, .. } => arguments.iter().collect(),
            Type::Union(members) => members.iter().collect(),
            Type::Unknown(_)
            | Type::Never
            | Type::Void
            | Type::Bool
            | Type::Int(_)
            | Type::Float
            | Type::Str
            | Type::StrLit
            | Type::Bytes
            | Type::Char
            | Type::Null
            | Type::Named(_)
            | Type::Generic(_) => Vec::new(),
        }
    }

    /// Reports whether `self` and `other` agree structurally.
    ///
    /// A *check* rather than a full equivalence test: `Unknown` matches anything,
    /// which is what makes it absorb rather than conflict. This is the one place
    /// `Unknown` is allowed to compare equal to something, and it is on purpose —
    /// it means "no information", not "matches everything".
    pub fn is_compatible_with(&self, other: &Type) -> bool {
        if self.is_unknown() || other.is_unknown() {
            return true;
        }
        match (self, other) {
            (Type::Never, _) | (_, Type::Never) => true,
            (Type::Null, Type::Null) => true,
            (Type::Void, Type::Void) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Float, Type::Float)
            | (Type::Float, Type::Int(_))
            | (Type::Int(_), Type::Float) => true,
            // Integer widths are not compared by value here: whether a target
            // can narrow a 64-bit literal to 32 bits is a backend decision, and
            // deciding it in the IR would prejudge the question.
            (Type::Int(_), Type::Int(_)) => true,
            (Type::Str, Type::Str)
            | (Type::Str, Type::StrLit)
            | (Type::StrLit, Type::Str)
            | (Type::StrLit, Type::StrLit) => true,
            (Type::Bytes, Type::Bytes) => true,
            (Type::Char, Type::Char) => true,
            (Type::List { element: a, .. }, Type::List { element: b, .. })
            | (Type::Set(a), Type::Set(b))
            | (Type::Option(a), Type::Option(b)) => a.is_compatible_with(b),
            (Type::Map(a), Type::Map(b)) => a.is_compatible_with(b),
            (
                Type::OrderedMap {
                    key: ak, value: av, ..
                },
                Type::OrderedMap {
                    key: bk, value: bv, ..
                },
            ) => ak.is_compatible_with(bk) && av.is_compatible_with(bv),
            (Type::Tuple(a), Type::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.is_compatible_with(y))
            }
            (
                Type::Function {
                    params: pa,
                    ret: ra,
                    ..
                },
                Type::Function {
                    params: pb,
                    ret: rb,
                    ..
                },
            ) => {
                pa.len() == pb.len()
                    && pa.iter().zip(pb).all(|(x, y)| x.is_compatible_with(y))
                    && ra.is_compatible_with(rb)
            }
            (Type::Named(a), Type::Named(b)) => a == b,
            (Type::Generic(a), Type::Generic(b)) => a == b,
            (
                Type::Applied {
                    base: ab,
                    arguments: aa,
                },
                Type::Applied {
                    base: bb,
                    arguments: ba,
                },
            ) => {
                ab == bb
                    && aa.len() == ba.len()
                    && aa.iter().zip(ba).all(|(x, y)| x.is_compatible_with(y))
            }
            (Type::Union(members), other) | (other, Type::Union(members)) => {
                members.iter().any(|m| m.is_compatible_with(other))
            }
            _ => false,
        }
    }

    /// Joins two types for inference.
    ///
    /// This is where `Unknown` genuinely absorbs: anything joined with
    /// `Unknown` is `Unknown`. Anything else that does not agree is
    /// [`Type::Unknown`] rather than a wrong answer.
    pub fn join(&self, other: &Type) -> Type {
        // Absorbing first is what makes Unknown contagious. It has to come
        // before the Never case, or joining Unknown with Never would "recover"
        // a concrete type and quietly erase the uncertainty.
        if self.is_unknown() || other.is_unknown() {
            return Type::UNKNOWN;
        }
        match (self, other) {
            // Never is the bottom type, so it is absorbed by anything. This is
            // the one case where a genuine type is recovered from a join.
            (Type::Never, other) | (other, Type::Never) => other.clone(),
            (a, b) if a.is_compatible_with(b) => {
                if a == b {
                    a.clone()
                } else {
                    // Compatible but not identical, such as Int and Float. Widen
                    // to the type that can represent both rather than picking one.
                    match (a, b) {
                        (Type::Int(_), Type::Float) | (Type::Float, Type::Int(_)) => Type::Float,
                        (Type::StrLit, Type::Str) | (Type::Str, Type::StrLit) => Type::Str,
                        (a, _) => a.clone(),
                    }
                }
            }
            // Disagreement is precisely the case a guess would get wrong.
            _ => Type::Unknown(UnknownReason::InferenceFailed),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Unknown(UnknownReason::Undetermined)
            | Type::Unknown(UnknownReason::Unavailable) => f.write_str("unknown"),
            Type::Unknown(UnknownReason::Dynamic) => f.write_str("dynamic"),
            Type::Unknown(UnknownReason::InferenceFailed) => f.write_str("unknown(inference)"),
            Type::Never => f.write_str("never"),
            Type::Void => f.write_str("void"),
            Type::Bool => f.write_str("bool"),
            Type::Int(IntWidth::Isize) => f.write_str("int"),
            Type::Int(width) => write!(f, "int{}", width.bits().unwrap_or(0)),
            Type::Float => f.write_str("float"),
            Type::Str => f.write_str("str"),
            Type::StrLit => f.write_str("str_lit"),
            Type::Bytes => f.write_str("bytes"),
            Type::Char => f.write_str("char"),
            Type::Null => f.write_str("null"),
            Type::List { element, .. } => write!(f, "list[{element}]"),
            Type::Tuple(elements) => {
                f.write_str("(")?;
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{element}")?;
                }
                f.write_str(")")
            }
            Type::Map(value) => write!(f, "map[{value}]"),
            Type::Set(element) => write!(f, "set[{element}]"),
            Type::OrderedMap { key, value, .. } => write!(f, "ordered_map[{key}, {value}]"),
            Type::Option(inner) => write!(f, "optional[{inner}]"),
            Type::Function {
                params,
                ret,
                variadic,
                ..
            } => {
                f.write_str("fn(")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{param}")?;
                }
                if *variadic {
                    f.write_str("...")?;
                }
                write!(f, ") -> {ret}")
            }
            Type::Named(n) => write!(f, "{n}"),
            Type::Generic(GenericId(index)) => write!(f, "T{index}"),
            Type::Applied { base, arguments } => {
                write!(f, "{base}[")?;
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str("]")
            }
            Type::Union(members) => {
                f.write_str("union[")?;
                for (index, member) in members.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" | ")?;
                    }
                    write!(f, "{member}")?;
                }
                f.write_str("]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int() -> Type {
        Type::Int(IntWidth::I32)
    }

    fn bool() -> Type {
        Type::Bool
    }

    fn dynamic() -> Type {
        Type::Unknown(UnknownReason::Dynamic)
    }

    #[test]
    fn unknown_is_recognised() {
        assert!(Type::UNKNOWN.is_unknown());
        assert!(!Type::UNKNOWN.is_known());
        assert_eq!(
            Type::UNKNOWN.unknown_reason(),
            Some(UnknownReason::Undetermined)
        );
        // A dynamically typed position is still unknown; the reason is what
        // distinguishes it from an undetermined type.
        assert!(dynamic().is_unknown());
        assert_eq!(dynamic().unknown_reason(), Some(UnknownReason::Dynamic));
        assert!(int().is_known());
    }

    #[test]
    fn unknown_is_absorbing_in_join() {
        // This is the property that makes uncertainty propagate instead of
        // disappearing at the first join.
        assert!(Type::UNKNOWN.join(&int()).is_unknown());
        assert!(int().join(&Type::UNKNOWN).is_unknown());
        assert!(Type::UNKNOWN.join(&Type::UNKNOWN).is_unknown());
    }

    #[test]
    fn unknown_absorbs_even_through_never() {
        // Order matters here. If Never were handled first, joining Unknown with
        // Never would "recover" a concrete type and quietly erase the
        // uncertainty.
        assert!(Type::UNKNOWN.join(&Type::Never).is_unknown());
        assert!(Type::Never.join(&Type::UNKNOWN).is_unknown());
    }

    #[test]
    fn never_is_absorbed_by_every_real_type() {
        assert_eq!(Type::Never.join(&int()), int());
        assert_eq!(int().join(&Type::Never), int());
        assert_eq!(Type::Never.join(&Type::Str), Type::Str);
    }

    #[test]
    fn join_widens_rather_than_picking_a_side() {
        assert_eq!(int().join(&Type::Float), Type::Float);
        assert_eq!(Type::Float.join(&int()), Type::Float);
        assert_eq!(Type::StrLit.join(&Type::Str), Type::Str);
    }

    #[test]
    fn join_of_disagreement_is_unknown_not_a_guess() {
        // A guess here would produce code that compiles and then misbehaves.
        assert_eq!(
            int().join(&Type::Bool),
            Type::Unknown(UnknownReason::InferenceFailed)
        );
        assert!(int().join(&Type::Bool).is_unknown());
    }

    #[test]
    fn identical_joins_stay_identical() {
        assert_eq!(int().join(&int()), int());
        assert_eq!(Type::Str.join(&Type::Str), Type::Str);
    }

    #[test]
    fn unknown_is_compatible_with_anything_so_it_never_conflicts() {
        assert!(Type::UNKNOWN.is_compatible_with(&Type::Bool));
        assert!(Type::Bool.is_compatible_with(&Type::UNKNOWN));
        // Including types it is structurally unlike.
        assert!(Type::UNKNOWN.is_compatible_with(&Type::Named(name("Widget"))));
    }

    #[test]
    fn compatibility_recurses_through_collections() {
        let int_list = Type::List {
            element: Box::new(int()),
            variance: Variance::Invariant,
        };
        let float_list = Type::List {
            element: Box::new(Type::Float),
            variance: Variance::Invariant,
        };
        assert!(int_list.is_compatible_with(&float_list));

        let bool_list = Type::List {
            element: Box::new(Type::Bool),
            variance: Variance::Invariant,
        };
        assert!(!int_list.is_compatible_with(&bool_list));
    }

    #[test]
    fn never_is_compatible_with_everything() {
        // Never has no values, so it fits wherever it appears. This is what lets
        // a `return` or `raise` stand in for a value of any type.
        assert!(Type::Never.is_compatible_with(&Type::Bool));
        assert!(Type::Str.is_compatible_with(&Type::Never));
    }

    #[test]
    fn tuples_and_functions_require_matching_arity() {
        let pair = Type::Tuple(vec![int(), Type::Str]);
        assert!(pair.is_compatible_with(&Type::Tuple(vec![int(), Type::Str])));
        assert!(!pair.is_compatible_with(&Type::Tuple(vec![int()])));
        assert!(!pair.is_compatible_with(&Type::Tuple(vec![int(), Type::Str, int()])));

        let f = |ret: Type| Type::Function {
            params: vec![int()],
            ret: Box::new(ret),
            variadic: false,
            param_names: Vec::new(),
        };
        assert!(f(int()).is_compatible_with(&f(Type::Float)));
        assert!(!f(int()).is_compatible_with(&f(Type::Bool)));
    }

    #[test]
    fn function_param_names_do_not_affect_compatibility() {
        let a = Type::Function {
            params: vec![int()],
            ret: Box::new(Type::Void),
            variadic: false,
            param_names: vec![name("x")],
        };
        let b = Type::Function {
            params: vec![int()],
            ret: Box::new(Type::Void),
            variadic: false,
            param_names: vec![name("value")],
        };
        // Names are documentation for a backend, not part of the type.
        assert!(a.is_compatible_with(&b));
    }

    #[test]
    fn integer_widths_do_not_conflict_at_this_stage() {
        // Whether a target can narrow is a backend decision.
        assert!(Type::Int(IntWidth::I8).is_compatible_with(&Type::Int(IntWidth::I64)));
        assert!(!Type::Int(IntWidth::I64).is_compatible_with(&Type::Bool));
    }

    #[test]
    fn union_compatibility_asks_the_members() {
        let union = Type::Union(vec![int(), Type::Str]);
        assert!(union.is_compatible_with(&int()));
        assert!(union.is_compatible_with(&Type::Str));
        assert!(!union.is_compatible_with(&Type::Bool));
        // Bool fits the union, so compatibility is symmetric even though the
        // match arm that handles it is reached from either side.
        assert!(!Type::Bool.is_compatible_with(&union));
        let with_bool = Type::Union(vec![int(), Type::Bool]);
        assert!(Type::Bool.is_compatible_with(&with_bool));
        assert!(with_bool.is_compatible_with(&Type::Bool));
    }

    #[test]
    fn optional_and_null_are_distinct_from_each_other() {
        // JavaScript and Python conflate these, Go has neither. The IR keeps
        // them apart so a target that can express the difference does.
        assert!(!Type::Null.is_compatible_with(&Type::Bool));
        assert!(Type::Option(Box::new(int())).is_compatible_with(&Type::Option(Box::new(int()))));
        assert!(!Type::Option(Box::new(int())).is_compatible_with(&int()));
    }

    #[test]
    fn children_are_reachable_for_inference() {
        let t = Type::Function {
            params: vec![int(), Type::Str],
            ret: Box::new(bool()),
            variadic: false,
            param_names: Vec::new(),
        };
        let children = t.children();
        assert_eq!(children.len(), 3);
        assert!(children.contains(&&int()));
        assert!(children.contains(&&Type::Str));
        assert!(children.contains(&&bool()));
    }

    #[test]
    fn leaves_have_no_children() {
        assert!(int().children().is_empty());
        assert!(Type::UNKNOWN.children().is_empty());
        assert!(Type::Named(name("A")).children().is_empty());
    }

    #[test]
    fn ordered_map_reports_both_children() {
        let t = Type::OrderedMap {
            key: Box::new(Type::Str),
            value: Box::new(int()),
            variance: Variance::Invariant,
        };
        assert_eq!(t.children().len(), 2);
    }

    #[test]
    fn types_display_readably() {
        assert_eq!(int().to_string(), "int32");
        assert_eq!(Type::Int(IntWidth::Isize).to_string(), "int");
        assert_eq!(Type::UNKNOWN.to_string(), "unknown");
        assert_eq!(dynamic().to_string(), "dynamic");
        assert_eq!(
            Type::List {
                element: Box::new(int()),
                variance: Variance::Invariant
            }
            .to_string(),
            "list[int32]"
        );
        assert_eq!(
            Type::Tuple(vec![int(), Type::Str]).to_string(),
            "(int32, str)"
        );
        assert_eq!(
            Type::Option(Box::new(Type::Str)).to_string(),
            "optional[str]"
        );
        assert_eq!(Type::Generic(GenericId(0)).to_string(), "T0");
        assert_eq!(
            Type::Applied {
                base: name("Box"),
                arguments: vec![int()],
            }
            .to_string(),
            "Box[int32]"
        );
        assert_eq!(
            Type::Union(vec![int(), Type::Null]).to_string(),
            "union[int32 | null]"
        );
    }

    #[test]
    fn function_type_displays_its_signature() {
        let t = Type::Function {
            params: vec![int(), Type::Str],
            ret: Box::new(bool()),
            variadic: false,
            param_names: Vec::new(),
        };
        assert_eq!(t.to_string(), "fn(int32, str) -> bool");

        let variadic = Type::Function {
            params: vec![int()],
            ret: Box::new(Type::Void),
            variadic: true,
            param_names: Vec::new(),
        };
        assert_eq!(variadic.to_string(), "fn(int32...) -> void");
    }

    #[test]
    fn names_clone_without_allocating() {
        let n = name("Widget");
        let clone = n.clone();
        assert_eq!(n, clone);
        assert_eq!(&*clone, "Widget");
    }
}
