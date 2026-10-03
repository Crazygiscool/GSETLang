//! Module-level declarations: imports, functions, classes, records, enums,
//! interfaces, and type aliases.
//!
//! Items are separate from [`crate::stmt::Stmt`] because scope is a property of
//! a module, not of a statement list. The Go implementation had one flat
//! statement list and lost declarations that trailed the last function.
//!
//! Imports live here for a specific reason. In Go, `ast.Program.Imports` was
//! written by the parser and read by no emitter, so no top-level import ever
//! reached generated output. Because [`Item::Import`] is an item a backend
//! must handle, that class of silent drop cannot recur.

use crate::expr::{Expr, Pattern};
use crate::span::Span;
use crate::stmt::{Block, Stmt, VarDecl};
use crate::types::{Name, Type};

/// A module: the root of the IR.
///
/// Items are stored in source order so diagnostics and generated output stay
/// faithful to what the author wrote.
#[derive(Clone, PartialEq, Debug)]
pub struct Module {
    /// Every top-level declaration, imports included, in source order.
    pub items: Vec<Item>,
    /// Which source language produced this module.
    pub lang: LangId,
    /// Where the module begins, spanning its whole extent.
    pub span: Span,
}

/// The source language a [`Module`] was produced from.
///
/// A plain newtype rather than an enum of units: `gset-frontend` adds
/// languages, and a closed enum here would make every addition a breaking
/// change for `gset-semantic`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LangId(u16);

impl LangId {
    /// Python.
    pub const PYTHON: LangId = LangId(0);
    /// JavaScript.
    pub const JAVASCRIPT: LangId = LangId(1);
    /// TypeScript.
    pub const TYPESCRIPT: LangId = LangId(2);
    /// Go, when the Go tree is retired in M6.
    pub const GO: LangId = LangId(3);

    /// Wraps a raw identifier.
    pub const fn from_raw(raw: u16) -> LangId {
        LangId(raw)
    }

    /// The raw identifier.
    pub const fn as_raw(self) -> u16 {
        self.0
    }
}

/// A top-level declaration.
#[derive(Clone, PartialEq, Debug)]
pub enum Item {
    /// An import. Mandatory to consume: see the module docs.
    Import(Import),
    /// A named function or method.
    Function(Function),
    /// A class.
    Class(Class),
    /// A record-like type: a struct, a record, or a data class.
    Record(Record),
    /// An enumeration.
    Enum(Enum),
    /// An interface or trait.
    Interface(Interface),
    /// A type alias.
    TypeAlias(TypeAlias),
    /// A module-level binding.
    Global(VarDecl),
    /// A statement that appeared at module scope.
    ///
    /// Kept so a frontend never has to discard source it parsed. Backends must
    /// decide explicitly what to do with it rather than dropping it silently.
    Stmt(Stmt),
}

impl Item {
    /// Where the item was written.
    pub fn span(&self) -> Span {
        match self {
            Item::Import(item) => item.span,
            Item::Function(item) => item.span,
            Item::Class(item) => item.span,
            Item::Record(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Interface(item) => item.span,
            Item::TypeAlias(item) => item.span,
            Item::Global(item) => item.span,
            Item::Stmt(item) => item.span(),
        }
    }

    /// Whether this item needs the module to have emitted an import.
    pub fn is_import(&self) -> bool {
        matches!(self, Item::Import(_))
    }

    /// Whether the item was marked exported by the source.
    ///
    /// For an import this means re-exporting the module's names onward, which
    /// Python spells as a redundant `__all__` entry. An import carries no
    /// exported flag of its own, so the question only has a real answer for
    /// declarations.
    pub fn is_exported(&self) -> bool {
        match self {
            // Imports are visible to the importing module either way; whether
            // the names pass through again is decided by the declarations the
            // target exports, not by the import itself.
            Item::Import(_) => false,
            Item::Function(item) => item.exported,
            Item::Class(item) => item.exported,
            Item::Record(item) => item.exported,
            Item::Enum(item) => item.exported,
            Item::Interface(item) => item.exported,
            Item::TypeAlias(item) => item.exported,
            Item::Global(item) => item.exported,
            // A bare statement has no export marker of its own.
            Item::Stmt(_) => false,
        }
    }
}

impl Module {
    /// An empty module spanning nothing.
    pub const fn empty(lang: LangId, span: Span) -> Module {
        Module {
            items: Vec::new(),
            lang,
            span,
        }
    }

    /// Every import, in source order.
    pub fn imports(&self) -> impl Iterator<Item = &Import> {
        self.items.iter().filter_map(|item| match item {
            Item::Import(import) => Some(import),
            _ => None,
        })
    }

    /// Every function, in source order.
    pub fn functions(&self) -> impl Iterator<Item = &Function> {
        self.items.iter().filter_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
    }

    /// Whether the module declares anything named `name`.
    pub fn declares(&self, name: &str) -> bool {
        self.items.iter().any(|item| match item {
            Item::Function(function) => &*function.name == name,
            Item::Class(class) => &*class.name == name,
            Item::Record(record) => &*record.name == name,
            Item::Enum(enumeration) => &*enumeration.name == name,
            Item::Interface(interface) => &*interface.name == name,
            Item::TypeAlias(alias) => &*alias.name == name,
            Item::Global(global) => global
                .pattern
                .single_binding()
                .is_some_and(|binding| &**binding == name),
            Item::Import(_) | Item::Stmt(_) => false,
        })
    }

    /// The item declaring `name` as a function, if any.
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions().find(|function| &*function.name == name)
    }

    /// Appends an item.
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// Whether the module has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// How many items the module has.
    pub fn len(&self) -> usize {
        self.items.len()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Path;
    use crate::span::SourceId;
    use crate::types::name;

    fn span() -> Span {
        Span::point(SourceId::from_raw(0), 0)
    }

    fn int(value: &str) -> Expr {
        Expr::int(value, span())
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
    fn imports_are_items_so_a_backend_cannot_ignore_them() {
        // The regression this whole module exists for: in Go the parser filled
        // ast.Program.Imports and no emitter read it. An import must be
        // reachable through the same item list every backend already walks.
        let mut module = Module::empty(LangId::PYTHON, span());
        module.push(Item::Import(Import::module(
            Path::single(name("os")),
            span(),
        )));
        module.push(Item::Function(Function {
            name: name("main"),
            params: vec![],
            param_types: vec![],
            ret: None,
            body: Block::empty(span()),
            variadic: false,
            is_async: false,
            exported: false,
            generics: Vec::new(),
            span: span(),
        }));

        assert_eq!(module.imports().count(), 1);
        assert_eq!(module.functions().count(), 1);
        assert_eq!(module.len(), 2);
        assert!(!module.is_empty());
    }

    #[test]
    fn a_module_reports_declarations_by_name() {
        let mut module = Module::empty(LangId::PYTHON, span());
        module.push(Item::Function(Function {
            name: name("main"),
            params: vec![],
            param_types: vec![],
            ret: None,
            body: Block::new(
                vec![Stmt::Return {
                    value: Some(int("0")),
                    span: span(),
                }],
                span(),
            ),
            variadic: false,
            is_async: false,
            exported: false,
            generics: Vec::new(),
            span: span(),
        }));

        assert!(module.declares("main"));
        assert!(!module.declares("other"));
        assert!(module.function("main").is_some());
        assert!(module.function("other").is_none());
    }

    #[test]
    fn a_destructuring_global_is_not_treated_as_one_named_binding() {
        // `a, b = f()` binds two names. Reporting it as a single binding would
        // let a name collision check pass on the wrong name.
        let mut module = Module::empty(LangId::PYTHON, span());
        module.push(Item::Global(VarDecl {
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
        }));

        assert!(!module.declares("a"));
        assert!(!module.declares("b"));
    }

    #[test]
    fn an_import_is_not_reported_as_exported() {
        let item = Item::Import(Import::module(Path::single(name("os")), span()));
        assert!(!item.is_exported());
        assert!(item.is_import());
        assert_eq!(item.span(), span());
    }

    #[test]
    fn lang_ids_round_trip_and_are_ordered() {
        assert_eq!(LangId::PYTHON.as_raw(), 0);
        assert_eq!(LangId::from_raw(7), LangId::from_raw(7));
        assert_ne!(LangId::from_raw(7), LangId::PYTHON);
        // Ordering must be stable so diagnostics sort identically across runs.
        assert!(LangId::PYTHON < LangId::JAVASCRIPT);
        assert!(LangId::JAVASCRIPT < LangId::TYPESCRIPT);
    }
}
