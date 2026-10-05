//! Lowers a Python syntax tree into the IR.
//!
//! Two rules run through the whole file.
//!
//! First, nothing is dropped. Go lost a whole struct when a class and an enum
//! shared a file, lost every top-level import because no emitter read them, lost
//! a trailing statement into the function above it, and turned `??` into `||`
//! because it looked for a token no target mapping had. Each of those was a
//! silent omission. Here a construct that cannot be represented becomes both a
//! [`Diagnostic`] and an error placeholder in the tree, so it cannot be lost
//! without also being reported.
//!
//! Second, no inference. An annotation the source wrote down becomes a [`Type`];
//! one it did not stays `None`, and every expression starts at
//! [`Type::UNKNOWN`]. Guessing here would move the guessing somewhere it cannot
//! be tested.

use gset_ir::{
    Block, Builder, CatchClause, Class, ComparisonOp, ComprehensionKind, Decorator, Diagnostic,
    Else, Expr, ExprKind, Function, GeneratorClause, Import, ImportedName, IntWidth, Item, LangId,
    Literal, LogicalOp, Lowered, MatchCase, Name, NamedArg, Path, Pattern, SourceId, SourceMap,
    Span, Stmt, Type, UnaryOp, VarDecl, Variance,
};

use super::parse;

/// Lowers `text` as Python, naming it `name` in every span and diagnostic.
pub(crate) fn lower(name: &str, text: &str) -> Lowered {
    let mut source_map = SourceMap::new();
    let source = source_map.add(name, text);

    let mut builder = Builder::new(LangId::PYTHON, Span::new(source, 0, text.len() as u32));
    builder.set_source_map(source_map);

    // A parse failure leaves no tree to walk. Reporting it and finishing with
    // `module: None` keeps the failure in the same channel as every other
    // problem, rather than inventing an empty module that looks valid.
    let parsed = match parse(source, text) {
        Ok(parsed) => parsed,
        Err(diagnostic) => {
            builder.report(diagnostic);
            return builder.finish();
        }
    };
    for diagnostic in parsed.diagnostics {
        builder.report(diagnostic);
    }

    let mut lowerer = Lowerer {
        text,
        source,
        builder,
        fn_depth: 0,
        loop_depth: 0,
    };
    lowerer.module(parsed.tree.root_node());

    lowerer.builder.finish()
}

/// Holds the state one lowering run needs.
///
/// The depths are load-bearing. `break` outside a loop and `yield` outside a
/// function are both errors, and the builder cannot judge either on its own
/// because it validates a statement without knowing what encloses it.
struct Lowerer<'a> {
    text: &'a str,
    source: SourceId,
    builder: Builder,
    fn_depth: u32,
    loop_depth: u32,
}

impl<'a> Lowerer<'a> {
    /// Lowers the whole module.
    fn module(&mut self, root: tree_sitter::Node<'_>) {
        for node in named_children(root) {
            self.item(node);
        }
    }

    /// The span of a node.
    fn span(&self, node: tree_sitter::Node<'_>) -> Span {
        Span::new(
            self.source,
            node.start_byte() as u32,
            node.end_byte() as u32,
        )
    }

    /// The source text a node covers.
    fn text_of(&self, node: tree_sitter::Node<'_>) -> &'a str {
        self.text.get(node.byte_range()).unwrap_or("")
    }

    /// The name a node denotes.
    ///
    /// Reads the identifier's own text rather than trimming the node, so a
    /// grammar node that covers more than the name does not smuggle punctuation
    /// into a symbol table.
    fn name_of(&self, node: tree_sitter::Node<'_>) -> gset_ir::Name {
        gset_ir::name(self.text_of(node).trim())
    }

    /// Reports a construct that has no representation.
    fn unsupported(&mut self, node: tree_sitter::Node<'_>, what: &str) {
        let found = self.text_of(node);
        let snippet: String = found.chars().take(40).collect();
        self.builder.report(
            Diagnostic::error(
                self.span(node),
                format!(
                    "cannot lower {what} `{}`: it has no equivalent in the IR, and dropping it \
                     silently would change what the program does",
                    snippet.trim()
                ),
            )
            .with_code("gset-python-unsupported"),
        );
    }

    // ---------------------------------------------------------------- items

    /// Lowers one top-level node.
    fn item(&mut self, node: tree_sitter::Node<'_>) {
        match node.kind() {
            "import_statement" | "import_from_statement" | "future_import_statement" => {
                let import = self.import(node);
                self.builder.item(Item::Import(import));
            }
            "decorated_definition" => match node.child_by_field_name("definition") {
                Some(definition) => self.item(definition),
                None => self.unsupported(node, "decorated definition"),
            },
            "function_definition" => {
                let mut function = self.function(node);
                function.decorators = Vec::new();
                self.builder.item(Item::Function(function));
            }
            "class_definition" => {
                let mut class = self.class(node);
                class.decorators = Vec::new();
                self.builder.item(Item::Class(class));
            }
            // A binding at module scope is a module-level binding. Keeping it an
            // item rather than a statement is what stops it being absorbed into
            // the function above it, which is Go defect class 14.
            "expression_statement" if child_of_kind(node, "assignment").is_some() => {
                self.module_binding(node);
            }
            "expression_statement"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "try_statement"
            | "with_statement"
            | "match_statement"
            | "return_statement"
            | "raise_statement"
            | "assert_statement"
            | "delete_statement"
            | "break_statement"
            | "continue_statement" => {
                for statement in self.statement(node) {
                    self.builder.stmt(statement);
                }
            }
            "pass_statement" => {
                self.builder.stmt(Stmt::Empty {
                    span: self.span(node),
                });
            }
            _ => self.unsupported(node, "module-level construct"),
        }
    }

    /// Lowers a module-level `x = 1` or `x: int = 1`.
    fn module_binding(&mut self, node: tree_sitter::Node<'_>) {
        let assignment = child_of_kind(node, "assignment").expect("checked by the caller");
        let pattern = self.assignment_target(assignment.child_by_field_name("left"));
        let ty = self.declared_type(assignment);
        let value = match assignment.child_by_field_name("right") {
            Some(right) => self.expr(right),
            None => Expr::error("assignment has no value", self.span(assignment)),
        };
        self.builder.global(VarDecl {
            pattern,
            ty,
            value: Some(value),
            mutable: true,
            exported: false,
            span: self.span(node),
        });
    }

    /// The annotation on an assignment, if it has one.
    fn declared_type(&mut self, assignment: tree_sitter::Node<'_>) -> Option<Type> {
        assignment
            .child_by_field_name("type")
            .map(|node| self.type_of(node))
    }

    // --------------------------------------------------------------- imports

    /// Lowers an import of any form.
    fn import(&mut self, node: tree_sitter::Node<'_>) -> Import {
        let span = self.span(node);
        match node.kind() {
            "import_statement" => self.plain_import(node, span),
            _ => self.import_from(node, span),
        }
    }

    /// Lowers `import a`, `import a.b`, `import a.b as c`.
    fn plain_import(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Import {
        let first = child_of_kind(node, "dotted_name")
            .or_else(|| child_of_kind(node, "aliased_import"))
            .unwrap_or(node);

        let (path_node, alias_node) = if first.kind() == "aliased_import" {
            (
                first.child_by_field_name("name"),
                first.child_by_field_name("alias"),
            )
        } else {
            (Some(first), None)
        };

        let path = path_node
            .map(|n| self.dotted_name(n, span))
            .unwrap_or_else(|| Path::new(Vec::new()));
        let alias = alias_node.map(|n| self.name_of(n));

        let mut import = Import::module(path, span);
        if let Some(alias) = alias {
            import = import.with_alias(alias);
        }
        import
    }

    /// Lowers `from a.b import c`, `from . import c`, `from a import *`.
    fn import_from(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Import {
        if node.kind() == "future_import_statement" {
            // `from __future__ import x` names a directive of the source
            // language itself. No target has one, so it has nowhere to go. A
            // warning rather than an error because `annotations` only changes how
            // the source interpreter reads the file, and generated code has no
            // such pass to affect.
            self.builder.report(
                Diagnostic::warning(
                    span,
                    "`from __future__ import` names a directive of the Python interpreter \
                     itself and was dropped; generated code has no equivalent",
                )
                .with_code("gset-python-future"),
            );
        }

        let path = self.import_source(node, span);
        let names = self.imported_names(node, span);
        Import::named(path, names, span)
    }

    /// The module an import names.
    ///
    /// A relative import's `import_prefix` is dropped: which module `..sibling`
    /// resolves to depends on the package layout, which is `gset-deps`' question.
    /// Inventing an absolute path here would be a guess that silently names the
    /// wrong module.
    fn import_source(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Path {
        // `from __future__ import x` has no `module_name` field: the grammar
        // gives it its own node kind. It is dropped with a warning by
        // `import_from`, so the synthetic path only keeps the builder's
        // non-empty invariant satisfied.
        if node.kind() == "future_import_statement" {
            return Path::new(vec![gset_ir::name("__future__")]);
        }

        let Some(name) = node.child_by_field_name("module_name") else {
            self.builder.report(
                Diagnostic::error(span, "import names no module").with_code("gset-python-import"),
            );
            return Path::new(Vec::new());
        };

        if name.kind() == "relative_import" {
            let dots = child_of_kind(name, "import_prefix")
                .map(|n| self.text_of(n).trim().to_string())
                .unwrap_or_default();
            self.builder.report(
                Diagnostic::warning(
                    span,
                    format!(
                        "relative import `{dots}` is not resolved to an absolute module; \
                         gset-deps must resolve it before a backend can emit it"
                    ),
                )
                .with_code("gset-python-relative"),
            );
            return match child_of_kind(name, "dotted_name") {
                Some(dotted) => self.dotted_name(dotted, span),
                // `from . import x` has no dotted name at all. The leading dots
                // are the whole module reference, so keep them as the path
                // rather than producing an empty one.
                None => Path::new(vec![gset_ir::name(dots)]),
            };
        }

        self.dotted_name(name, span)
    }

    /// The names a `from` import brings into scope.
    fn imported_names(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Vec<ImportedName> {
        let container = child_of_kind(node, "import_list").unwrap_or(node);
        let module_name = node.child_by_field_name("module_name");
        let mut names = Vec::new();

        for child in named_children(container) {
            // The module being imported from is a child too; it is the path, not
            // one of the imported names.
            if module_name.is_some_and(|name| name.id() == child.id()) {
                continue;
            }
            match child.kind() {
                "dotted_name" | "identifier" => {
                    let path = self.dotted_name(child, span);
                    let bound = path.segments.last().cloned().unwrap_or_default();
                    names.push(ImportedName::new(bound));
                }
                "aliased_import" => {
                    let original = child
                        .child_by_field_name("name")
                        .map(|n| self.dotted_name(n, span))
                        .unwrap_or_else(|| Path::new(Vec::new()));
                    let bound = original.segments.last().cloned().unwrap_or_default();
                    let alias = child
                        .child_by_field_name("alias")
                        .map(|n| self.name_of(n))
                        .unwrap_or_default();
                    names.push(ImportedName::new(bound).as_alias(alias));
                }
                "wildcard_import" => {
                    // A star import has no target-language equivalent derivable
                    // from the source: naming the module's exports means resolving
                    // the module, which is `gset-deps`' job. Failing loudly beats
                    // emitting an import of nothing, which is exactly what Go did.
                    self.builder.report(
                        Diagnostic::error(
                            self.span(child),
                            "star import cannot be translated: the exporting module's names must \
                             be resolved first, and this IR import binds nothing",
                        )
                        .with_code("gset-python-star-import"),
                    );
                }
                other => self.unsupported(child, other),
            }
        }
        names
    }

    /// Lowers a dotted name into a [`Path`].
    fn dotted_name(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Path {
        let mut segments = Vec::new();
        for child in named_children(node) {
            if child.kind() == "identifier" {
                segments.push(self.name_of(child));
            }
        }
        if segments.is_empty() && node.kind() == "identifier" {
            segments.push(self.name_of(node));
        }
        if segments.is_empty() {
            self.builder.report(
                Diagnostic::error(span, "expected a dotted name").with_code("gset-python-name"),
            );
        }
        Path::new(segments)
    }

    // ---------------------------------------------------- functions, classes

    /// Lowers a `def`, including its parameters and body.
    ///
    /// Callers set `decorators` themselves, because a decorated definition wraps
    /// the `def` in a `decorated_definition` node and the decorators have to be
    /// collected before this runs.
    fn function(&mut self, node: tree_sitter::Node<'_>) -> Function {
        let span = self.span(node);
        let name = node
            .child_by_field_name("name")
            .map(|n| self.name_of(n))
            .unwrap_or_default();
        let is_async = child_of_kind(node, "async").is_some();
        let (params, param_types, variadic) = self.parameters(node);
        let ret = node
            .child_by_field_name("return_type")
            .map(|n| self.type_of(n));

        self.fn_depth += 1;
        let body = self.block(node.child_by_field_name("body"));
        self.fn_depth -= 1;

        Function {
            name,
            decorators: Vec::new(),
            params,
            param_types,
            ret,
            body,
            variadic,
            is_async,
            exported: false,
            generics: Vec::new(),
            span,
        }
    }

    /// Lowers a parameter list into patterns, annotations, and a variadic flag.
    ///
    /// `self` and `cls` stay ordinary parameters. Dropping them would renumber
    /// the rest, and the IR has no receiver concept: whether a target needs one
    /// written out is a target question.
    ///
    /// Default values are not recorded, because [`Function`] has nowhere to put
    /// them. Giving it a slot would make every backend answer whether its
    /// language can express a default; omitting them keeps that decision visible
    /// as a gap instead of silently losing `b=DEFAULT`.
    fn parameters(
        &mut self,
        node: tree_sitter::Node<'_>,
    ) -> (Vec<Pattern>, Vec<Option<Type>>, bool) {
        let mut params = Vec::new();
        let mut types = Vec::new();
        let mut variadic = false;

        let Some(list) = node.child_by_field_name("parameters") else {
            return (params, types, variadic);
        };
        for child in named_children(list) {
            match child.kind() {
                "identifier" => {
                    params.push(Pattern::bind(self.name_of(child), self.span(child)));
                    types.push(None);
                }
                "typed_parameter" => {
                    params.push(self.parameter_name(child));
                    types.push(child.child_by_field_name("type").map(|n| self.type_of(n)));
                }
                "default_parameter" | "typed_default_parameter" => {
                    params.push(self.parameter_name(child));
                    types.push(child.child_by_field_name("type").map(|n| self.type_of(n)));
                }
                "list_splat_pattern" | "dictionary_splat_pattern" => {
                    params.push(self.parameter_name(child));
                    types.push(None);
                    variadic = true;
                }
                // The `*` that separates keyword-only parameters, and the `/`
                // that marks positional-only ones. Both name nothing; the
                // distinction is which parameters precede them, and the IR's
                // single `variadic` flag cannot carry it.
                "keyword_separator" | "positional_separator" => {}
                other => self.unsupported(child, other),
            }
        }
        (params, types, variadic)
    }

    /// The name of a parameter, whatever form it takes.
    fn parameter_name(&self, node: tree_sitter::Node<'_>) -> Pattern {
        let name = node
            .child_by_field_name("name")
            .or_else(|| {
                node.named_child(0)
                    .filter(|n| matches!(n.kind(), "identifier" | "_"))
            })
            .map(|n| self.name_of(n))
            .unwrap_or_default();
        Pattern::bind(name, self.span(node))
    }

    /// Lowers a `class`.
    fn class(&mut self, node: tree_sitter::Node<'_>) -> Class {
        let span = self.span(node);
        let name = node
            .child_by_field_name("name")
            .map(|n| self.name_of(n))
            .unwrap_or_default();

        // Python's bases are one list; the first is the superclass and the rest
        // are mixins, which is the distinction every target cares about.
        let mut extends = None;
        let mut implements = Vec::new();
        if let Some(list) = node.child_by_field_name("superclasses") {
            for child in named_children(list) {
                match child.kind() {
                    "identifier" | "dotted_name" => {
                        let path = self.dotted_name(child, span);
                        if extends.is_none() {
                            extends = Some(path);
                        } else {
                            implements.push(path);
                        }
                    }
                    "keyword_argument" => {
                        let key = child
                            .child_by_field_name("name")
                            .map(|n| self.name_of(n))
                            .unwrap_or_default();
                        // `metaclass=` decides how the class is built. The IR's
                        // `Class` has nowhere to put it, and dropping it yields a
                        // class constructed the wrong way.
                        self.builder.report(
                            Diagnostic::error(
                                self.span(child),
                                format!(
                                    "class keyword `{key}=` has no place in the IR; a class that \
                                     depends on it cannot be translated"
                                ),
                            )
                            .with_code("gset-python-class-keyword"),
                        );
                    }
                    other => self.unsupported(child, other),
                }
            }
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut constructors = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            for child in named_children(body) {
                match child.kind() {
                    "expression_statement" => {
                        if let Some(field) = self.class_field(child) {
                            fields.push(field);
                        }
                    }
                    "function_definition" => {
                        let method = self.function(child);
                        push_method(&mut methods, &mut constructors, method);
                    }
                    "decorated_definition" => {
                        let Some(definition) = child.child_by_field_name("definition") else {
                            continue;
                        };
                        let decorators = self.decorators(child);
                        if definition.kind() == "function_definition" {
                            let mut method = self.function(definition);
                            method.decorators = decorators;
                            push_method(&mut methods, &mut constructors, method);
                        } else {
                            self.unsupported(definition, "decorated class member");
                        }
                    }
                    "pass_statement" => {}
                    other => self.unsupported(child, other),
                }
            }
        }

        Class {
            name,
            decorators: Vec::new(),
            extends,
            implements,
            fields,
            methods,
            constructors,
            exported: false,
            generics: Vec::new(),
            span,
        }
    }

    /// Lowers one `x: int = 0` in a class body into a field.
    ///
    /// Returns `None` for a body statement that is not a field, so a class-level
    /// expression statement is skipped rather than reported: Python allows it and
    /// the IR's `Class` has no place for it.
    fn class_field(&mut self, node: tree_sitter::Node<'_>) -> Option<VarDecl> {
        let assignment = child_of_kind(node, "assignment")?;
        let ty = self.declared_type(assignment);
        ty.as_ref()?;
        let pattern = self.assignment_target(assignment.child_by_field_name("left"));
        let value = assignment
            .child_by_field_name("right")
            .map(|n| self.expr(n));
        Some(VarDecl {
            pattern,
            ty,
            value,
            mutable: true,
            exported: false,
            span: self.span(node),
        })
    }

    /// Lowers a decorated definition's decorators, in source order.
    ///
    /// Order is preserved rather than reversed: Python applies the last decorator
    /// first, and a backend mapping one onto a native feature has to reproduce
    /// that order rather than re-derive it.
    fn decorators(&mut self, node: tree_sitter::Node<'_>) -> Vec<Decorator> {
        let mut decorators = Vec::new();
        for child in named_children(node) {
            if child.kind() != "decorator" {
                continue;
            }
            // The `@` token is anonymous, so the first named child is the
            // expression. Skipping by index would break the moment the grammar
            // inserts a token.
            let Some(expression) = child.named_child(0) else {
                self.builder.report(
                    Diagnostic::error(self.span(child), "decorator has no expression")
                        .with_code("gset-python-decorator"),
                );
                continue;
            };
            let expr = self.expr(expression);
            decorators.push(Decorator::new(expr, self.span(child)));
        }
        decorators
    }
}

/// Routes a method to `constructors` when it is `__init__`, else to `methods`.
fn push_method(methods: &mut Vec<Function>, constructors: &mut Vec<Function>, method: Function) {
    if &*method.name == "__init__" {
        constructors.push(method);
    } else {
        methods.push(method);
    }
}

/// The named children of `node`, in source order.
fn named_children(node: tree_sitter::Node<'_>) -> Vec<tree_sitter::Node<'_>> {
    let mut children = Vec::new();
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                children.push(child);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    children
}

/// The first direct child of `node` with the given kind.
fn child_of_kind<'t>(node: tree_sitter::Node<'t>, kind: &str) -> Option<tree_sitter::Node<'t>> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() == kind)
}

// -------------------------------------------------------------- statements

impl<'a> Lowerer<'a> {
    /// Lowers a block: a `block` node, or a single statement used as a body.
    fn block(&mut self, node: Option<tree_sitter::Node<'_>>) -> Block {
        let Some(node) = node else {
            return Block::empty(Span::synthetic());
        };
        let span = self.span(node);
        let mut statements = Vec::new();
        for child in named_children(node) {
            // A block's own children are statements; the brackets are anonymous.
            if child.kind() == "block" {
                statements.extend(self.block(Some(child)).statements);
            } else {
                statements.extend(self.statement(child));
            }
        }
        Block::new(statements, span)
    }

    /// Lowers one statement, possibly into several.
    ///
    /// Returning a `Vec` rather than one `Stmt` because a `with` clause with
    /// several resources becomes several `With` statements, and a walrus in a
    /// condition has to hoist a binding out of the condition's scope.
    fn statement(&mut self, node: tree_sitter::Node<'_>) -> Vec<Stmt> {
        let span = self.span(node);
        match node.kind() {
            "pass_statement" => vec![Stmt::Empty { span }],
            "expression_statement" => self.expression_statement(node),
            "return_statement" => {
                let value = node.named_child(0).map(|n| self.expr(n));
                vec![Stmt::Return { value, span }]
            }
            "raise_statement" => {
                // `raise` with no operand re-raises the active exception. That
                // is a re-throw, not an expression, so the IR carries `None`
                // rather than inventing an operand.
                let value = node.named_child(0).map(|n| self.expr(n));
                vec![Stmt::Throw { value, span }]
            }
            "break_statement" => {
                if self.loop_depth == 0 {
                    self.builder.report(
                        Diagnostic::error(span, "`break` outside a loop")
                            .with_code("gset-python-break"),
                    );
                }
                vec![Stmt::Break { label: None, span }]
            }
            "continue_statement" => {
                if self.loop_depth == 0 {
                    self.builder.report(
                        Diagnostic::error(span, "`continue` outside a loop")
                            .with_code("gset-python-break"),
                    );
                }
                vec![Stmt::Continue { label: None, span }]
            }
            "yield_statement" => {
                // A bare `yield` pauses a generator and produces nothing, which
                // is not the same as `yield None`; the IR's optional value is
                // what distinguishes them.
                let first = node.named_child(0);
                // `yield from` delegates iteration to another sequence, which is
                // not the same as yielding its elements one at a time: it also
                // forwards `send` and `throw`. The IR cannot express that, so it
                // is reported rather than flattened.
                if first.is_some_and(|n| n.kind() == "from") {
                    self.builder.report(
                        Diagnostic::error(
                            span,
                            "`yield from` has no representation: it delegates iteration and \
                             forwards `send`/`throw`, which yielding the elements one at a \
                             time does not do",
                        )
                        .with_code("gset-python-yield-from"),
                    );
                    return Vec::new();
                }
                let value = first.map(|n| self.expr(n));
                if self.fn_depth == 0 {
                    self.builder.report(
                        Diagnostic::error(span, "`yield` outside a function")
                            .with_code("gset-python-yield"),
                    );
                }
                vec![Stmt::Yield { value, span }]
            }
            "assert_statement" => {
                let mut parts = named_children(node);
                if parts.is_empty() {
                    return vec![Stmt::error("empty `assert`", span)];
                }
                let condition = self.expr(parts.remove(0));
                let message = parts.first().map(|n| self.expr(*n));
                vec![Stmt::Assert {
                    condition,
                    message,
                    span,
                }]
            }
            "delete_statement" => {
                let Some(target) = node.named_child(0) else {
                    return vec![Stmt::error("empty `del`", span)];
                };
                vec![Stmt::Delete {
                    target: self.expr(target),
                    span,
                }]
            }
            "if_statement" => self.if_statement(node, span),
            "while_statement" => self.while_statement(node, span),
            "for_statement" => self.for_statement(node, span),
            "try_statement" => self.try_statement(node, span),
            "with_statement" => self.with_statement(node),
            "match_statement" => self.match_statement(node, span),
            "global_statement" | "nonlocal_statement" => {
                // These change name resolution, and the IR has no scope
                // declaration. Silently ignoring one would change which binding
                // a name refers to.
                let keyword = if node.kind() == "global_statement" {
                    "global"
                } else {
                    "nonlocal"
                };
                self.builder.report(
                    Diagnostic::error(
                        span,
                        format!(
                            "`{keyword}` changes name resolution and has no representation in \
                             the IR"
                        ),
                    )
                    .with_code("gset-python-scope"),
                );
                Vec::new()
            }
            "import_statement" | "import_from_statement" | "future_import_statement" => {
                // A local import binds names, so it lowers to declarations in the
                // enclosing block rather than being dropped. Which module it
                // names is a `gset-deps` question; the IR's `Stmt` cannot hold an
                // `Import` item.
                self.builder.report(
                    Diagnostic::error(
                        span,
                        "an import inside a function or method has no representation: \
                         `Stmt` cannot hold an `Import` item, and dropping it would leave the \
                         names it binds unbound",
                    )
                    .with_code("gset-python-local-import"),
                );
                Vec::new()
            }
            "decorated_definition" => {
                // A nested decorated `def` or `class`. Both are items, and a
                // `Block` holds statements, so there is nowhere to put them that
                // is not a lie.
                let Some(definition) = node.child_by_field_name("definition") else {
                    return Vec::new();
                };
                let decorators = self.decorators(node);
                match definition.kind() {
                    "function_definition" => {
                        let mut function = self.function(definition);
                        function.decorators = decorators;
                        vec![Stmt::LocalItem(Box::new(Item::Function(function)))]
                    }
                    "class_definition" => {
                        let mut class = self.class(definition);
                        class.decorators = decorators;
                        vec![Stmt::LocalItem(Box::new(Item::Class(class)))]
                    }
                    other => {
                        self.unsupported(definition, other);
                        Vec::new()
                    }
                }
            }
            // A closure or local class is a declaration inside a body. The IR
            // carries it as a local item so a scope pass can see it and a
            // backend can emit it, rather than discarding it.
            "function_definition" => {
                let function = self.function(node);
                vec![Stmt::LocalItem(Box::new(Item::Function(function)))]
            }
            "class_definition" => {
                let class = self.class(node);
                vec![Stmt::LocalItem(Box::new(Item::Class(class)))]
            }
            "block" => self.block(Some(node)).statements,
            other => {
                self.unsupported(node, other);
                Vec::new()
            }
        }
    }

    /// Lowers an expression statement, which is where Python hides most forms.
    fn expression_statement(&mut self, node: tree_sitter::Node<'_>) -> Vec<Stmt> {
        let span = self.span(node);
        let Some(inner) = node.named_child(0) else {
            return vec![Stmt::Empty { span }];
        };

        match inner.kind() {
            "assignment" => {
                let pattern = self.assignment_target(inner.child_by_field_name("left"));
                let value = match inner.child_by_field_name("right") {
                    Some(right) => self.expr(right),
                    None => Expr::error("assignment has no value", span),
                };
                // An annotated assignment inside a function is a declaration, not
                // an assignment: the annotation is a claim about the type, and
                // that claim travels with the value so a backend does not have to
                // re-read the annotation.
                match self.declared_type(inner) {
                    Some(ty) => {
                        let value = Expr::typed(value.kind, ty.clone(), value.span);
                        vec![Stmt::Decl(VarDecl {
                            pattern,
                            ty: Some(ty),
                            value: Some(value),
                            mutable: true,
                            exported: false,
                            span,
                        })]
                    }
                    None => vec![Stmt::Assign {
                        target: pattern,
                        value,
                        span,
                    }],
                }
            }
            "augmented_assignment" => {
                // `x += v` keeps its operator. The IR models a compound
                // assignment rather than desugaring it to `x = x + v`, because
                // the two differ for types where the operation is in place, such
                // as a list append.
                let Some(target) = inner.child_by_field_name("left") else {
                    return vec![Stmt::error("compound assignment has no target", span)];
                };
                let Some(op_node) = inner.child_by_field_name("operator") else {
                    return vec![Stmt::error("compound assignment has no operator", span)];
                };
                let Some(rhs) = inner.child_by_field_name("right") else {
                    return vec![Stmt::error("compound assignment has no value", span)];
                };
                let op_text = self.text_of(op_node).trim();
                let base = op_text.strip_suffix('=').unwrap_or(op_text);
                let Some(op) = self.binary_op_text(base) else {
                    self.unsupported(op_node, "compound operator");
                    return Vec::new();
                };
                let pattern = self.assignment_target(Some(target));
                let value = self.expr(rhs);
                let previous = match pattern.single_binding() {
                    Some(name) => Expr::path(name.clone(), pattern.span),
                    None => {
                        self.builder.report(
                            Diagnostic::error(
                                pattern.span,
                                "a compound assignment to anything but a name has no \
                                 representation: reading the previous value needs the same target \
                                 twice, which is not expressible here",
                            )
                            .with_code("gset-python-augmented"),
                        );
                        return Vec::new();
                    }
                };
                vec![Stmt::Expr(Expr::new(
                    ExprKind::Assign {
                        target: Box::new(pattern),
                        previous: Some(Box::new(previous)),
                        op: Some(op),
                        value: Box::new(value),
                    },
                    span,
                ))]
            }
            "named_expression" => {
                let Some(name) = inner.child_by_field_name("name") else {
                    return vec![Stmt::error("walrus has no name", span)];
                };
                let Some(value) = inner.child_by_field_name("value") else {
                    return vec![Stmt::error("walrus has no value", span)];
                };
                let pattern = Pattern::bind(self.name_of(name), self.span(name));
                let value = self.expr(value);
                vec![Stmt::Assign {
                    target: pattern,
                    value,
                    span,
                }]
            }
            // `a, b = pair` parses the target as a tuple; the pattern carries it.
            "expression_list" | "tuple" => {
                let mut parts = named_children(inner);
                let pattern = if parts.len() == 1 {
                    self.pattern(parts.remove(0))
                } else {
                    let subpatterns = parts.iter().map(|n| self.pattern(*n)).collect();
                    Pattern::sequence(subpatterns, span)
                };
                let value = if parts.is_empty() {
                    Expr::error("tuple assignment has no value", span)
                } else {
                    self.expr(parts[0])
                };
                vec![Stmt::Assign {
                    target: pattern,
                    value,
                    span,
                }]
            }
            // `yield` is an expression node in the grammar but a statement in
            // the IR, so it is caught here before the expression fallback.
            "yield" => {
                let value = inner.named_child(0).map(|n| self.expr(n));
                if self.fn_depth == 0 {
                    self.builder.report(
                        Diagnostic::error(span, "`yield` outside a function")
                            .with_code("gset-python-yield"),
                    );
                }
                vec![Stmt::Yield { value, span }]
            }
            _ => {
                let expr = self.expr(inner);
                vec![Stmt::Expr(expr)]
            }
        }
    }

    /// Lowers the target of an assignment.
    fn assignment_target(&mut self, node: Option<tree_sitter::Node<'_>>) -> Pattern {
        let Some(node) = node else {
            return Pattern::ignore(Span::synthetic());
        };
        match node.kind() {
            "identifier" => Pattern::bind(self.name_of(node), self.span(node)),
            "_" => Pattern::ignore(self.span(node)),
            "as_pattern_target" => Pattern::bind(self.name_of(node), self.span(node)),
            // A field or subscript target binds no name, so it becomes a
            // location pattern. Representing `self.count` as the binding `count`
            // would silently rebind a local of the same name.
            "attribute" | "subscript" => Pattern::location(self.expr(node), self.span(node)),
            "tuple" | "pattern_list" | "expression_list" => {
                let subpatterns = named_children(node)
                    .iter()
                    .map(|n| self.pattern(*n))
                    .collect();
                Pattern::sequence(subpatterns, self.span(node))
            }
            "list_splat_pattern" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.pattern(n))
                    .unwrap_or_else(|| Pattern::ignore(self.span(node)));
                Pattern::sequence(vec![inner], self.span(node))
            }
            _ => {
                self.unsupported(node, "assignment target");
                Pattern::ignore(self.span(node))
            }
        }
    }

    /// Lowers a conditional, hoisting any binding its condition makes.
    fn if_statement(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Vec<Stmt> {
        // A walrus in a condition binds its name for the rest of the enclosing
        // block, which a `Let` cannot express: the value outlives the condition.
        // A preceding assignment is Python's rule, and statement level is the
        // only place hoisting one is sound.
        let (mut statements, condition) = match node.child_by_field_name("condition") {
            Some(n) => peel_lets(self.condition(n)),
            None => (Vec::new(), Expr::error("`if` with no condition", span)),
        };
        let then_branch = self.block(node.child_by_field_name("consequence"));
        let else_branch = self.else_branch(node);

        statements.push(Stmt::If {
            condition,
            then_branch,
            else_branch: else_branch.map(Box::new),
            span,
        });
        statements
    }

    /// Lowers an `else` or `elif` chain.
    ///
    /// An `elif` is an `if` in the `else` position, so it lowers recursively and
    /// comes out as [`Else::If`]. Flattening it into a plain block would make a
    /// backend emit `} else { if ... }`, which is Go defect class 1.
    ///
    /// The grammar keeps every `elif` as its own clause beside the `if`, and the
    /// trailing `else` as one more clause after them. Reading only the `else`
    /// clause therefore answers with the last branch of the chain and drops
    /// every condition before it — a program that runs and does something else.
    fn else_branch(&mut self, node: tree_sitter::Node<'_>) -> Option<Else> {
        let mut clauses: Vec<tree_sitter::Node<'_>> = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if matches!(child.kind(), "elif_clause" | "else_clause") {
                clauses.push(child);
            }
        }
        let mut branch: Option<Else> = None;
        // The chain is nested from the back: the trailing `else` is the
        // innermost, and each `elif` wraps what follows it.
        for clause in clauses.into_iter().rev() {
            let else_branch = branch.map(Box::new);
            branch = match clause.kind() {
                "else_clause" => {
                    let body = clause.child_by_field_name("body");
                    Some(Else::Block(self.block(body)))
                }
                _ => Some(Else::If(Box::new(self.elif_if(clause, else_branch)))),
            };
        }
        branch
    }

    /// Lowers one `elif` clause to the `if` statement that carries it.
    fn elif_if(&mut self, clause: tree_sitter::Node<'_>, else_branch: Option<Box<Else>>) -> Stmt {
        let span = self.span(clause);
        let (mut statements, condition) = match clause.child_by_field_name("condition") {
            Some(n) => peel_lets(self.condition(n)),
            None => (Vec::new(), Expr::error("`elif` with no condition", span)),
        };
        let then_branch = self.block(clause.child_by_field_name("consequence"));
        statements.push(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span,
        });
        // An `elif` sits in an else position, where a preceding statement
        // cannot be emitted: the binding a walrus in its condition makes would
        // have to be written before the chain reached this branch. Reporting it
        // is the only honest answer, since dropping the binding would silently
        // change what the condition reads.
        if statements.len() > 1 {
            self.builder.report(
                Diagnostic::error(span, "a walrus in an `elif` condition has no place to bind")
                    .with_code("gset-python-elif-walrus"),
            );
        }
        statements.pop().expect("an `elif` always lowers")
    }

    /// Lowers a `while`, hoisting any binding its condition makes.
    fn while_statement(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Vec<Stmt> {
        let (mut statements, condition) = match node.child_by_field_name("condition") {
            Some(n) => peel_lets(self.condition(n)),
            None => (Vec::new(), Expr::error("`while` with no condition", span)),
        };
        self.loop_depth += 1;
        let body = self.block(node.child_by_field_name("body"));
        self.loop_depth -= 1;
        let else_body = self.loop_else(node);

        statements.push(Stmt::While {
            condition,
            body,
            else_body,
            span,
        });
        statements
    }

    /// Lowers a loop's `else` clause, which runs when the loop ends without
    /// `break`.
    fn loop_else(&mut self, node: tree_sitter::Node<'_>) -> Option<Block> {
        let clause = child_of_kind(node, "else_clause")?;
        clause.named_child(0).map(|block| self.block(Some(block)))
    }

    /// Lowers a `for`.
    ///
    /// `async for` is rejected: it needs the target to drive an async iterator,
    /// which is a capability question no IR loop node answers.
    fn for_statement(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Vec<Stmt> {
        let Some(left) = node.child_by_field_name("left") else {
            return vec![Stmt::error("`for` has no target", span)];
        };
        let Some(iterable) = node.child_by_field_name("right") else {
            return vec![Stmt::error("`for` has no sequence", span)];
        };
        let is_async = child_of_kind(node, "async").is_some();
        if is_async {
            self.builder.report(
                Diagnostic::error(
                    span,
                    "`async for` has no representation: it needs an async iterator protocol, \
                     which no IR loop statement models",
                )
                .with_code("gset-python-async-for"),
            );
            return Vec::new();
        }

        let pattern = self.pattern(left);
        let iterable = self.expr(iterable);

        self.loop_depth += 1;
        let body = self.block(node.child_by_field_name("body"));
        self.loop_depth -= 1;
        let else_body = self.loop_else(node);

        vec![Stmt::ForIn {
            pattern,
            iterable,
            body,
            else_body,
            is_parallel: false,
            label: None,
            span,
        }]
    }

    /// Lowers a `try`.
    fn try_statement(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Vec<Stmt> {
        self.fn_depth += 1;
        let body = self.block(node.child_by_field_name("body"));
        self.fn_depth -= 1;

        let mut handlers = Vec::new();
        let mut finally = None;
        for child in named_children(node) {
            match child.kind() {
                "except_clause" => {
                    if let Some(handler) = self.except_clause(child) {
                        handlers.push(handler);
                    }
                }
                "except_group_clause" => self.unsupported(child, "`except*` group"),
                "finally_clause" => {
                    finally = Some(
                        child
                            .named_child(0)
                            .map(|b| self.block(Some(b)))
                            .unwrap_or_else(|| Block::empty(self.span(child))),
                    );
                }
                _ => {}
            }
        }
        vec![Stmt::Try {
            body,
            handlers,
            finally,
            span,
        }]
    }

    /// Lowers one `except` clause.
    ///
    /// The exception type is recorded by name and left untranslated, because
    /// which hierarchy exists is a target question `gset-deps` answers.
    fn except_clause(&mut self, node: tree_sitter::Node<'_>) -> Option<CatchClause> {
        let span = self.span(node);
        let value = node.child_by_field_name("value");
        let (binding, types) = match value {
            None => (None, Vec::new()),
            Some(value) => match value.kind() {
                "as_pattern" => {
                    let types = match value.named_child(0) {
                        Some(inner) => self.exception_types(inner),
                        None => Vec::new(),
                    };
                    let binding = value
                        .child_by_field_name("alias")
                        .map(|n| Pattern::bind(self.name_of(n), self.span(n)));
                    (binding, types)
                }
                _ => (None, self.exception_types(value)),
            },
        };

        self.fn_depth += 1;
        let body = child_of_kind(node, "block")
            .map(|b| self.block(Some(b)))
            .unwrap_or_else(|| Block::empty(span));
        self.fn_depth -= 1;

        Some(CatchClause {
            binding,
            types,
            body,
            span,
        })
    }

    /// The exception names an `except` clause catches.
    fn exception_types(&mut self, node: tree_sitter::Node<'_>) -> Vec<Name> {
        match node.kind() {
            "identifier" | "dotted_name" => self
                .dotted_name(node, self.span(node))
                .segments
                .into_iter()
                .collect(),
            // `except (A, B):` catches either.
            "tuple" => named_children(node)
                .iter()
                .flat_map(|child| self.exception_types(*child))
                .collect(),
            other => {
                self.unsupported(node, other);
                Vec::new()
            }
        }
    }

    /// Lowers a `with`, one resource at a time.
    fn with_statement(&mut self, node: tree_sitter::Node<'_>) -> Vec<Stmt> {
        let items: Vec<_> = named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "with_clause" || child.kind() == "with_item")
            .collect();

        let mut out = Vec::new();
        let total = items.len();
        for (index, item) in items.into_iter().enumerate() {
            let item_span = self.span(item);
            let entry = if item.kind() == "with_clause" {
                child_of_kind(item, "with_item").unwrap_or(item)
            } else {
                item
            };

            let value_node = entry.child_by_field_name("value").unwrap_or(entry);
            let (value_expr, binding) = match value_node.kind() {
                "as_pattern" => {
                    let resource = value_node
                        .named_child(0)
                        .map(|n| self.expr(n))
                        .unwrap_or_else(|| Expr::error("`with` names no resource", item_span));
                    let pattern = value_node
                        .child_by_field_name("alias")
                        .map(|n| self.pattern(n));
                    (resource, pattern)
                }
                _ => (self.expr(value_node), None),
            };

            let is_last = index + 1 == total;
            let body = if is_last {
                self.block(node.child_by_field_name("body"))
            } else {
                // Each resource needs its own `With`, but only the last one owns
                // the body. The earlier ones get an empty body, which is what
                // says "acquire only".
                Block::empty(item_span)
            };

            out.push(Stmt::With {
                value: value_expr,
                binding,
                body,
                is_last,
                span: item_span,
            });
        }
        out
    }

    /// Lowers a `match`.
    ///
    /// Only literal and capture patterns are translated. Python's class, mapping
    /// and or-patterns have no IR equivalent, and translating a subset silently
    /// would change which cases match.
    fn match_statement(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Vec<Stmt> {
        let scrutinee = node.child_by_field_name("subject").map(|n| self.expr(n));
        let mut cases = Vec::new();
        let mut default = None;

        for child in named_children(node) {
            if child.kind() != "case_clause" {
                continue;
            }
            let case_span = self.span(child);
            // The first named child is the pattern and the second is the body.
            let Some(pattern_node) = child.named_child(0) else {
                continue;
            };
            let body = child
                .named_child(1)
                .map(|b| self.block(Some(b)))
                .unwrap_or_else(|| Block::empty(case_span));

            let mut guard = None;
            if let Some(guard_node) = child.child_by_field_name("guard") {
                guard = Some(self.expr(guard_node));
            }

            if is_wildcard_pattern(pattern_node) {
                default = Some(body);
                continue;
            }
            let pattern = self.match_pattern(pattern_node);
            cases.push(MatchCase {
                pattern,
                guard,
                body,
                span: case_span,
            });
        }

        if scrutinee.is_none() {
            self.builder.report(
                Diagnostic::error(
                    span,
                    "`match` with no subject cannot be translated: a tag-style switch is a \
                     different construct",
                )
                .with_code("gset-python-match"),
            );
        }

        vec![Stmt::Switch {
            scrutinee,
            cases,
            default,
            span,
        }]
    }

    /// Lowers a `case` pattern.
    fn match_pattern(&mut self, node: tree_sitter::Node<'_>) -> Pattern {
        let span = self.span(node);
        match node.kind() {
            "capture_pattern" | "as_pattern" => {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| self.name_of(n))
                    .unwrap_or_default();
                Pattern::bind(name, span)
            }
            "wildcard_pattern" => Pattern::ignore(span),
            other => {
                // A literal case still matches a value; the IR's `Pattern` has
                // no literal form, so this is reported rather than approximated
                // by a binding that would match everything.
                self.unsupported(node, other);
                Pattern::ignore(span)
            }
        }
    }

    /// Lowers a loop or conditional condition.
    ///
    /// No prelude is needed: a walrus lowers to `ExprKind::Let`, which scopes the
    /// binding over the condition itself rather than needing a statement in front
    /// of it.
    fn condition(&mut self, node: tree_sitter::Node<'_>) -> Expr {
        self.expr(node)
    }
}

/// Whether a `case` pattern is the catch-all `_`.
fn is_wildcard_pattern(node: tree_sitter::Node<'_>) -> bool {
    node.kind() == "wildcard_pattern" || node.kind() == "_"
}

// ------------------------------------------------------------- expressions

impl<'a> Lowerer<'a> {
    /// Lowers one expression.
    ///
    /// Every arm returns an [`Expr`] of unknown type. The frontend records what
    /// the source said; it does not decide what type a value has, which is
    /// `gset-semantic`'s job and the reason [`Type::UNKNOWN`] is contagious
    /// rather than being quietly filled in here.
    fn expr(&mut self, node: tree_sitter::Node<'_>) -> Expr {
        let span = self.span(node);
        match node.kind() {
            // ------------------------------------------------------ literals
            "integer" => {
                // The text is kept verbatim. Python integers are unbounded, and
                // Go's implementation formatted every numeric literal with `%g`,
                // which saturated large integers at int64 max and produced quoted
                // floats. Keeping the source text cannot lose that.
                Expr::new(
                    ExprKind::Literal(Literal::Int(self.text_of(node).to_string())),
                    span,
                )
            }
            "float" => Expr::new(
                ExprKind::Literal(Literal::Float(self.text_of(node).to_string())),
                span,
            ),
            "true" => Expr::new(ExprKind::Literal(Literal::Bool(true)), span),
            "false" => Expr::new(ExprKind::Literal(Literal::Bool(false)), span),
            "none" => Expr::new(ExprKind::Literal(Literal::Null), span),
            "string" | "concatenated_string" => self.string(node, span),
            "ellipsis" => Expr::error("`...` has no equivalent in the IR", span),

            // --------------------------------------------------------- names
            "identifier" => {
                // `_` discards a value. The IR has no discard expression, and
                // binding it to a name would shadow the conventional throwaway.
                if self.text_of(node) == "_" {
                    Expr::error("`_` discards its value and has no expression form", span)
                } else {
                    Expr::path(self.name_of(node), span)
                }
            }
            "dotted_name" => {
                let path = self.dotted_name(node, span);
                Expr::new(ExprKind::Path(path), span)
            }
            "attribute" => {
                let target = node
                    .child_by_field_name("object")
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("attribute has no receiver", span));
                let field = node
                    .child_by_field_name("attribute")
                    .map(|n| self.name_of(n))
                    .unwrap_or_default();
                Expr::new(
                    ExprKind::Field {
                        target: Box::new(target),
                        field,
                    },
                    span,
                )
            }

            // --------------------------------------------------------- calls
            "call" => {
                let callee = node
                    .child_by_field_name("function")
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("call has no callee", span));
                let (args, named_args) = self.arguments(node);

                // `obj.m(...)` is a method call when `obj.m` is a field access.
                // Splitting it here rather than in a backend means the IR says
                // which it was, instead of leaving a backend to pattern-match.
                match &callee.kind {
                    ExprKind::Field { target, field } => {
                        let receiver = target.clone();
                        let method = field.clone();
                        Expr::new(
                            ExprKind::MethodCall {
                                receiver,
                                method,
                                args,
                                named_args,
                            },
                            span,
                        )
                    }
                    _ => Expr::new(
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args,
                            named_args,
                        },
                        span,
                    ),
                }
            }
            // An `argument_list` is only ever a call's argument node. Reaching
            // this arm means the grammar produced one where no call was, which
            // is reported instead of being wrapped in an invented tuple.
            "argument_list" => {
                self.builder.report(
                    Diagnostic::error(span, "an argument list outside a call has no expression")
                        .with_code("gset-python-unsupported"),
                );
                Expr::error("argument list is not an expression", span)
            }
            "generator_expression"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension" => self.comprehension(node, span),

            // ----------------------------------------------------- operators
            "binary_operator" => {
                let lhs = node
                    .child_by_field_name("left")
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("operator has no left operand", span));
                let rhs = node
                    .child_by_field_name("right")
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("operator has no right operand", span));

                let operator = node.child_by_field_name("operator");
                match operator.map(|n| n.kind()) {
                    // `and`/`or` short-circuit and return an operand, not a
                    // boolean. Representing them as an arithmetic operation
                    // would make a backend invent a conversion.
                    Some("and") | Some("or") => {
                        let op = if self.text_of(operator.expect("checked above")) == "and" {
                            LogicalOp::And
                        } else {
                            LogicalOp::Or
                        };
                        Expr::new(
                            ExprKind::Logical {
                                op,
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                            },
                            span,
                        )
                    }
                    Some("comparison_operator") => self.comparison(node, span),
                    _ => {
                        let Some(op_node) = operator else {
                            return Expr::error("operator has no operator", span);
                        };
                        match self.binary_op(op_node) {
                            Some(op) => Expr::new(
                                ExprKind::Binary {
                                    op,
                                    lhs: Box::new(lhs),
                                    rhs: Box::new(rhs),
                                },
                                span,
                            ),
                            None => {
                                self.unsupported(op_node, "operator");
                                Expr::error("unsupported operator", span)
                            }
                        }
                    }
                }
            }
            "boolean_operator" => {
                let mut parts = named_children(node);
                if parts.len() != 2 {
                    // Chained `a and b and c` nests to the left, so two operands
                    // is the only shape that reaches here.
                    return Expr::error("boolean operator needs two operands", span);
                }
                let rhs = parts.pop().expect("checked length");
                let lhs = parts.pop().expect("checked length");
                let operator = node
                    .child_by_field_name("operator")
                    .map(|n| self.text_of(n).trim().to_string())
                    .unwrap_or_default();
                let op = if operator == "and" {
                    LogicalOp::And
                } else {
                    LogicalOp::Or
                };
                Expr::new(
                    ExprKind::Logical {
                        op,
                        lhs: Box::new(self.expr(lhs)),
                        rhs: Box::new(self.expr(rhs)),
                    },
                    span,
                )
            }
            "comparison_operator" => self.comparison(node, span),
            "not_operator" => {
                let operand = node
                    .named_child(0)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("`not` has no operand", span));
                Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    },
                    span,
                )
            }
            "unary_operator" => {
                let Some(op_node) = node.child_by_field_name("operator") else {
                    return Expr::error("unary operator has no operator", span);
                };
                let operand = node
                    .child_by_field_name("argument")
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("unary operator has no operand", span));
                let Some(op) = self.unary_op(op_node) else {
                    self.unsupported(op_node, "unary operator");
                    return Expr::error("unsupported unary operator", span);
                };
                Expr::new(
                    ExprKind::Unary {
                        op,
                        operand: Box::new(operand),
                    },
                    span,
                )
            }

            // ---------------------------------------------------- collections
            // A comma-separated list in expression position, such as an
            // argument or a `return`. It is a tuple literal here; the same node
            // kind in a target position is a pattern, handled by `pattern`.
            "expression_list" => {
                let elements: Vec<_> = named_children(node).iter().map(|n| self.expr(*n)).collect();
                Expr::new(ExprKind::Tuple { elements }, span)
            }
            "list" | "set" => {
                let elements = named_children(node).iter().map(|n| self.expr(*n)).collect();
                let kind = if node.kind() == "list" {
                    ExprKind::List { elements }
                } else {
                    ExprKind::Set { elements }
                };
                Expr::new(kind, span)
            }
            "tuple" | "parenthesized_expression" => {
                let elements = named_children(node).iter().map(|n| self.expr(*n)).collect();
                Expr::new(ExprKind::Tuple { elements }, span)
            }
            "dictionary" => {
                let mut entries = Vec::new();
                for child in named_children(node) {
                    if child.kind() != "pair" {
                        self.unsupported(child, "dictionary entry");
                        continue;
                    }
                    let key = child
                        .child_by_field_name("key")
                        .map(|n| self.expr(n))
                        .unwrap_or_else(|| Expr::error("dictionary entry has no key", span));
                    let value = child
                        .child_by_field_name("value")
                        .map(|n| self.expr(n))
                        .unwrap_or_else(|| Expr::error("dictionary entry has no value", span));
                    entries.push(self.pair_entry(key, value, span));
                }
                Expr::new(ExprKind::Map { entries }, span)
            }
            "list_splat" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("`*` has no value", span));
                Expr::new(
                    ExprKind::Unpack {
                        expr: Box::new(inner),
                    },
                    span,
                )
            }
            "dictionary_splat" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("`**` has no value", span));
                Expr::new(
                    ExprKind::Unpack {
                        expr: Box::new(inner),
                    },
                    span,
                )
            }
            "list_splat_pattern" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("`*` has no value", span));
                Expr::new(
                    ExprKind::Unpack {
                        expr: Box::new(inner),
                    },
                    span,
                )
            }

            // ---------------------------------------------------------- index
            "subscript" => self.subscript(node, span),
            "slice" => self.slice(node, span),

            // ------------------------------------------------------ functions
            "lambda" => self.lambda(node, span),

            // -------------------------------------------------------- control
            "conditional_expression" => {
                // `a if b else c`: the grammar exposes no field names here, so
                // the three operands are positional. The middle named child is
                // the condition, not the second operand in reading order.
                let then_branch = node
                    .named_child(0)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("conditional has no true branch", span));
                let condition = node
                    .named_child(1)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("conditional has no condition", span));
                let else_branch = node
                    .named_child(2)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("conditional has no false branch", span));
                Expr::new(
                    ExprKind::Conditional {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    span,
                )
            }
            "await" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.expr(n))
                    .unwrap_or_else(|| Expr::error("`await` has no operand", span));
                Expr::new(
                    ExprKind::Await {
                        expr: Box::new(inner),
                    },
                    span,
                )
            }
            "named_expression" => {
                // The walrus keeps its binding rather than being reduced to its
                // value: `(n := f())` is how a Python author binds and tests in
                // one expression, and dropping the binding changes what the name
                // means for the rest of the statement.
                let Some(name) = node.child_by_field_name("name") else {
                    return Expr::error("walrus has no name", span);
                };
                let Some(value) = node.child_by_field_name("value") else {
                    return Expr::error("walrus has no value", span);
                };
                let pattern = Pattern::bind(self.name_of(name), self.span(name));
                let value = self.expr(value);
                let body = Expr::path(pattern.names.first().cloned().unwrap_or_default(), span);
                Expr::new(
                    ExprKind::Let {
                        pattern,
                        value: Box::new(value),
                        body: Box::new(body),
                    },
                    span,
                )
            }

            other => {
                self.unsupported(node, other);
                Expr::error("unsupported expression", span)
            }
        }
    }

    /// Lowers a call's arguments, splitting positional from keyword.
    fn arguments(&mut self, node: tree_sitter::Node<'_>) -> (Vec<Expr>, Vec<NamedArg>) {
        let Some(list) = node.child_by_field_name("arguments") else {
            return (Vec::new(), Vec::new());
        };
        // A bare generator argument, `f(x for x in xs)`, has no
        // `argument_list`: the `arguments` field points straight at the
        // comprehension. Treat it as the single positional argument it is
        // rather than descending into its `for` clause.
        if matches!(
            list.kind(),
            "generator_expression"
                | "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
        ) {
            return (vec![self.expr(list)], Vec::new());
        }
        let mut args = Vec::new();
        let mut named = Vec::new();
        for child in named_children(list) {
            match child.kind() {
                "keyword_argument" => {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| self.name_of(n))
                        .unwrap_or_default();
                    let value = child
                        .child_by_field_name("value")
                        .map(|n| self.expr(n))
                        .unwrap_or_else(|| {
                            Expr::error("keyword argument has no value", self.span(child))
                        });
                    named.push(NamedArg { name, value });
                }
                _ => args.push(self.expr(child)),
            }
        }
        (args, named)
    }

    /// Builds a map entry, which may be a `**` splat.
    ///
    /// The IR's `Map` entries are all `NamedArg`, so a `**` merge has nowhere to
    /// go. It is reported rather than being flattened into a named argument,
    /// because the source cannot say what the splatted keys are.
    fn pair_entry(&mut self, key: Expr, value: Expr, span: Span) -> NamedArg {
        if key.is_error() {
            self.builder.report(
                Diagnostic::error(span, "a `**` merge has no IR map entry")
                    .with_code("gset-python-map-splat"),
            );
            return NamedArg {
                name: Name::default(),
                value,
            };
        }
        let name = match &key.kind {
            ExprKind::Literal(Literal::Str(text)) => gset_ir::name(text),
            _ => gset_ir::name(""),
        };
        NamedArg { name, value }
    }

    /// Lowers a comparison, including Python's chained form.
    ///
    /// `a < b < c` evaluates `b` once and short-circuits. The IR's comparison is
    /// binary, so the chain becomes two comparisons under `and`, which is only
    /// equivalent when the middle operand cannot have side effects. A warning
    /// says so when it might.
    fn comparison(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        // Operands are the named children; operators are the unnamed ones that
        // `comparison_op` recognises. Some operators (`<`, `>`) are a single
        // character, so length cannot be used to tell them from punctuation.
        let mut operands = Vec::new();
        let mut operators = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                operands.push(child);
            } else if self.comparison_op(child).is_some() {
                operators.push(child);
            }
        }

        if operands.len() == 2 && operators.len() == 1 {
            let Some(op) = self.comparison_op(operators[0]) else {
                self.unsupported(operators[0], "comparison operator");
                return Expr::error("unsupported comparison", span);
            };
            let mut hoisted = Vec::new();
            let lhs = self.operand(operands[0], &mut hoisted);
            let rhs = self.operand(operands[1], &mut hoisted);
            let comparison = Expr::new(
                ExprKind::Compare {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            );
            return hoist(hoisted, comparison);
        }

        if operands.len() < 2 || operators.len() < 2 {
            // `compare(a)` style node with a single operand is malformed.
            return Expr::error("comparison needs two operands", span);
        }

        let left = self.expr(operands[0]);
        let middle = self.expr(operands[1]);
        if !is_repeatable(&middle) {
            self.builder.report(
                Diagnostic::warning(
                    middle.span,
                    "the middle operand of this chained comparison is not a simple value, so \
                     expanding it into two comparisons may evaluate it more than once",
                )
                .with_code("gset-python-chained-compare"),
            );
        }

        // Left-associated, each comparison reading the same middle expression.
        let mut result = match self.comparison_op(operators[0]) {
            Some(op) => Expr::new(
                ExprKind::Compare {
                    op,
                    lhs: Box::new(left),
                    rhs: Box::new(middle.clone()),
                },
                span,
            ),
            None => {
                self.unsupported(operators[0], "comparison operator");
                return Expr::error("unsupported comparison", span);
            }
        };
        for (index, op_node) in operators[1..].iter().enumerate() {
            let operand = &operands[index + 2];
            let Some(op) = self.comparison_op(*op_node) else {
                self.unsupported(*op_node, "comparison operator");
                return Expr::error("unsupported comparison", span);
            };
            let rhs = self.expr(*operand);
            let lhs = match self.comparison_op(operators[index]) {
                Some(_) => middle.clone(),
                None => return Expr::error("unsupported comparison", span),
            };
            let comparison = Expr::new(
                ExprKind::Compare {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            );
            result = Expr::new(
                ExprKind::Logical {
                    op: LogicalOp::And,
                    lhs: Box::new(result),
                    rhs: Box::new(comparison),
                },
                span,
            );
        }
        result
    }

    /// Lowers an operand, pulling out a walrus it contains.
    ///
    /// `(n := len(xs)) > 2` binds `n` for the comparison, not for the rest of
    /// the function, so the binding has to wrap the comparison rather than sit
    /// inside it. Lowering the operand on its own would produce a binding whose
    /// scope is the operand, which is a name no other expression can see.
    fn operand(&mut self, node: tree_sitter::Node<'_>, hoisted: &mut Vec<(Pattern, Expr)>) -> Expr {
        if let Some((pattern, value, span)) = self.walrus(node) {
            let name = match pattern.single_binding() {
                Some(name) => name.to_string(),
                None => {
                    self.builder.report(
                        Diagnostic::error(span, "a walrus binds exactly one name")
                            .with_code("gset-python-unsupported"),
                    );
                    return Expr::error("a walrus that binds no name", span);
                }
            };
            hoisted.push((pattern, value));
            return Expr::path(gset_ir::name(&name), span);
        }
        self.expr(node)
    }

    /// The binding and value of a node that is only a walrus, if it is one.
    fn walrus(&mut self, node: tree_sitter::Node<'_>) -> Option<(Pattern, Expr, Span)> {
        let span = self.span(node);
        let inner = match node.kind() {
            "named_expression" => node,
            "parenthesized_expression" => {
                let inner = node.named_child(0)?;
                if inner.kind() != "named_expression" {
                    return None;
                }
                inner
            }
            _ => return None,
        };
        let name = inner.child_by_field_name("name")?;
        let value = inner.child_by_field_name("value")?;
        let pattern = Pattern::bind(self.name_of(name), self.span(name));
        let value = self.expr(value);
        Some((pattern, value, span))
    }

    /// Lowers a subscript, distinguishing an index from a slice.
    fn subscript(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        let target = node
            .child_by_field_name("value")
            .map(|n| self.expr(n))
            .unwrap_or_else(|| Expr::error("subscript has no target", span));
        let index = node.child_by_field_name("subscript");

        match index.map(|n| n.kind()) {
            Some("slice") => {
                let bounds = index.expect("checked above");
                self.slice_of(target, bounds, span)
            }
            Some(_) => {
                let bounds = index.expect("checked above");
                let subscript = self.expr(bounds);
                Expr::new(
                    ExprKind::Index {
                        target: Box::new(target),
                        index: Box::new(subscript),
                    },
                    span,
                )
            }
            None => Expr::error("subscript has no index", span),
        }
    }

    /// Lowers a standalone `a:b:c` slice.
    fn slice(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        // A bare slice has no target of its own; the IR requires one, so this is
        // reported rather than invented.
        self.builder.report(
            Diagnostic::error(span, "a slice needs a target to slice")
                .with_code("gset-python-slice"),
        );
        let target = Expr::error("slice has no target", span);
        self.slice_of(target, node, span)
    }

    /// Lowers `target[start:end:step]`.
    ///
    /// The bounds are read by counting the colons, not by position among the
    /// named children: a colon is not a named child, so `xs[:2]` has exactly
    /// one named child and reading it as the start bound quietly turns the
    /// first two elements into the last n-1.
    fn slice_of(&mut self, target: Expr, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        let mut start = None;
        let mut end = None;
        let mut colons = 0usize;
        for child in node.children(&mut node.walk()) {
            if child.kind() == ":" {
                colons += 1;
                continue;
            }
            if !child.is_named() {
                continue;
            }
            match colons {
                0 => start = Some(Box::new(self.expr(child))),
                1 => end = Some(Box::new(self.expr(child))),
                _ => {
                    // A step is not an absent bound but a different slice: it
                    // drops elements, so dropping it here would answer a
                    // question the program did not ask.
                    self.builder.report(
                        Diagnostic::error(span, "a slice step has no representation")
                            .with_code("gset-python-slice-step"),
                    );
                }
            }
        }
        Expr::new(
            ExprKind::Slice {
                target: Box::new(target),
                start,
                end,
                inclusive: false,
            },
            span,
        )
    }

    /// Lowers a `lambda`.
    ///
    /// A lambda raises the function depth: a `yield` inside one is a generator,
    /// and the builder needs to know a function encloses it.
    fn lambda(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        let mut params = Vec::new();
        if let Some(list) = node.child_by_field_name("parameters") {
            for child in named_children(list) {
                match child.kind() {
                    "identifier" => {
                        params.push(Pattern::bind(self.name_of(child), self.span(child)));
                    }
                    "default_parameter" | "typed_default_parameter" => {
                        params.push(self.parameter_name(child));
                    }
                    "typed_parameter" => params.push(self.parameter_name(child)),
                    "list_splat_pattern" => params.push(self.parameter_name(child)),
                    "keyword_separator" => {}
                    other => self.unsupported(child, other),
                }
            }
        }
        self.fn_depth += 1;
        let body = node
            .child_by_field_name("body")
            .map(|n| self.expr(n))
            .unwrap_or_else(|| Expr::error("`lambda` has no body", span));
        self.fn_depth -= 1;

        Expr::new(
            ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            span,
        )
    }

    /// Lowers one of the four comprehension forms.
    ///
    /// The clauses and the trailing filter are read in source order, so a filter
    /// that appears before a later `for` is reported rather than silently
    /// applied to the whole result. Python's `[x for x in a if p(x) for y in b]`
    /// filters one clause's iteration; widening that to a single condition on the
    /// finished comprehension would keep items the source discards.
    fn comprehension(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        let (kind, body) = match node.kind() {
            "list_comprehension" => (ComprehensionKind::List, node.child_by_field_name("body")),
            "set_comprehension" => (ComprehensionKind::Set, node.child_by_field_name("body")),
            // A mapping comprehension's body is a pair, and the IR wants its key
            // and its value separately.
            "dictionary_comprehension" => {
                (ComprehensionKind::Map, node.child_by_field_name("body"))
            }
            _ => (
                ComprehensionKind::Generator,
                node.child_by_field_name("body"),
            ),
        };

        let pair = if kind == ComprehensionKind::Map {
            body.filter(|b| b.kind() == "pair")
        } else {
            None
        };
        let key = pair
            .and_then(|p| p.child_by_field_name("key"))
            .map(|n| self.expr(n))
            .map(Box::new);
        let value_node = pair.and_then(|p| p.child_by_field_name("value")).or(body);
        let element = match value_node {
            Some(n) => self.expr(n),
            None => Expr::error("comprehension has no body", span),
        };

        // Walking the clauses in source order is what makes the interleaved-`if`
        // check possible at all.
        let mut clauses: Vec<GeneratorClause> = Vec::new();
        let mut condition: Option<Box<Expr>> = None;
        for child in named_children(node) {
            match child.kind() {
                "for_in_clause" => {
                    let clause_span = self.span(child);
                    let Some(left) = child.child_by_field_name("left") else {
                        continue;
                    };
                    let Some(right) = child.child_by_field_name("right") else {
                        continue;
                    };
                    let pattern = self.pattern(left);
                    let iterable = self.expr(right);
                    clauses.push(GeneratorClause {
                        pattern,
                        iterable,
                        span: clause_span,
                    });
                }
                "if_clause" => {
                    let Some(inner) = child.named_child(0) else {
                        continue;
                    };
                    let later_clause = named_children(node).iter().any(|sibling| {
                        sibling.kind() == "for_in_clause"
                            && self.span(*sibling).start() > self.span(child).start()
                    });
                    if later_clause {
                        self.builder.report(
                            Diagnostic::error(
                                self.span(child),
                                "this filter is followed by another `for`, so it applies to one \
                                 clause's iteration; this comprehension form can only carry a \
                                 filter over the finished result",
                            )
                            .with_code("gset-python-comprehension"),
                        );
                        continue;
                    }
                    if condition.is_some() {
                        self.builder.report(
                            Diagnostic::error(
                                self.span(child),
                                "a comprehension may carry only one filter",
                            )
                            .with_code("gset-python-comprehension"),
                        );
                        continue;
                    }
                    condition = Some(Box::new(self.expr(inner)));
                }
                _ => {}
            }
        }

        if clauses.is_empty() {
            self.builder.report(
                Diagnostic::error(span, "a comprehension needs at least one `for`")
                    .with_code("gset-python-comprehension"),
            );
        }

        // The IR's `element` slot is documented as the produced value for a
        // list, set or generator, and a mapping's produced value has its own
        // slot. Leaving `value` empty made every backend read the key instead,
        // so `{n: n * 2 for n in xs}` came out as a set of keys.
        let value = match kind {
            ComprehensionKind::Map => Some(Box::new(element.clone())),
            _ => None,
        };
        Expr::new(
            ExprKind::Comprehension {
                kind,
                element: Box::new(element),
                key,
                value,
                clauses,
                condition,
            },
            span,
        )
    }
}

/// Resolves the doubled braces an f-string uses to escape a literal one.
fn unescape_braces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let doubled = matches!(character, '{' | '}') && characters.peek() == Some(&character);
        if doubled {
            characters.next();
        }
        out.push(character);
    }
    out
}

/// Moves the bindings a condition introduced into statements before it.
///
/// A [`ExprKind::Let`] scopes over the expression it wraps, which is exactly
/// right for a walrus inside a call argument and exactly wrong for one in a
/// condition: `if (n := f()):` leaves `n` bound for the rest of the block. The
/// binding therefore becomes an assignment statement and the condition keeps
/// only what it evaluated to.
fn peel_lets(expr: Expr) -> (Vec<Stmt>, Expr) {
    let mut statements = Vec::new();
    let mut current = expr;
    // Innermost first, so the statements come out in the source order the
    // nestings were written in and each name is bound before it is used.
    while let ExprKind::Let {
        pattern,
        value,
        body,
    } = current.kind
    {
        let span = current.span;
        statements.push(Stmt::Assign {
            target: pattern,
            value: *value,
            span,
        });
        current = *body;
    }
    statements.reverse();
    (statements, current)
}

/// Wraps `body` in the bindings `hoisted` collected, innermost first.
///
/// `ExprKind::Let` is the IR's scoped binding, so one wrap per walrus in source
/// order gives each name exactly the scope Python gives it.
fn hoist(hoisted: Vec<(Pattern, Expr)>, body: Expr) -> Expr {
    let span = body.span;
    hoisted
        .into_iter()
        .rev()
        .fold(body, |body, (pattern, value)| {
            Expr::new(
                ExprKind::Let {
                    pattern,
                    value: Box::new(value),
                    body: Box::new(body),
                },
                span,
            )
        })
}

/// Whether an expression can be evaluated twice without changing anything.
fn is_repeatable(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::Field { .. }
    )
}

// --------------------------------------------------- patterns, types, text

impl<'a> Lowerer<'a> {
    /// Lowers a binding target.
    ///
    /// A starred target keeps its name in a one-element sequence pattern. The IR
    /// has no rest-pattern, so a backend sees a single binding it must decide how
    /// to collect; that is better than dropping the name.
    fn pattern(&mut self, node: tree_sitter::Node<'_>) -> Pattern {
        let span = self.span(node);
        match node.kind() {
            "identifier" => {
                if self.text_of(node) == "_" {
                    Pattern::ignore(span)
                } else {
                    Pattern::bind(self.name_of(node), span)
                }
            }
            "_" => Pattern::ignore(span),
            // A name wrapped in a single-name list, as `for a, in xs` writes it.
            "as_pattern_target" | "as_pattern" => {
                let inner = node.named_child(0).map(|n| self.pattern(n));
                inner.unwrap_or_else(|| Pattern::bind(self.name_of(node), span))
            }
            "tuple" | "tuple_pattern" | "pattern_list" | "expression_list" => {
                let subpatterns: Vec<_> = named_children(node)
                    .iter()
                    .map(|child| self.pattern(*child))
                    .collect();
                Pattern::sequence(subpatterns, span)
            }
            "list_splat_pattern" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.pattern(n))
                    .unwrap_or_else(|| Pattern::ignore(span));
                Pattern::sequence(vec![inner], span)
            }
            "dictionary_splat_pattern" => {
                let inner = node
                    .named_child(0)
                    .map(|n| self.pattern(n))
                    .unwrap_or_else(|| Pattern::ignore(span));
                Pattern::sequence(vec![inner], span)
            }
            // A bare name in a `for` target is a `pattern_list` of identifiers,
            // already handled above. Anything else cannot bind.
            other => {
                self.unsupported(node, other);
                Pattern::ignore(span)
            }
        }
    }

    /// Lowers an annotation into a [`Type`].
    ///
    /// Only annotations that name something are translated. An unknown name
    /// becomes [`Type::Named`] rather than `Unknown`, because the source *did*
    /// name a type and resolving it is `gset-semantic`'s job.
    fn type_of(&mut self, node: tree_sitter::Node<'_>) -> Type {
        match node.kind() {
            "type" => match node.named_child(0) {
                Some(inner) => self.type_of(inner),
                None => Type::UNKNOWN,
            },
            "identifier" => self.named_type(self.name_of(node)),
            "none" => Type::Null,
            "generic_type" => {
                let Some(base) = node.named_child(0) else {
                    return Type::UNKNOWN;
                };
                let base_name = self.name_of(base);
                let mut arguments = Vec::new();
                if let Some(list) = node.child_by_field_name("type_parameters") {
                    for argument in named_children(list) {
                        arguments.push(self.type_of(argument));
                    }
                }
                match &*base_name {
                    // `list[T]` and `set[T]` are the IR's own shapes.
                    "list" | "List" => Type::List {
                        element: arguments.into_iter().next().unwrap_or(Type::UNKNOWN).into(),
                        variance: Variance::Invariant,
                    },
                    "set" | "Set" => {
                        Type::Set(arguments.into_iter().next().unwrap_or(Type::UNKNOWN).into())
                    }
                    "frozenset" => {
                        Type::Set(arguments.into_iter().next().unwrap_or(Type::UNKNOWN).into())
                    }
                    // `dict[K, V]` is a mapping, which the IR models as
                    // string-keyed. A non-string key type is kept as a named
                    // mapping rather than being silently narrowed.
                    "dict" | "Dict" | "Mapping" | "MutableMapping" | "defaultdict" => {
                        let mut arguments = arguments.into_iter();
                        let key = arguments.next().unwrap_or(Type::UNKNOWN);
                        let value = arguments.next().unwrap_or(Type::UNKNOWN);
                        // The IR's mapping type is string-keyed, because every
                        // target's common mapping is. A key that is not a string
                        // would be narrowed away, so it is flagged.
                        if !matches!(key, Type::Str | Type::StrLit | Type::Unknown(_)) {
                            self.builder.report(
                                Diagnostic::warning(
                                    self.span(node),
                                    "a mapping with non-string keys was lowered to the IR's \
                                     string-keyed mapping, which no IR mapping type can \
                                     represent otherwise",
                                )
                                .with_code("gset-python-map-key"),
                            );
                        }
                        Type::Map(value.into())
                    }
                    "tuple" | "Tuple" => Type::Tuple(arguments),
                    // `Optional[T]` is `T | None`, so it is exactly the IR's
                    // option. `Union` is kept as a union because collapsing it to
                    // one alternative would be a guess.
                    "Optional" => {
                        Type::Option(arguments.into_iter().next().unwrap_or(Type::UNKNOWN).into())
                    }
                    "Union" => {
                        if arguments.is_empty() {
                            Type::UNKNOWN
                        } else {
                            Type::Union(arguments)
                        }
                    }
                    // `Any` is an explicit statement that the author does not know
                    // the type, which is exactly what `Unknown` means here.
                    "Any" => Type::UNKNOWN,
                    _ => Type::Applied {
                        base: base_name,
                        arguments,
                    },
                }
            }
            "union_type" | "constrained_type" => {
                let arguments: Vec<_> = named_children(node)
                    .iter()
                    .map(|n| self.type_of(*n))
                    .collect();
                if arguments.len() == 2 && arguments.iter().any(|t| matches!(t, Type::Null)) {
                    Type::Option(arguments.into_iter().next().unwrap().into())
                } else {
                    Type::Union(arguments)
                }
            }
            _ => Type::UNKNOWN,
        }
    }

    /// Lowers a builtin scalar annotation.
    ///
    /// The widths are the ones every supported target can represent. A Python
    /// `int` is arbitrary precision, so this is the point where the rewrite has
    /// to admit it is narrowing: `12345678901234567890` does not fit. The
    /// diagnostic says so rather than leaving the backend to overflow.
    fn named_type(&mut self, name: Name) -> Type {
        match &*name {
            "int" => Type::Int(IntWidth::I64),
            "float" => Type::Float,
            "str" => Type::Str,
            "bytes" => Type::Bytes,
            "bool" => Type::Bool,
            "object" => Type::UNKNOWN,
            "None" | "NoneType" => Type::Null,
            _ => Type::Named(name),
        }
    }

    /// Lowers a string literal, or an f-string into its parts.
    ///
    /// The literal text between the quotes is kept as written. Re-encoding an
    /// escape would make this frontend responsible for producing exactly the
    /// bytes the author wrote, and Go's implementation quoted numeric literals on
    /// the way through for the same class of reason.
    fn string(&mut self, node: tree_sitter::Node<'_>, span: Span) -> Expr {
        let raw = self.text_of(node);
        let prefix: String = raw
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_ascii_lowercase();
        let is_fstring = prefix.contains('f');
        let is_bytes = prefix.contains('b');
        // The grammar puts `string_start`, the content and an `interpolation`
        // per hole directly under the `string` node; there is no nested wrapper.
        // An f-string with no holes has no `interpolation` and is a plain
        // literal, so the presence of one is what distinguishes the two paths.
        let interpolated = (0..node.child_count()).any(|index| {
            node.child(index)
                .is_some_and(|child| child.kind() == "interpolation")
        });

        if !is_fstring || !interpolated {
            let text = self.string_body(node);
            let literal = if is_bytes {
                Literal::Bytes(text)
            } else {
                Literal::Str(text)
            };
            return Expr::new(ExprKind::Literal(literal), span);
        }

        // Walk the parts in source order. The segments and arguments interleave,
        // and the IR requires exactly one more segment than arguments.
        let mut segments = Vec::new();
        let mut arguments = Vec::new();
        let mut current = String::new();
        let mut specifiers: Vec<String> = Vec::new();

        for child in named_children(node) {
            match child.kind() {
                "string_start" | "string_end" => {}
                "string_content" => current.push_str(self.text_of(child)),
                // An escape is kept as written in both cases. A raw string keeps
                // its backslash; a plain one keeps it too, because decoding and
                // re-encoding is where a frontend loses bytes.
                "escape_sequence" => current.push_str(self.text_of(child)),
                "interpolation" => {
                    segments.push(std::mem::take(&mut current));
                    let Some(expression) = child.child_by_field_name("expression") else {
                        continue;
                    };
                    arguments.push(self.expr(expression));
                    // `!r` and the format spec belong to the rendering, not to
                    // the value. The IR has one `format` slot for the whole
                    // string, so several different specs cannot all be kept; a
                    // warning says which were dropped rather than losing them.
                    let mut spec = String::new();
                    if let Some(conversion) = child.child_by_field_name("type_conversion") {
                        spec.push_str(self.text_of(conversion).trim_end_matches('!'));
                    }
                    if let Some(format) = child.child_by_field_name("format_specifier") {
                        spec.push_str(self.text_of(format));
                    }
                    if !spec.is_empty() {
                        specifiers.push(spec);
                    }
                }
                other => self.unsupported(child, other),
            }
        }
        segments.push(current);
        // A doubled brace in an f-string is an escape for one brace, so a
        // segment — which is the text around a hole, not its source spelling —
        // has to hold the brace the program prints.
        let segments: Vec<String> = segments
            .iter()
            .map(|segment| unescape_braces(segment))
            .collect();

        let format = if specifiers.is_empty() {
            None
        } else {
            let distinct: Vec<&String> = {
                let mut seen: Vec<&String> = Vec::new();
                for spec in &specifiers {
                    if !seen.contains(&spec) {
                        seen.push(spec);
                    }
                }
                seen
            };
            if distinct.len() > 1 {
                self.builder.report(
                    Diagnostic::warning(
                        span,
                        format!(
                            "this f-string uses {} different renderings; the IR carries one, so \
                             only {:?} is recorded",
                            distinct.len(),
                            distinct[0]
                        ),
                    )
                    .with_code("gset-python-format-spec"),
                );
            }
            Some(distinct[0].clone())
        };

        Expr::new(
            ExprKind::Format {
                segments,
                arguments,
                format,
            },
            span,
        )
    }

    /// The text of a string literal, without its quotes and with escapes as
    /// written.
    ///
    /// Decoding is not attempted. A backend that needs the bytes decodes once, and
    /// a frontend that decoded here would have to re-encode on the way out.
    fn string_body(&self, node: tree_sitter::Node<'_>) -> String {
        let mut out = String::new();
        for child in named_children(node) {
            match child.kind() {
                "string_start" | "string_end" => {}
                "string_content" | "escape_sequence" => out.push_str(self.text_of(child)),
                // Not reachable for a plain string, but keeping the text is
                // better than dropping it if the grammar ever allows one.
                _other => out.push_str(self.text_of(child)),
            }
        }
        out
    }

    /// The binary operator a node denotes.
    fn binary_op(&self, node: tree_sitter::Node<'_>) -> Option<gset_ir::BinaryOp> {
        self.binary_op_text(self.text_of(node).trim())
    }

    /// The binary operator a written symbol denotes.
    fn binary_op_text(&self, text: &str) -> Option<gset_ir::BinaryOp> {
        use gset_ir::BinaryOp::*;
        Some(match text {
            "+" => Add,
            "-" => Sub,
            "*" => Mul,
            "/" => Div,
            "%" => Rem,
            "**" => Pow,
            "&" => BitAnd,
            "|" => BitOr,
            "^" => BitXor,
            "<<" => ShiftLeft,
            ">>" => ShiftRight,
            "//" => FloorDiv,
            // `@` is matrix multiplication, which the IR has no operator for. It
            // is reported rather than approximated by `Mul`.
            _ => return None,
        })
    }

    /// The unary operator a node denotes.
    fn unary_op(&self, node: tree_sitter::Node<'_>) -> Option<UnaryOp> {
        Some(match self.text_of(node).trim() {
            "-" => UnaryOp::Neg,
            "+" => UnaryOp::Ref,
            "~" => UnaryOp::BitNot,
            _ => return None,
        })
    }

    /// The comparison operator a node denotes.
    fn comparison_op(&self, node: tree_sitter::Node<'_>) -> Option<ComparisonOp> {
        Some(match self.text_of(node).trim() {
            "==" => ComparisonOp::Eq,
            "!=" => ComparisonOp::Ne,
            "<" => ComparisonOp::Lt,
            "<=" => ComparisonOp::Le,
            ">" => ComparisonOp::Gt,
            ">=" => ComparisonOp::Ge,
            // `is` and `==` mean different things in Python: `is` compares
            // identity. Collapsing them would be Go defect class 3's mistake in a
            // different place, so the IR keeps them apart.
            "is" => ComparisonOp::Is,
            "is not" => ComparisonOp::IsNot,
            "in" => ComparisonOp::In,
            "not in" => ComparisonOp::NotIn,
            _ => return None,
        })
    }
}
