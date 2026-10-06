//! The Go backend.
//!
//! Emits a complete, `gofmt`-clean Go file for the IR subset it supports.
//! Anything outside that subset produces an error diagnostic and, where a
//! placeholder keeps the output parseable, a `panic("gset: unsupported …")`
//! rather than a silently wrong translation.
//!
//! # What this backend refuses to guess
//!
//! Go is statically typed and Python is not, so the hard part is not syntax but
//! types. This backend never invents a concrete type for a value the source left
//! untyped: it emits `interface{}`. That is the honest translation of "the
//! frontend does not know", and it is what keeps a wrong guess out of compiled
//! output. `gset-semantic` is where real types will come from.
//!
//! # A Go program is a package and a `main`
//!
//! Python executes module-level statements; Go does not. Every top-level
//! statement is emitted inside `func main`, in source order, and declarations
//! (functions, globals) are emitted at package scope. Emitting a top-level
//! `print(...)` where it appeared would be a Go syntax error, which is the
//! class of defect the M2 gate exists to catch.

use std::collections::{BTreeSet, HashMap, HashSet};

use gset_ir::{
    BinaryOp, Block, ComparisonOp, ComprehensionKind, Else, Expr, ExprKind, Function, Import,
    ImportKind, Item, Literal, LogicalOp, Module, NamedArg, Pattern, PatternKind, SourceMap, Span,
    Stmt, Type, UnaryOp, VarDecl,
};

use crate::backend::{Backend, unsupported};
use crate::target::{Capability, Support, TargetId};
use crate::writer::CodeWriter;
use gset_ir::DiagnosticBag;

/// The Go backend.
pub struct Go;

impl Backend for Go {
    fn id(&self) -> TargetId {
        TargetId::GO
    }

    fn indentation(&self) -> &'static str {
        // Go's own convention, which `gofmt` enforces. Four spaces here was the
        // Go emitter's bug and is why generated Go never passed `gofmt`.
        "\t"
    }

    fn capability(&self, capability: Capability) -> Support {
        use Capability::*;
        match capability {
            Imports | Classes => Support::Desugared,
            // A Python module-level construct becomes a Go declaration plus a
            // `main` statement. Not native, but faithful.
            Records | Enums | Interfaces | TypeAliases => Support::Unsupported,
            Comprehensions => Support::Unsupported,
            Generators => Support::Unsupported,
            Exceptions => Support::Unsupported,
            ContextManagers => Support::Unsupported,
            Decorators => Support::Unsupported,
            Async => Support::Unsupported,
            // An f-string becomes `"lit" + fmt.Sprint(x) + "lit"`, which is
            // faithful for a value with no format spec.
            FStrings => Support::Desugared,
            Closures => Support::Native,
            PatternMatching => Support::Unsupported,
            Slices => Support::Unsupported,
            KeywordArguments => Support::Unsupported,
            MultipleAssignment => Support::Desugared,
            // `**` needs `math.Pow`, `//` needs a floor helper. A later slice
            // adds the helper pass rather than approximating with the wrong
            // operator.
            OperatorCall => Support::Unsupported,
            LoopElse => Support::Unsupported,
        }
    }

    fn emit_module(
        &self,
        module: &Module,
        _source_map: &SourceMap,
        writer: &mut CodeWriter,
        diagnostics: &mut DiagnosticBag,
    ) {
        // Build the body first so `fmt` is only imported when something
        // actually needs it. An unused import is a Go compile error, so
        // emitting it unconditionally would fail every program that does not
        // print.
        // Two passes, because the helper a program needs is not known until the
        // statement that uses it has been rendered, while the helpers have to be
        // written before it. The first pass discovers them and its diagnostics
        // are thrown away, since the second pass reports the same ones for real.
        let mut discovery = DiagnosticBag::new();
        let mut probe = CodeWriter::new(self.indentation());
        let discovered = {
            let mut emitter = Emitter::new(&mut probe, &mut discovery);
            emitter.emit(module);
            emitter.helpers.clone()
        };

        let mut body = CodeWriter::new(self.indentation());
        let (imports, text) = {
            let mut emitter = Emitter::new(&mut body, diagnostics);
            emitter.helpers = discovered;
            emitter.emit(module);
            (emitter.imports, body.into_string())
        };

        writer.writeln("package main");
        writer.blank();
        if !imports.is_empty() {
            // Collected in a sorted set because Go compiles imports in source
            // order but the set decides which are needed: emitting them in
            // discovery order would make output depend on the order the
            // constructs appeared, and the golden would churn.
            if imports.len() == 1 {
                let path = imports.iter().next().expect("one import");
                writer.writeln(&format!("import \"{path}\""));
            } else {
                writer.writeln("import (");
                writer.indent();
                for path in &imports {
                    writer.writeln(&format!("\"{path}\""));
                }
                writer.dedent();
                writer.writeln(")");
            }
            writer.blank();
        }
        writer.write(&text);
    }
}

/// One emission run.
struct Emitter<'a> {
    out: &'a mut CodeWriter,
    diagnostics: &'a mut DiagnosticBag,
    /// Standard library packages the emitted program imports.
    imports: BTreeSet<&'static str>,
    /// Helper functions the emitted program needs, emitted once each.
    helpers: BTreeSet<&'static str>,
    /// Top-level statements, collected so they can be emitted inside `main`.
    ///
    /// Cloned rather than borrowed: the borrow checker cost of holding module
    /// references alongside the writer is paid once per run, and a module's
    /// statement list is small.
    main_statements: Vec<Stmt>,
    /// Names declared at package scope, so a function-local assignment promotes
    /// with `:=` only when the name is genuinely new.
    globals: HashSet<String>,
    /// Names declared in the current function scope.
    locals: HashSet<String>,
    /// The names this emitter declared as `interface{}` even though inference
    /// gave them a type.
    ///
    /// Everything downstream asks what Go type an expression will have, and
    /// for one of these names the answer is `interface{}` however confident the
    /// inference was about the single assignment it saw.
    dynamic_locals: HashSet<String>,
    /// The Go types each name is bound to somewhere in the current function.
    ///
    /// A Python name is a reference to whatever object was last assigned to it,
    /// so a name the function binds to an `int` and later to an arithmetic
    /// result of unknown type is one value that can be either. Go's `:=` fixes
    /// a variable's type at its first assignment, so those names are declared
    /// `interface{}` and every assignment lands in them unchanged.
    bind_types: HashMap<String, BTreeSet<String>>,
    /// Every name the current function reads.
    ///
    /// Go rejects a local that is declared and never used, while Python has no
    /// such rule: `if items: found = items[0]` binds a name the program never
    /// mentions again. The emitter has to know which names that applies to
    /// before it can emit the declaration Go will accept.
    reads: HashSet<String>,
    /// The declared type of each annotated parameter of the current function.
    ///
    /// Python states a parameter's type in the source, so a use of that
    /// parameter inside the body has a type without any inference having to
    /// rediscover it. Inference does not flow that far yet, so this is where the
    /// emitter looks before deciding a value is dynamic.
    param_types: HashMap<String, Type>,
    /// Whether the current function returns a value, which decides whether a
    /// bare `return` becomes `return nil`.
    returns_value: bool,
    /// How many multi-line expressions are still being built around this one.
    ///
    /// An expansion that contains another expansion — a comprehension whose
    /// element is itself a comprehension — has to indent the inner one's lines by
    /// the block it sits in, and the inner one is rendered before the outer one
    /// knows where it will land. This counts the blocks in between.
    expression_depth: usize,
    /// Names of the flags that mark "this loop was left by `break`", innermost
    /// last.
    ///
    /// Python runs a loop's `else` only when the loop finished without a
    /// `break`, which Go has no syntax for: `break` and falling out of a `for`
    /// are indistinguishable afterwards. A flag set by every `break` inside the
    /// loop restores the distinction, and it has to be threaded through the
    /// body because that is where the `break`s are.
    loop_else_flags: Vec<String>,
    /// Every name the module binds somewhere, so a call can be told apart from
    /// a name that was never defined.
    ///
    /// The set is a whole-module approximation on purpose: it over-approximates,
    /// so a call to a name bound in some other scope still emits. Under-claiming
    /// would report a program that is fine, and a false report is as wrong as a
    /// false silence.
    declared: HashSet<String>,
    /// Imports this backend dropped because Go has no equivalent.
    ///
    /// The name is recorded rather than forgotten, so a use of it reports the
    /// import instead of emitting a call to something that was never defined.
    dropped_imports: HashSet<String>,
    /// Whether the expression being emitted is a statement of its own.
    ///
    /// Python's mutating methods return `None` but Go's equivalent returns the
    /// collection, so the expansion is an assignment and needs statement
    /// position to have anywhere to put it. Nested inside a larger expression
    /// there is nowhere, and the method is reported instead.
    in_statement: bool,
}

impl<'a> Emitter<'a> {
    fn new(out: &'a mut CodeWriter, diagnostics: &'a mut DiagnosticBag) -> Self {
        Emitter {
            out,
            diagnostics,
            imports: BTreeSet::new(),
            helpers: BTreeSet::new(),
            main_statements: Vec::new(),
            globals: HashSet::new(),
            locals: HashSet::new(),
            dynamic_locals: HashSet::new(),
            bind_types: HashMap::new(),
            reads: HashSet::new(),
            param_types: HashMap::new(),
            returns_value: false,
            loop_else_flags: Vec::new(),
            declared: HashSet::new(),
            dropped_imports: HashSet::new(),
            expression_depth: 0,
            in_statement: false,
        }
    }

    // -------------------------------------------------------------- module

    fn emit(&mut self, module: &Module) {
        // Names first, so a call can be checked against what the module binds
        // even when the definition comes later in the source — and it does,
        // because Python resolves names when they are called, not where they are
        // written.
        self.collect_declared(module);
        // Declarations first, so a global's initialiser can call a function
        // that appears later in the source: Go does not care about order.
        for item in &module.items {
            match item {
                Item::Import(import) => self.emit_import(import),
                Item::Global(global) => self.emit_global(global),
                Item::Function(function) => self.emit_function(function),
                Item::Class(_)
                | Item::Record(_)
                | Item::Enum(_)
                | Item::Interface(_)
                | Item::TypeAlias(_) => {
                    unsupported(self.diagnostics, item.span(), describe_item(item));
                }
                Item::Stmt(statement) => self.main_statements.push(statement.clone()),
            }
        }

        self.emit_helpers();
        self.emit_main();
    }

    /// Emits the helper functions the program called, once each.
    ///
    /// Helpers are emitted after the program's own declarations and before
    /// `main`, so a helper can be used by a declaration that textually precedes
    /// it, which is what Go requires of nothing but is the least surprising
    /// order for a reader.
    fn emit_helpers(&mut self) {
        for name in std::mem::take(&mut self.helpers) {
            let Some(helper) = helpers().iter().find(|helper| helper.name == name) else {
                continue;
            };
            for path in helper.imports {
                self.imports.insert(path);
            }
            self.out.blank();
            self.out.writeln(&format!("{} {{", helper.signature));
            self.out.indent();
            for line in helper.body {
                if line.is_empty() {
                    self.out.blank();
                } else {
                    self.out.writeln(line);
                }
            }
            self.out.dedent();
            self.out.writeln("}");
        }
    }

    /// Records that the program uses a helper, and returns its name.
    ///
    /// A helper that calls another helper pulls it in too: emitting `gsetGetOr`
    /// without `gsetGet` would produce a file that does not compile, which is the
    /// one outcome worse than reporting the construct.
    fn use_helper(&mut self, name: &'static str) -> &'static str {
        self.helpers.insert(name);
        let table = helpers();
        let mut pending: Vec<&'static str> = table
            .iter()
            .find(|helper| helper.name == name)
            .map(|helper| helper.deps.to_vec())
            .unwrap_or_default();
        while let Some(dependency) = pending.pop() {
            if !self.helpers.insert(dependency) {
                continue;
            }
            if let Some(helper) = table.iter().find(|helper| helper.name == dependency) {
                pending.extend(helper.deps.iter().copied());
            }
        }
        name
    }

    fn emit_main(&mut self) {
        // A bare `package main` with no `main` does not build, so `main` is
        // always emitted, even empty. A transpiled program is an executable.
        self.out.blank();
        self.out.writeln("func main() {");
        self.out.indent();
        self.locals.clear();
        self.returns_value = false;
        let statements = std::mem::take(&mut self.main_statements);
        for statement in &statements {
            self.emit_stmt(statement);
        }
        self.out.dedent();
        self.out.writeln("}");
    }

    fn emit_import(&mut self, import: &Import) {
        // A module whose whole content is compile-time (type hints, future
        // directives) has no runtime effect in the source, so dropping it
        // cannot change what the program does. Everything else is reported:
        // silently dropping a runtime import is the defect the IR's mandatory
        // import item exists to prevent.
        let head = import
            .path
            .segments
            .first()
            .map(|segment| segment.to_string())
            .unwrap_or_default();
        if matches!(head.as_str(), "typing" | "__future__" | "typing_extensions") {
            return;
        }
        // Every name the import brings into scope. A whole-module import binds
        // the module path (`import os` binds `os`); a named import binds each
        // name it names, under its alias where it has one.
        match &import.kind {
            ImportKind::Module => {
                if let Some(alias) = &import.alias {
                    self.dropped_imports.insert(alias.to_string());
                }
                if let Some(head) = import.path.segments.first() {
                    self.dropped_imports.insert(head.to_string());
                }
            }
            ImportKind::Named(names) => {
                for name in names {
                    self.dropped_imports.insert(name.local().to_string());
                }
            }
            ImportKind::Default(name) => {
                self.dropped_imports.insert(name.local().to_string());
            }
            // `from x import *` binds names the source never wrote down, so
            // there is nothing to record: a use of one is a name the module
            // never binds, which is reported as such.
            ImportKind::Star => {}
        }
        self.diagnostics.push(
            gset_ir::Diagnostic::warning(
                import.span,
                format!(
                    "import `{}` has no Go equivalent and was dropped; a dependency resolver \
                     must map it before this target can compile",
                    import
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.as_ref())
                        .collect::<Vec<_>>()
                        .join(".")
                ),
            )
            .with_code("gset-backend-import"),
        );
    }

    /// Records every name the module binds, in every scope.
    fn collect_declared(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                // An import this backend cannot map binds nothing, so its
                // names are deliberately absent from `declared`: a use of one
                // is reported as a dropped import rather than treated as a
                // name the module defines.
                Item::Import(_) => {}
                Item::Function(function) => self.collect_declared_function(function),
                Item::Global(global) => {
                    for name in global.pattern.bound_names() {
                        self.declared.insert(name.to_string());
                    }
                }
                Item::Stmt(statement) => self.collect_declared_statement(statement),
                _ => {}
            }
        }
    }

    fn collect_declared_function(&mut self, function: &Function) {
        self.declared.insert(function.name.to_string());
        for param in &function.params {
            for name in param.bound_names() {
                self.declared.insert(name.to_string());
            }
        }
        for statement in &function.body.statements {
            self.collect_declared_statement(statement);
        }
    }

    fn collect_declared_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Assign { target, .. } => {
                for name in target.bound_names() {
                    self.declared.insert(name.to_string());
                }
            }
            Stmt::Decl(decl) => {
                for name in decl.pattern.bound_names() {
                    self.declared.insert(name.to_string());
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                for statement in &then_branch.statements {
                    self.collect_declared_statement(statement);
                }
                match else_branch.as_deref() {
                    Some(Else::Block(block)) => {
                        for statement in &block.statements {
                            self.collect_declared_statement(statement);
                        }
                    }
                    Some(Else::If(nested)) => self.collect_declared_statement(nested),
                    None => {}
                }
            }
            Stmt::While {
                body, else_body, ..
            }
            | Stmt::ForIn {
                body, else_body, ..
            } => {
                for statement in &body.statements {
                    self.collect_declared_statement(statement);
                }
                if let Some(else_body) = else_body {
                    for statement in &else_body.statements {
                        self.collect_declared_statement(statement);
                    }
                }
            }
            Stmt::For { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::With { body, .. } => {
                for statement in &body.statements {
                    self.collect_declared_statement(statement);
                }
            }
            Stmt::LocalItem(item) => {
                if let Item::Function(function) = item.as_ref() {
                    self.collect_declared_function(function);
                }
            }
            _ => {}
        }
    }

    fn emit_global(&mut self, global: &VarDecl) {
        self.out.blank();
        let Some(name) = global.pattern.single_binding() else {
            unsupported(
                self.diagnostics,
                global.span,
                "a destructuring global binding",
            );
            return;
        };
        self.globals.insert(name.to_string());
        let value = global.value.as_ref().map(|value| self.emit_expr(value));
        match (global.ty.as_ref(), value) {
            (Some(ty), Some(value)) => {
                let ty = go_type(ty);
                self.out.writeln(&format!("var {name} {ty} = {value}"));
            }
            (Some(ty), None) => {
                let ty = go_type(ty);
                self.out.writeln(&format!("var {name} {ty}"));
            }
            (None, Some(value)) => self.out.writeln(&format!("var {name} = {value}")),
            (None, None) => self.out.writeln(&format!("var {name} interface{{}}")),
        }
    }

    fn emit_function(&mut self, function: &Function) {
        if !function.decorators.is_empty() {
            unsupported(self.diagnostics, function.span, "a decorated function");
        }
        if function.is_async {
            unsupported(self.diagnostics, function.span, "an async function");
        }

        let params = self.emit_params(function);
        let returns_value = function_returns_value(function);
        let ret = function_return_type(function, returns_value);
        let signature = match &ret {
            Some(ret) => format!("func {}{params} {ret} {{", function.name),
            None => format!("func {}{params} {{", function.name),
        };
        self.out.blank();

        let outer_locals = std::mem::take(&mut self.locals);
        let outer_dynamic_locals = std::mem::take(&mut self.dynamic_locals);
        let outer_bind_types = std::mem::take(&mut self.bind_types);
        let outer_reads = std::mem::take(&mut self.reads);
        let outer_param_types = std::mem::take(&mut self.param_types);
        let outer_returns = self.returns_value;
        self.returns_value = returns_value;

        self.out.writeln(&signature);
        self.out.indent();
        for statement in &function.body.statements {
            collect_reads_statement(statement, &mut self.reads);
            collect_bind_types_statement(statement, &mut self.bind_types);
        }
        self.bind_params(function);
        self.emit_block_body(&function.body);
        // Python reaches the end of a function by returning `None`, and Go
        // rejects a function whose result is missing on some path. The zero
        // value stands for the `None`, which is the only answer Go can give and
        // the one a caller comparing against `nil` expects.
        if let Some(ret) = ret.as_ref().filter(|_| !always_returns(&function.body)) {
            self.out.writeln(&format!("return {}", zero_value(ret)));
        }
        self.out.dedent();
        self.out.writeln("}");

        self.locals = outer_locals;
        self.dynamic_locals = outer_dynamic_locals;
        self.bind_types = outer_bind_types;
        self.reads = outer_reads;
        self.param_types = outer_param_types;
        self.returns_value = outer_returns;
    }

    fn emit_params(&mut self, function: &Function) -> String {
        let mut parts = Vec::new();
        for (index, pattern) in function.params.iter().enumerate() {
            let name = match pattern.single_binding() {
                Some(name) => name.to_string(),
                None => "_".to_string(),
            };
            let ty = function
                .param_types
                .get(index)
                .and_then(|ty| ty.as_ref())
                .map(go_type)
                .unwrap_or_else(|| "interface{}".to_string());
            let variadic = function.variadic && index + 1 == function.params.len();
            if variadic {
                parts.push(format!("{name} ...{ty}"));
            } else {
                parts.push(format!("{name} {ty}"));
            }
        }
        format!("({})", parts.join(", "))
    }

    fn bind_params(&mut self, function: &Function) {
        for (index, pattern) in function.params.iter().enumerate() {
            for name in pattern.bound_names() {
                self.locals.insert(name.to_string());
                if let Some(Some(ty)) = function.param_types.get(index) {
                    self.param_types.insert(name.to_string(), ty.clone());
                }
            }
        }
    }

    // ---------------------------------------------------------- statements

    fn emit_block_body(&mut self, block: &Block) {
        // A Go block may be empty, so no filler statement is needed.
        for statement in &block.statements {
            self.emit_stmt(statement);
        }
    }

    fn emit_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Decl(decl) => self.emit_local_decl(decl),
            Stmt::Assign { target, value, .. } => self.emit_assign(target, value),
            Stmt::Expr(expr) => self.emit_expr_statement(expr),
            Stmt::Return { value, .. } => self.emit_return(value.as_ref()),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.emit_if(condition, then_branch, else_branch.as_deref()),
            Stmt::While {
                condition,
                body,
                else_body,
                span,
            } => self.emit_while(condition, body, else_body.as_ref(), *span),
            Stmt::ForIn {
                pattern,
                iterable,
                body,
                else_body,
                span,
                ..
            } => self.emit_for_in(pattern, iterable, body, else_body.as_ref(), *span),
            Stmt::Block(block) => {
                self.out.writeln("{");
                self.out.indent();
                self.emit_block_body(block);
                self.out.dedent();
                self.out.writeln("}");
            }
            Stmt::Break { label, span } => {
                if label.is_some() {
                    unsupported(self.diagnostics, *span, "a labelled `break`");
                }
                // A loop with an `else` clause runs it only when no `break`
                // escaped, so the escape has to be recorded before it happens.
                if let Some(flag) = self.loop_else_flags.last().cloned() {
                    self.out.writeln(&format!("{flag} = false"));
                }
                self.out.writeln("break");
            }
            Stmt::Continue { label, span } => {
                if label.is_some() {
                    unsupported(self.diagnostics, *span, "a labelled `continue`");
                }
                self.out.writeln("continue");
            }
            Stmt::Assert {
                condition, message, ..
            } => {
                let condition = self.emit_condition(condition);
                self.out.writeln(&format!("if !({condition}) {{"));
                self.out.indent();
                match message {
                    Some(message) => {
                        let message = self.emit_expr(message);
                        self.out.writeln(&format!("panic({message})"));
                    }
                    None => self.out.writeln("panic(\"assertion failed\")"),
                }
                self.out.dedent();
                self.out.writeln("}");
            }
            Stmt::Throw {
                value: Some(value), ..
            } => {
                let value = self.emit_expr(value);
                self.out.writeln(&format!("panic({value})"));
            }
            Stmt::Throw { value: None, span } => {
                // A bare re-throw has no Go form: Go panics are values, and
                // recovering `panic(nil)` is not reliable enough to fake it.
                unsupported(self.diagnostics, *span, "a bare re-throw");
            }
            Stmt::Defer { expr, .. } => {
                // A bare call is `defer f(...)`. A non-call cannot be deferred,
                // so it is wrapped in a closure.
                if matches!(
                    expr.kind,
                    ExprKind::Call { .. } | ExprKind::MethodCall { .. }
                ) {
                    let value = self.emit_expr(expr);
                    self.out.writeln(&format!("defer {value}"));
                } else {
                    let value = self.emit_expr(expr);
                    self.out
                        .writeln(&format!("defer func() {{ _ = {value} }}()"));
                }
            }
            Stmt::LocalItem(item) => self.emit_local_item(item),
            Stmt::Empty { .. } => {}
            Stmt::Error { reason, .. } => {
                self.out.writeln(&format!("// gset: {reason}"));
            }
            other => {
                // Every remaining variant is one Go has no direct statement
                // for. Reported with its span rather than dropped, which is the
                // whole reason `Stmt` has a variant per construct.
                unsupported(self.diagnostics, other.span(), describe_stmt(other));
            }
        }
    }

    fn emit_local_decl(&mut self, decl: &VarDecl) {
        let value = decl.value.as_ref().map(|value| self.emit_expr(value));
        match (&decl.pattern.kind, decl.ty.as_ref()) {
            (PatternKind::Bind, Some(ty)) => {
                let name = pattern_name(&decl.pattern);
                self.locals.insert(name.clone());
                let ty = go_type(ty);
                if self.bind_types_have_more_than_one(&name) {
                    self.declare_dynamic(&name, value);
                    self.discard_unused(&name);
                    return;
                }
                match value {
                    Some(value) => self.out.writeln(&format!("var {name} {ty} = {value}")),
                    None => self.out.writeln(&format!("var {name} {ty}")),
                }
                self.discard_unused(&name);
            }
            (PatternKind::Bind, None) => {
                let name = pattern_name(&decl.pattern);
                let declared = self.locals.contains(&name) || self.globals.contains(&name);
                self.locals.insert(name.clone());
                match value {
                    // A declaration with no type and no value has no Go form
                    // that infers anything, so it is explicitly `interface{}`.
                    None => self.out.writeln(&format!("var {name} interface{{}}")),
                    Some(value) if declared => {
                        self.out.writeln(&format!("{name} = {value}"));
                    }
                    Some(value) if self.bind_types_have_more_than_one(&name) => {
                        self.declare_dynamic(&name, Some(value));
                        self.discard_unused(&name);
                    }
                    Some(value) => {
                        self.out.writeln(&format!("{name} := {value}"));
                        self.discard_unused(&name);
                    }
                }
            }
            (PatternKind::Ignore, _) => {
                if let Some(value) = value {
                    self.out.writeln(&format!("_ = {value}"));
                }
            }
            _ => unsupported(self.diagnostics, decl.span, "a destructuring declaration"),
        }
    }

    /// Declares a local whose Go type is `interface{}`, whether or not it has a
    /// value yet.
    fn declare_dynamic(&mut self, name: &str, value: Option<String>) {
        self.dynamic_locals.insert(name.to_string());
        match value {
            Some(value) => self
                .out
                .writeln(&format!("var {name} interface{{}} = {value}")),
            None => self.out.writeln(&format!("var {name} interface{{}}")),
        }
    }

    /// The Go type this emitter will write for `expr`.
    ///
    /// This is the expression's inferred type unless the name it refers to was
    /// declared `interface{}` here, which happens when the function binds that
    /// name to values of more than one Go type. Asking the wrong one produces a
    /// file Go rejects: `interface{} > 2` is not an operator, and no `int` and
    /// no `float64` are what the variable holds.
    fn go_type_of(&self, expr: &Expr) -> Option<String> {
        if let ExprKind::Path(path) = &expr.kind
            && path.is_bare()
            && let Some(name) = path.segments.first()
            && self.dynamic_locals.contains(name.as_ref())
        {
            return None;
        }
        concrete_type(expr)
    }

    /// Whether `name` is bound to values of more than one Go type in this
    /// function.
    fn bind_types_have_more_than_one(&self, name: &str) -> bool {
        self.bind_types
            .get(name)
            .is_some_and(|types| types.len() > 1)
    }

    /// Marks a just-declared local as used when nothing in the function reads
    /// it.
    ///
    /// The assignment does nothing at runtime and is exactly what Go's own
    /// tooling suggests for a binding the program does not otherwise need.
    fn discard_unused(&mut self, name: &str) {
        if !self.reads.contains(name) {
            self.out.writeln(&format!("_ = {name}"));
        }
    }

    fn emit_assign(&mut self, target: &Pattern, value: &Expr) {
        match &target.kind {
            PatternKind::Bind => {
                let name = pattern_name(target);
                let declared = self.locals.contains(&name) || self.globals.contains(&name);
                self.locals.insert(name.clone());
                let value = self.emit_expr(value);
                if declared {
                    self.out.writeln(&format!("{name} = {value}"));
                } else if self.bind_types_have_more_than_one(&name) {
                    self.declare_dynamic(&name, Some(value));
                    self.discard_unused(&name);
                } else {
                    self.out.writeln(&format!("{name} := {value}"));
                    self.discard_unused(&name);
                }
            }
            PatternKind::Ignore => {
                let value = self.emit_expr(value);
                self.out.writeln(&format!("_ = {value}"));
            }
            PatternKind::Location(location) => {
                let location = self.emit_expr(location);
                let value = self.emit_expr(value);
                self.out.writeln(&format!("{location} = {value}"));
            }
            PatternKind::Sequence => self.emit_sequence_assign(target, value),
            PatternKind::Mapping => {
                unsupported(
                    self.diagnostics,
                    target.span,
                    "a mapping destructuring assignment",
                );
            }
        }
    }

    fn emit_sequence_assign(&mut self, target: &Pattern, value: &Expr) {
        let names: Vec<String> = target
            .subpatterns
            .iter()
            .map(|subpattern| match subpattern.single_binding() {
                Some(name) => name.to_string(),
                None => "_".to_string(),
            })
            .collect();
        // Only a matching tuple literal is expressible: Go has no destructuring
        // assignment from an arbitrary expression.
        if let ExprKind::Tuple { elements } = &value.kind
            && elements.len() == names.len()
        {
            let values: Vec<String> = elements
                .iter()
                .map(|element| self.emit_expr(element))
                .collect();
            let fresh = names.iter().any(|name| {
                name != "_" && !self.locals.contains(name) && !self.globals.contains(name)
            });
            for name in &names {
                if name != "_" {
                    self.locals.insert(name.clone());
                }
            }
            let operator = if fresh { ":=" } else { "=" };
            self.out.writeln(&format!(
                "{} {operator} {}",
                names.join(", "),
                values.join(", ")
            ));
            return;
        }
        unsupported(
            self.diagnostics,
            target.span,
            "a multiple assignment whose right-hand side is not a tuple literal",
        );
    }

    fn emit_expr_statement(&mut self, expr: &Expr) {
        // Everything inside this statement is statement position, including the
        // expressions nested in it: `xs.append(f())` is still a statement.
        let outer = std::mem::replace(&mut self.in_statement, true);
        self.emit_expr_statement_inner(expr);
        self.in_statement = outer;
    }

    fn emit_expr_statement_inner(&mut self, expr: &Expr) {
        // A compound assignment arrives as an expression so it can carry the
        // previous value and the operator. Go spells it `target op= value`.
        if let ExprKind::Assign {
            target,
            op,
            value,
            previous,
        } = &expr.kind
        {
            match op {
                Some(op) => {
                    let Some(symbol) = compound_symbol(*op) else {
                        unsupported(self.diagnostics, expr.span, "this compound operator");
                        return;
                    };
                    let target = self.emit_pattern(target);
                    let value = self.emit_expr(value);
                    self.out.writeln(&format!("{target} {symbol}= {value}"));
                }
                None => {
                    if previous.is_some() {
                        // `previous` present with no operator would be
                        // malformed; do not silently drop the value.
                        let value = self.emit_expr(value);
                        let target = self.emit_pattern(target);
                        self.out.writeln(&format!("{target} = {value}"));
                    } else {
                        let value = self.emit_expr(value);
                        let target = self.emit_pattern(target);
                        self.out.writeln(&format!("{target} = {value}"));
                    }
                }
            }
            return;
        }
        // Go only accepts a call as a bare expression statement. A Python
        // expression statement can be any expression; a docstring is a bare
        // string literal, and Go rejects `""` as an unused value. Literals and
        // names have no side effect, so they are dropped; anything that might
        // (a chained comparison of calls, say) is assigned to the blank
        // identifier so it still evaluates.
        match &expr.kind {
            ExprKind::Literal(_) | ExprKind::Path(_) => {}
            ExprKind::Call { .. } | ExprKind::MethodCall { .. } => {
                let value = self.emit_expr(expr);
                if !value.is_empty() {
                    self.out.writeln(&value);
                }
            }
            _ => {
                let value = self.emit_expr(expr);
                if !value.is_empty() {
                    self.out.writeln(&format!("_ = {value}"));
                }
            }
        }
    }

    fn emit_return(&mut self, value: Option<&Expr>) {
        match value {
            Some(value) => {
                let value = self.emit_expr(value);
                self.out.writeln(&format!("return {value}"));
            }
            None => {
                if self.returns_value {
                    self.out.writeln("return nil");
                } else {
                    self.out.writeln("return");
                }
            }
        }
    }

    /// Renders a control-clause condition.
    ///
    /// `emit_expr` parenthesises every binary, comparison and logical
    /// expression so nested precedence is always explicit. `gofmt` removes
    /// exactly those outer parentheses from `if`/`for` headers, so the emitter
    /// removes them itself rather than leaving output that reformats.
    fn emit_condition(&mut self, condition: &Expr) -> String {
        let rendered = self.emit_expr(condition);
        match &condition.kind {
            ExprKind::Binary { .. } | ExprKind::Compare { .. } | ExprKind::Logical { .. } => {
                rendered
                    .strip_prefix('(')
                    .and_then(|inner| inner.strip_suffix(')'))
                    .map(str::to_string)
                    .unwrap_or(rendered)
            }
            _ => self.emit_truthy(condition, rendered),
        }
    }

    /// Wraps a condition that is not already a bool in Go's truthiness test.
    ///
    /// Python lets any value be a condition; Go does not. Emitting the value
    /// bare is a compile error for everything but a bool, so anything else is
    /// asked about at runtime, which is what Python does.
    fn emit_truthy(&mut self, expr: &Expr, rendered: String) -> String {
        if matches!(expr.ty, Type::Bool) {
            return rendered;
        }
        let helper = self.use_helper("gsetTruthy");
        format!("{helper}({rendered})")
    }

    fn emit_if(&mut self, condition: &Expr, then_branch: &Block, else_branch: Option<&Else>) {
        self.hoist_branch_bindings(then_branch, else_branch);
        let condition = self.emit_condition(condition);
        self.out.writeln(&format!("if {condition} {{"));
        self.out.indent();
        self.emit_block_body(then_branch);
        self.out.dedent();
        self.emit_else(else_branch);
        self.out.writeln("}");
    }

    /// Declares, ahead of an `if`, every name that all of its paths bind.
    ///
    /// Python binds a name when a branch assigns it, and the binding outlives
    /// the branch: `if c: chosen = "fast" else: chosen = "slow"` leaves
    /// `chosen` readable afterwards. Go's `:=` is scoped to the block it
    /// appears in, so the same source leaves `chosen` undefined and the
    /// program does not compile. Emitting the declaration here, and letting
    /// the branches assign to it, is the form that has the same meaning.
    ///
    /// Only names every path binds are hoisted. A name some path leaves
    /// unbound is read as a `NameError` in Python, and Go's zero value is not
    /// one, so a declaration there would quietly answer a different question.
    fn hoist_branch_bindings(&mut self, then_branch: &Block, else_branch: Option<&Else>) {
        let Some(else_branch) = else_branch else {
            return;
        };
        let then_bound = definitely_bound(&then_branch.statements);
        let else_bound = match else_branch {
            Else::Block(block) => definitely_bound(&block.statements),
            Else::If(nested) => definitely_bound(std::slice::from_ref(nested.as_ref())),
        };
        let mut shared: Vec<String> = then_bound
            .intersection(&else_bound)
            .filter(|name| !self.locals.contains(*name))
            .cloned()
            .collect();
        shared.sort();
        if shared.is_empty() {
            return;
        }
        let types = assignment_types(&then_branch.statements);
        let else_types = match else_branch {
            Else::Block(block) => assignment_types(&block.statements),
            Else::If(nested) => assignment_types(std::slice::from_ref(nested.as_ref())),
        };
        for name in shared {
            // The branches agree on the type in every program Python accepts;
            // where they do not, the declared type has to be the one both
            // assign to, so it is `interface{}` and each branch's value still
            // lands in it unchanged.
            let go = match (types.get(&name), else_types.get(&name)) {
                (Some(left), Some(right)) if left == right => named_type(left),
                _ => None,
            };
            let go = go.unwrap_or_else(|| "interface{}".to_string());
            self.out.writeln(&format!("var {name} {go}"));
            self.locals.insert(name);
        }
    }

    fn emit_else(&mut self, else_branch: Option<&Else>) {
        match else_branch {
            None => {}
            Some(Else::Block(block)) => {
                self.out.writeln("} else {");
                self.out.indent();
                self.emit_block_body(block);
                self.out.dedent();
            }
            Some(Else::If(nested)) => {
                // An `elif` is an `if` in the else position. Emitting `} else {
                // if ...` is valid but not idiomatic; Go's own form is
                // `} else if ... {`, which this reproduces by writing the `if`
                // header without the opening brace the nested emit would add.
                if let Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } = nested.as_ref()
                {
                    let condition = self.emit_condition(condition);
                    self.out.writeln(&format!("}} else if {condition} {{"));
                    self.out.indent();
                    self.emit_block_body(then_branch);
                    self.out.dedent();
                    self.emit_else(else_branch.as_deref());
                } else {
                    self.out.writeln("} else {");
                    self.out.indent();
                    self.emit_stmt(nested);
                    self.out.dedent();
                }
            }
        }
    }

    fn emit_for_in(
        &mut self,
        pattern: &Pattern,
        iterable: &Expr,
        body: &Block,
        else_body: Option<&Block>,
        _span: Span,
    ) {
        let completed = else_body.map(|_| self.fresh_name("completed"));
        // `for x in range(...)` is the one Python loop that maps onto a
        // counting `for` without materialising a sequence.
        if let Some(range) = range_arguments(iterable) {
            let index = match pattern.single_binding() {
                Some(name) => name.to_string(),
                None => "_".to_string(),
            };
            if index != "_" {
                self.locals.insert(index.clone());
            }
            let header = range.header(&index, self);
            self.emit_loop(&header, body, else_body, &completed, &[]);
            return;
        }

        // A sequence pattern keeps its names in subpatterns, so they have to be
        // collected before the pattern can be read as a list of bindings.
        let mut pattern_names = Vec::new();
        pattern.collect_names(&mut pattern_names);
        let pairs = matches!(pattern.kind, PatternKind::Sequence) && pattern_names.len() == 2;
        let dynamic = self.go_type_of(iterable).is_none();
        let iterable_helper = dynamic.then(|| self.use_helper("gsetIter"));
        let mut iterable = self.emit_expr(iterable);
        if let Some(helper) = iterable_helper {
            iterable = format!("{helper}({iterable})");
        }
        let mut prologue = Vec::new();
        let header = if pairs {
            // `for k, v in d.items()` on a mapping the source never typed: Go
            // cannot range over a map as pairs, so the helper produces them.
            let helper = self.use_helper("gsetPairs");
            let (pair, bindings) = self.pair_bindings("pair", &pattern_names);
            prologue = bindings;
            format!("_, {pair} := range {helper}({iterable})")
        } else {
            match pattern.single_binding() {
                Some(name) => {
                    let name = name.to_string();
                    self.locals.insert(name.clone());
                    format!("_, {name} := range {iterable}")
                }
                None if matches!(pattern.kind, PatternKind::Ignore) => format!("range {iterable}"),
                None => {
                    unsupported(
                        self.diagnostics,
                        pattern.span,
                        "a destructuring loop binding",
                    );
                    format!("range {iterable}")
                }
            }
        };
        self.emit_loop(&header, body, else_body, &completed, &prologue);
    }

    /// Emits `for <header> { ... }` plus any `else` clause.
    ///
    /// The `else` becomes an `if` on the flag the loop's `break`s reset, which is
    /// the only place Python's rule can live in Go.
    fn emit_loop(
        &mut self,
        header: &str,
        body: &Block,
        else_body: Option<&Block>,
        completed: &Option<String>,
        prologue: &[String],
    ) {
        if let Some(flag) = completed {
            self.out.writeln(&format!("{flag} := true"));
            // Two loops in one function must not both declare `completed`, and
            // `fresh_name` only looks at names already in scope.
            self.locals.insert(flag.clone());
            self.loop_else_flags.push(flag.clone());
        }
        self.out.writeln(&format!("for {header} {{"));
        self.out.indent();
        for line in prologue {
            self.out.writeln(line);
        }
        self.emit_block_body(body);
        self.out.dedent();
        self.out.writeln("}");
        if let (Some(flag), Some(else_body)) = (completed, else_body) {
            self.loop_else_flags.pop();
            self.out.writeln(&format!("if {flag} {{"));
            self.out.indent();
            self.emit_block_body(else_body);
            self.out.dedent();
            self.out.writeln("}");
        }
    }

    /// Emits `while`, which is a `for` with no header in Go.
    fn emit_while(
        &mut self,
        condition: &Expr,
        body: &Block,
        else_body: Option<&Block>,
        _span: Span,
    ) {
        let completed = else_body.map(|_| self.fresh_name("completed"));
        let header = self.emit_condition(condition);
        self.emit_loop(&header, body, else_body, &completed, &[]);
    }

    fn emit_local_item(&mut self, item: &Item) {
        match item {
            Item::Function(function) => {
                // A nested `def` is a Go closure bound to a local name.
                let params = self.emit_params(function);
                let returns_value = function_returns_value(function);
                let ret = function_return_type(function, returns_value);
                let header = match ret {
                    Some(ret) => {
                        format!("{} := func{params} {ret} {{", function.name)
                    }
                    None => format!("{} := func{params} {{", function.name),
                };
                self.locals.insert(function.name.to_string());
                let outer_locals = std::mem::take(&mut self.locals);
                let outer_returns = self.returns_value;
                self.returns_value = returns_value;
                self.out.writeln(&header);
                self.out.indent();
                self.bind_params(function);
                self.emit_block_body(&function.body);
                self.out.dedent();
                self.out.writeln("}");
                self.locals = outer_locals;
                self.returns_value = outer_returns;
            }
            other => unsupported(
                self.diagnostics,
                other.span(),
                "a nested declaration that is not a function",
            ),
        }
    }

    // --------------------------------------------------------- expressions

    /// Emits a literal, checking that the target can represent it.
    ///
    /// Go's `int` is 64 bits and Python integers are not, so an out-of-range
    /// literal has no translation. Emitting it anyway produces a file that does
    /// not compile, which is worse than reporting: the program looks supported
    /// until something else tries to run it.
    /// A value that stands in for something this backend reported.
    ///
    /// `nil` is an expression, not a statement: a reported call in statement
    /// position would emit a line that does not compile, which is the one
    /// outcome a diagnostic is supposed to avoid. Discarding it keeps the file
    /// buildable while the diagnostic says what was lost.
    /// A placeholder typed to what the surrounding program expects.
    ///
    /// A bare `nil` is only valid where the type is an interface, a pointer or
    /// a collection. Where the source's own type is concrete the zero value
    /// stands in, so a reported construct does not also break the file.
    fn unusable_value_of(&self, ty: Option<&Type>, rendered: String) -> String {
        if self.in_statement {
            // `_ = nil` does not compile either: untyped `nil` has no type to
            // assign. A discarded zero is as good as any placeholder, and the
            // value it stands for is being thrown away either way.
            return format!("_ = 0 /* {rendered} */");
        }
        let zero = ty
            .and_then(named_type)
            .map(|go_type_name| zero_value(&go_type_name))
            .unwrap_or("nil");
        format!("{zero} /* {rendered} */")
    }

    fn emit_literal(&mut self, span: Span, literal: &Literal) -> String {
        if let Literal::Int(text) = literal
            && let Some(digits) = plain_decimal(text)
            && digits.parse::<i64>().is_err()
        {
            unsupported(
                self.diagnostics,
                span,
                format!(
                    "the integer literal `{text}`, which needs more than 64 bits; the target's \
                     widest integer cannot hold it"
                ),
            );
            return "0".to_string();
        }
        go_literal(literal)
    }

    fn emit_pattern(&mut self, pattern: &Pattern) -> String {
        match &pattern.kind {
            PatternKind::Bind => pattern_name(pattern),
            PatternKind::Ignore => "_".to_string(),
            PatternKind::Location(expr) => self.emit_expr(expr),
            _ => {
                unsupported(self.diagnostics, pattern.span, "this assignment target");
                "_".to_string()
            }
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal(literal) => self.emit_literal(expr.span, literal),
            ExprKind::Path(path) => {
                // An import this backend dropped binds nothing, so a use of it
                // is a name Go has never heard of. Reporting at the use is the
                // only place the program is actually wrong; the import itself
                // was already warned about.
                if let Some(root) = root_name(expr)
                    && self.dropped_imports.contains(root)
                {
                    unsupported(self.diagnostics, expr.span, "a use of a dropped import");
                    let ty = expr.ty.clone();
                    let rendered = path
                        .segments
                        .iter()
                        .map(|segment| segment.to_string())
                        .collect::<Vec<_>>()
                        .join(".");
                    return self.unusable_value_of(Some(&ty), rendered);
                }
                path.segments
                    .iter()
                    .map(|segment| segment.to_string())
                    .collect::<Vec<_>>()
                    .join(".")
            }
            ExprKind::Unary { op, operand } => {
                let operand = self.emit_expr(operand);
                match op {
                    UnaryOp::Neg => format!("-{operand}"),
                    UnaryOp::Not => {
                        if matches!(expr.ty, Type::Bool) {
                            format!("!{operand}")
                        } else {
                            let helper = self.use_helper("gsetTruthy");
                            format!("!{helper}({operand})")
                        }
                    }
                    UnaryOp::BitNot => format!("^{operand}"),
                    _ => {
                        unsupported(self.diagnostics, expr.span, "this unary operator");
                        operand
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => self.emit_binary(expr.span, *op, lhs, rhs),
            ExprKind::Compare { op, lhs, rhs } => self.emit_compare(expr.span, *op, lhs, rhs),
            ExprKind::Logical { op, lhs, rhs } => {
                // `and` and `or` yield an operand, not a bool. Go's `&&` and `||`
                // yield a bool, which is only the same value when both operands
                // are bools; otherwise the operand itself has to survive.
                let boolean = matches!(lhs.ty, Type::Bool) && matches!(rhs.ty, Type::Bool);
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                if boolean {
                    let symbol = match op {
                        LogicalOp::And => "&&",
                        LogicalOp::Or => "||",
                    };
                    return format!("({lhs} {symbol} {rhs})");
                }
                let helper = match op {
                    LogicalOp::And => self.use_helper("gsetAnd"),
                    LogicalOp::Or => self.use_helper("gsetOr"),
                };
                format!("{helper}({lhs}, {rhs})")
            }
            ExprKind::Call {
                callee,
                args,
                named_args,
            } => self.emit_call(callee, args, named_args, expr.span, &expr.ty),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
                named_args,
            } => self.emit_method_call(receiver, method, args, named_args, expr.span, &expr.ty),
            ExprKind::Index { target, index } => {
                let target_text = self.emit_expr(target);
                if self.go_type_of(target).is_none() {
                    // An `interface{}` cannot be indexed in Go at all, and the
                    // index may be negative, which `reflect` resolves itself.
                    let helper = self.use_helper("gsetIndex");
                    let index_text = self.emit_expr(index);
                    return format!("{helper}({target_text}, {index_text})");
                }
                let index_text = self.emit_negative_index(expr.span, target, &target_text, index);
                format!("{target_text}[{index_text}]")
            }
            ExprKind::Slice {
                target,
                start,
                end,
                inclusive,
            } => self.emit_slice(
                expr.span,
                target,
                start.as_deref(),
                end.as_deref(),
                *inclusive,
            ),
            ExprKind::Field { target, field } => {
                let target = self.emit_expr(target);
                format!("{target}.{field}")
            }
            ExprKind::List { elements } | ExprKind::Tuple { elements } => {
                self.emit_sequence_literal(elements)
            }
            ExprKind::Set { elements } => self.emit_set_literal(elements),
            ExprKind::Map { entries } => self.emit_map_literal(entries),
            ExprKind::Lambda { params, body } => self.emit_lambda(params, body, expr.span),
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => self.emit_conditional(expr.span, condition, then_branch, else_branch),
            ExprKind::Comprehension { .. } => self.emit_comprehension(expr),
            ExprKind::Let {
                pattern,
                value,
                body,
            } => self.emit_let(expr.span, pattern, value, body),
            ExprKind::Format {
                segments,
                arguments,
                format,
            } => self.emit_format(segments, arguments, format.as_deref(), expr.span),
            ExprKind::Error(_) => "nil".to_string(),
            other => {
                unsupported(self.diagnostics, expr.span, describe_expr(other));
                "nil".to_string()
            }
        }
    }

    fn emit_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        named_args: &[NamedArg],
        span: Span,
        ty: &Type,
    ) -> String {
        if !named_args.is_empty() {
            unsupported(self.diagnostics, span, "keyword arguments");
        }
        let rendered: Vec<String> = args.iter().map(|arg| self.emit_expr(arg)).collect();
        if let ExprKind::Path(path) = &callee.kind
            && path.is_bare()
        {
            let name = path.segments[0].as_ref();
            match name {
                "print" => {
                    self.imports.insert("fmt");
                    return format!("fmt.Println({})", rendered.join(", "));
                }
                "len" => {
                    // `len` of an `interface{}` has no Go built-in form.
                    if args
                        .first()
                        .is_some_and(|argument| self.go_type_of(argument).is_none())
                    {
                        let helper = self.use_helper("gsetLen");
                        return format!("{helper}({})", rendered.join(", "));
                    }
                    return format!("len({})", rendered.join(", "));
                }
                // Python's `sorted` is stable and returns a new sequence, and a
                // set of pairs is not something Go's `sort` can see through an
                // `interface{}` without being told to.
                "sorted" => {
                    let helper = self.use_helper("gsetSort");
                    return format!("{helper}({})", rendered.join(", "));
                }
                "str" => {
                    self.imports.insert("fmt");
                    return format!("fmt.Sprint({})", rendered.join(", "));
                }
                // A conversion whose operand type the source never stated is a
                // runtime question: Python accepts an int where Go's
                // `float64(...)` requires a float, and both `int` and `float`
                // parse a string in Python.
                "int" => {
                    if args
                        .first()
                        .is_some_and(|argument| self.go_type_of(argument).is_none())
                    {
                        let helper = self.use_helper("gsetInt");
                        return format!("{helper}({})", rendered.join(", "));
                    }
                    return format!("int({})", rendered.join(", "));
                }
                "float" => {
                    if args
                        .first()
                        .is_some_and(|argument| self.go_type_of(argument).is_none())
                    {
                        let helper = self.use_helper("gsetFloat");
                        return format!("{helper}({})", rendered.join(", "));
                    }
                    return format!("float64({})", rendered.join(", "));
                }
                "bool" => {
                    let helper = self.use_helper("gsetTruthy");
                    return format!("{helper}({})", rendered.join(", "));
                }
                "range" => {
                    // `range` only has meaning as a loop header, which
                    // `emit_for_in` handles. Reaching here means it was used as
                    // a value.
                    unsupported(self.diagnostics, span, "`range` outside a loop header");
                    return "nil".to_string();
                }
                "sum" => {
                    // Python's `sum` starts from an integer zero, so an empty
                    // collection sums to `0` and a value that is not a number
                    // is a `TypeError`. A typed collection keeps its type; a
                    // dynamic one is classified at runtime.
                    let typed = args.first().is_some_and(|argument| {
                        let resolved = self.static_type_of(argument);
                        sequence_element_type_of(&resolved)
                            .is_some_and(|element| numeric_go_type(&element).is_some())
                    });
                    let helper = if typed {
                        self.use_helper("gsetSum")
                    } else {
                        self.use_helper("gsetSumDynamic")
                    };
                    return format!("{helper}({})", rendered.join(", "));
                }
                _ => {}
            }
            // A name the module never binds cannot be called, and emitting the
            // call anyway would leave a file that does not compile. Reporting
            // is the honest outcome for both the builtins this backend cannot
            // express and the names the source itself never defined.
            if self.dropped_imports.contains(name) {
                unsupported(self.diagnostics, span, "a use of a dropped import");
                return self.unusable_value_of(Some(ty), format!("{name}()"));
            }
            if !self.declared.contains(name) {
                if python_builtins().contains(&name) {
                    unsupported(self.diagnostics, span, "this builtin");
                } else {
                    unsupported(
                        self.diagnostics,
                        span,
                        "a call to a name the module never binds",
                    );
                }
                let call = format!("{name}({})", rendered.join(", "));
                return self.unusable_value_of(Some(ty), call);
            }
        }
        let callee = self.emit_expr(callee);
        format!("{callee}({})", rendered.join(", "))
    }

    /// Emits a method call.
    ///
    /// Python's methods come from types that do not exist in Go, so most of them
    /// are spelled with a standard library call or a helper rather than with a
    /// method on the receiver. A method that mutates and returns nothing —
    /// `append`, `extend`, `insert`, `sort`, `add` — becomes an assignment, and
    /// only in statement position: in expression position there is nowhere to
    /// put the new value, so it is reported instead of being dropped.
    fn emit_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        named_args: &[NamedArg],
        span: Span,
        ty: &Type,
    ) -> String {
        if !named_args.is_empty() {
            unsupported(self.diagnostics, span, "keyword arguments");
        }
        // `os.path.join(...)` is a method call on a name the dropped import never
        // bound. Rendering the receiver would produce `nil.path`, so the call
        // is reported here instead of assembled from a placeholder.
        if let Some(root) = root_name(receiver)
            && self.dropped_imports.contains(root)
        {
            let arguments: Vec<String> = args.iter().map(|arg| self.emit_expr(arg)).collect();
            let rendered = format!(
                "{}.{method}({})",
                dotted_text(receiver),
                arguments.join(", ")
            );
            return self.unusable_value_of(Some(ty), rendered);
        }
        // Read before rendering: rendering borrows the emitter, and the
        // expansion of a mutating method needs the receiver's type to name the
        // slice or map it is assigning back to.
        let receiver_type = self.go_type_of(receiver);
        let mapping_like = is_mapping_like(receiver);
        let receiver = self.emit_expr(receiver);
        // Read before rendering: an argument count decides which helper a method
        // needs, and a rendered list cannot answer that.
        let arity = args.len();
        let rendered: Vec<String> = args.iter().map(|arg| self.emit_expr(arg)).collect();
        let args = rendered.join(", ");
        match method {
            // ------------------------------------------------------------ strings
            "upper" => self.call_std("strings", "ToUpper", &receiver),
            "lower" => self.call_std("strings", "ToLower", &receiver),
            "strip" => self.call_std("strings", "TrimSpace", &receiver),
            // Python's default cutset is every whitespace character; Go's
            // `TrimLeft` takes the set explicitly.
            "lstrip" => format!("strings.TrimLeft({receiver}, \" \\t\\n\\r\\v\\f\")"),
            "rstrip" => format!("strings.TrimRight({receiver}, \" \\t\\n\\r\\v\\f\")"),
            "split" if arity == 0 => self.call_std("strings", "Fields", &receiver),
            "split" => {
                self.imports.insert("strings");
                format!("strings.Split({receiver}, {args})")
            }
            "splitlines" => {
                let helper = self.use_helper("gsetLines");
                format!("{helper}({receiver})")
            }
            "join" => {
                let helper = self.use_helper("gsetJoin");
                format!("{helper}({receiver}, {args})")
            }
            "replace" => {
                self.imports.insert("strings");
                format!("strings.ReplaceAll({receiver}, {args})")
            }
            "startswith" => {
                self.imports.insert("strings");
                format!("strings.HasPrefix({receiver}, {args})")
            }
            "endswith" => {
                self.imports.insert("strings");
                format!("strings.HasSuffix({receiver}, {args})")
            }
            "find" => {
                self.imports.insert("strings");
                format!("strings.Index({receiver}, {args})")
            }
            "rfind" => {
                self.imports.insert("strings");
                format!("strings.LastIndex({receiver}, {args})")
            }
            // `str.count` counts substrings and `list.count` counts members, so
            // the receiver's type picks the two apart.
            "count" if receiver_type.as_deref().is_some_and(|ty| ty == "string") => {
                self.imports.insert("strings");
                format!("strings.Count({receiver}, {args})")
            }
            "count" => {
                let helper = self.use_helper("gsetCount");
                format!("{helper}({receiver}, {args})")
            }
            // ------------------------------------------------------------- lists
            "index" => {
                let helper = self.use_helper("gsetIndexOf");
                format!("{helper}({receiver}, {args})")
            }
            // -------------------------------------------------------------- dicts
            "get" => {
                let helper = if arity > 1 {
                    self.use_helper("gsetGetOr")
                } else {
                    self.use_helper("gsetGet")
                };
                format!("{helper}({receiver}, {args})")
            }
            "keys" => {
                let helper = self.use_helper("gsetKeys");
                format!("{helper}({receiver})")
            }
            "update" => {
                let helper = self.use_helper("gsetUpdate");
                let expansion = format!("{helper}({receiver}, {args})");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, true)
            }
            "values" => {
                let helper = self.use_helper("gsetValues");
                format!("{helper}({receiver})")
            }
            "items" => {
                let helper = self.use_helper("gsetItems");
                format!("{helper}({receiver})")
            }
            // ------------------------------------------------- mutating methods
            "append" if arity == 0 => {
                unsupported(self.diagnostics, span, "`.append` with no value to add");
                format!("append({receiver})")
            }
            "append" => {
                let expansion = format!("append({receiver}, {args})");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, false)
            }
            "extend" => {
                let helper = self.use_helper("gsetExtend");
                let expansion = format!("{helper}({receiver}, {args})");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, true)
            }
            "insert" => {
                let helper = self.use_helper("gsetInsert");
                let expansion = format!("{helper}({receiver}, {args})");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, true)
            }
            "reverse" => {
                let helper = self.use_helper("gsetReverse");
                let expansion = format!("{helper}({receiver})");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, true)
            }
            "clear" => {
                // Truncating a slice in place keeps the same backing array, which
                // is what Python's `clear` does to a list's identity.
                let expansion = format!("{receiver}[:0]");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, false)
            }
            "pop" => self.emit_pop(span, &receiver, &receiver_type),
            "sort" => {
                let helper = self.use_helper("gsetSort");
                let statement = format!("{helper}({receiver})");
                self.emit_statement_only(span, method, statement)
            }
            // A set's `add` is a map write, which is a statement and returns
            // nothing, so it cannot be an expression either.
            "add" => {
                let statement = format!("{receiver}[{args}] = struct{{}}{{}}");
                self.emit_statement_only(span, method, statement)
            }
            "discard" | "remove" if mapping_like => {
                let statement = format!("delete({receiver}, {args})");
                self.emit_statement_only(span, method, statement)
            }
            "remove" => {
                let helper = self.use_helper("gsetRemove");
                let expansion = format!("{helper}({receiver}, {args})");
                self.emit_mutation(span, method, &receiver, &receiver_type, expansion, true)
            }
            _ => {
                unsupported(self.diagnostics, span, describe_method(method));
                format!("{receiver}.{method}({args})")
            }
        }
    }

    /// Emits `xs.pop()`.
    ///
    /// Python returns the value and shortens the list; Go needs both, so the
    /// expansion is a two-value assignment. `:=` only works when the value's
    /// name is new, so the declared case has to use `=`.
    fn emit_pop(&mut self, span: Span, receiver: &str, receiver_type: &Option<String>) -> String {
        if receiver_type.is_none() {
            unsupported(
                self.diagnostics,
                span,
                "`.pop` on a value whose type this backend cannot name",
            );
            return "nil".to_string();
        }
        if !self.in_statement {
            unsupported(
                self.diagnostics,
                span,
                "`.pop` in expression position, which needs a statement to bind its result",
            );
            return "nil".to_string();
        }
        let helper = self.use_helper("gsetPop");
        let popped = self.fresh_name("popped");
        let rest = self.fresh_name("rest");
        // `:=` only declares names that are new, so a name already in scope
        // falls back to `var`, which redeclares. Both names come from one call:
        // Go has no way to spread one multi-value call over two statements.
        let declared = [
            self.locals.contains(&popped) || self.globals.contains(&popped),
            self.locals.contains(&rest) || self.globals.contains(&rest),
        ]
        .into_iter()
        .any(|declared| declared);
        self.locals.insert(popped.clone());
        self.locals.insert(rest.clone());
        let keyword = if declared { "var" } else { ":=" };
        if declared {
            self.out
                .writeln(&format!("var {popped}, {rest} = {helper}({receiver})"));
        } else {
            self.out
                .writeln(&format!("{popped}, {rest} {keyword} {helper}({receiver})"));
        }
        // The helper hands both back as `interface{}`s, so the collection goes
        // back into its own type: without this the rest of the function sees a
        // value it can no longer index, slice or take the length of.
        let ty = receiver_type.as_deref().unwrap_or("interface{}");
        self.out.writeln(&format!("{receiver} = {rest}.({ty})"));
        popped
    }

    /// Emits an index, shifting Python's negative one into Go's bounds-checked form.
    ///
    /// `xs[-1]` is `xs[len(xs)-1]`, and Go rejects a negative constant outright.
    /// The shift needs the length, so it repeats the target — sound only when
    /// evaluating it twice cannot differ.
    fn emit_negative_index(
        &mut self,
        span: Span,
        target: &Expr,
        target_text: &str,
        index: &Expr,
    ) -> String {
        let negative = match &index.kind {
            ExprKind::Literal(Literal::Int(text)) => match text.parse::<i64>() {
                Ok(value) if value < 0 => Some(value.unsigned_abs() as usize),
                _ => None,
            },
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match &operand.kind {
                ExprKind::Literal(Literal::Int(text)) => {
                    text.parse::<i64>().ok().map(|v| v as usize)
                }
                _ => None,
            },
            _ => None,
        };
        let Some(offset) = negative else {
            return self.emit_expr(index);
        };
        if !is_repeatable(target) {
            unsupported(
                self.diagnostics,
                span,
                "a negative index on a value that would have to be evaluated twice",
            );
        }
        format!("len({target_text}) - {offset}")
    }

    /// Emits `a if c else b`.
    ///
    /// Go has no ternary operator, so the only faithful expansion is an
    /// immediately invoked function literal: one that returns from the arm the
    /// condition selects. Evaluating both arms and picking afterwards would be
    /// wrong for any arm with a call in it.
    fn emit_conditional(
        &mut self,
        span: Span,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Expr,
    ) -> String {
        // A Go function literal needs one result type. Python's two arms agree
        // only if the source's types agree, so anything else is `interface{}`
        // rather than a guess at the wider of the two.
        let result = match (&then_branch.ty, &else_branch.ty) {
            (then, otherwise) if then == otherwise => {
                named_type(then).unwrap_or_else(|| "interface{}".to_string())
            }
            _ => "interface{}".to_string(),
        };
        let condition = self.emit_condition(condition);
        // The taken arm sits inside the `if`'s block, so anything it renders has
        // to be indented as if it were there.
        self.expression_depth += 1;
        let then_value = self.emit_expr(then_branch);
        self.expression_depth -= 1;
        let else_value = self.emit_expr(else_branch);
        let body = vec![
            format!("if {condition} {{"),
            format!("{}return {then_value}", indent(1)),
            "}".to_string(),
            format!("return {else_value}"),
        ];
        let tail = vec!["}()".to_string()];
        let _ = span;
        self.multiline(format!("func() {result} {{"), &body, &tail)
    }

    /// Emits a comprehension.
    ///
    /// The expansion is an immediately invoked function literal that accumulates
    /// into a variable and returns it, which keeps the construct in expression
    /// position where the source put it. A generator is *not* emitted this way:
    /// Python's is lazy, and an eagerly built slice is a different program.
    fn emit_comprehension(&mut self, expr: &Expr) -> String {
        let span = expr.span;
        let ExprKind::Comprehension {
            kind,
            element,
            key,
            value,
            clauses,
            condition,
        } = &expr.kind
        else {
            return "nil".to_string();
        };
        let (kind, element, key, value, clauses, condition) = (
            *kind,
            element,
            key.as_deref(),
            value.as_deref(),
            clauses,
            condition.as_deref(),
        );
        if kind == ComprehensionKind::Generator {
            unsupported(
                self.diagnostics,
                span,
                "a generator, whose laziness is observable and which a built slice cannot express",
            );
            return "nil".to_string();
        }
        // A clause that destructures binds its names out of a pair, which is an
        // `interface{}` at that point, so nothing downstream can claim a static
        // element type without inventing one the program never said.
        let destructures = clauses
            .iter()
            .any(|clause| matches!(clause.pattern.kind, PatternKind::Sequence));
        let (result, zero) = match kind {
            ComprehensionKind::List => {
                let element = if destructures {
                    "interface{}".to_string()
                } else {
                    element_go_type(element)
                };
                (format!("[]{element}"), format!("var out []{element}"))
            }
            ComprehensionKind::Set => {
                let element = if destructures {
                    "interface{}".to_string()
                } else {
                    element_go_type(element)
                };
                (
                    format!("map[{element}]struct{{}}"),
                    // A nil map cannot be written to, so the accumulator starts
                    // as an empty one rather than a nil one.
                    format!("out := map[{element}]struct{{}}{{}}"),
                )
            }
            ComprehensionKind::Map => {
                let key_type = if destructures {
                    "interface{}".to_string()
                } else {
                    key.map_or_else(|| "string".to_string(), element_go_type)
                };
                let value_type = if destructures {
                    "interface{}".to_string()
                } else {
                    value.map_or_else(|| "interface{}".to_string(), element_go_type)
                };
                (
                    format!("map[{key_type}]{value_type}"),
                    // A nil map cannot be written to, and a map built from
                    // nothing has to start as one.
                    format!("out := map[{key_type}]{value_type}{{}}"),
                )
            }
            ComprehensionKind::Generator => unreachable!("reported above"),
        };

        // Lines are indented by their nesting depth here and the writer adds the
        // statement's own level, because a multi-line expansion inside an
        // expression has to line up with where the expression lands.
        let mut body = vec![zero];
        let mut depth = 0usize;
        for clause in clauses {
            let mut clause_names = Vec::new();
            clause.pattern.collect_names(&mut clause_names);
            let destructuring =
                matches!(clause.pattern.kind, PatternKind::Sequence) && clause_names.len() == 2;
            let single = clause.pattern.single_binding().map(ToString::to_string);
            if !destructuring && single.is_none() {
                unsupported(
                    self.diagnostics,
                    clause.span,
                    "a comprehension clause that binds more than one name",
                );
                return "nil".to_string();
            }
            let helper = destructuring.then(|| self.use_helper("gsetPairs"));
            // A clause over a value with no Go type of its own cannot be ranged
            // over, so the elements are materialised first.
            let dynamic = self.go_type_of(&clause.iterable).is_none();
            let iterable_helper = dynamic.then(|| self.use_helper("gsetIter"));
            let mut iterable = self.emit_expr(&clause.iterable);
            if let Some(helper) = iterable_helper {
                iterable = format!("{helper}({iterable})");
            }
            let (header, bindings) = match (&helper, single) {
                (Some(helper), _) => {
                    let (pair, bindings) = self.pair_bindings("pair", &clause_names);
                    (format!("_, {pair} := range {helper}({iterable})"), bindings)
                }
                (None, Some(name)) => {
                    self.locals.insert(name.clone());
                    (format!("_, {name} := range {iterable}"), Vec::new())
                }
                (None, None) => (format!("range {iterable}"), Vec::new()),
            };
            body.push(format!("{}for {header} {{", indent(depth)));
            depth += 1;
            for binding in bindings {
                body.push(format!("{}{binding}", indent(depth)));
            }
        }
        let produce = match kind {
            ComprehensionKind::List => {
                // An argument of a call is indented two levels in by `gofmt`,
                // because the call's own continuation lines already are.
                self.expression_depth += 2;
                let element = self.emit_expr(element);
                self.expression_depth -= 2;
                format!("out = append(out, {element})")
            }
            ComprehensionKind::Set => {
                self.expression_depth += 1;
                let element = self.emit_expr(element);
                self.expression_depth -= 1;
                format!("out[{element}] = struct{{}}{{}}")
            }
            ComprehensionKind::Map => {
                let Some(key) = key else {
                    unsupported(
                        self.diagnostics,
                        span,
                        "a mapping comprehension with no key",
                    );
                    return "nil".to_string();
                };
                let Some(value) = value else {
                    unsupported(
                        self.diagnostics,
                        span,
                        "a mapping comprehension with no value",
                    );
                    return "nil".to_string();
                };
                self.expression_depth += 2;
                let key = self.emit_expr(key);
                let value = self.emit_expr(value);
                self.expression_depth -= 2;
                format!("out[{key}] = {value}")
            }
            ComprehensionKind::Generator => unreachable!("reported above"),
        };
        if let Some(condition) = condition {
            let condition = self.emit_condition(condition);
            body.push(format!("{}if {condition} {{", indent(depth)));
            depth += 1;
        }
        body.push(format!("{}{produce}", indent(depth)));
        while depth > 0 {
            depth -= 1;
            body.push(format!("{}}}", indent(depth)));
        }
        body.push("return out".to_string());
        let tail = vec!["}()".to_string()];
        self.multiline(format!("func() {result} {{"), &body, &tail)
    }

    /// Emits a scoped binding, such as Python's walrus.
    ///
    /// The binding is live for exactly one expression, which is also what the
    /// function literal's scope gives: a `let` in an enclosing scope would
    /// export the name to code that cannot see it.
    fn emit_let(&mut self, span: Span, pattern: &Pattern, value: &Expr, body: &Expr) -> String {
        let Some(name) = pattern.single_binding() else {
            unsupported(
                self.diagnostics,
                span,
                "a scoped binding of more than one name",
            );
            return "nil".to_string();
        };
        let result = concrete_type(body).unwrap_or_else(|| "interface{}".to_string());
        let declared = self.locals.contains(name.as_ref()) || self.globals.contains(name.as_ref());
        let operator = if declared { "=" } else { ":=" };
        // Both lines are statements in the literal's own body, so a nested
        // expansion inside either of them needs no extra depth.
        let value = self.emit_expr(value);
        let body = self.emit_expr(body);
        let lines = vec![
            format!("{name} {operator} {value}"),
            format!("return {body}"),
        ];
        let tail = vec!["}()".to_string()];
        self.multiline(format!("func() {result} {{"), &lines, &tail)
    }

    /// Joins a multi-line expansion into an expression.
    ///
    /// `gofmt` rewrites indentation but not line breaks, so the lines after the
    /// first have to carry the indentation `gofmt` would give them: `body` sits
    /// one level in from the line the expression starts on, and `tail` — the
    /// closing `}()`, which ends the literal — sits at that line's own level.
    /// The writer's level is that level, because an expression is always
    /// rendered for the statement it is written into, never for a line that
    /// already exists.
    fn multiline(&self, header: String, body: &[String], tail: &[String]) -> String {
        let level = self.out.level() + self.expression_depth;
        let mut text = header;
        for line in body {
            text.push('\n');
            for _ in 0..=level {
                text.push('\t');
            }
            text.push_str(line);
        }
        for line in tail {
            text.push('\n');
            for _ in 0..level {
                text.push('\t');
            }
            text.push_str(line);
        }
        text
    }

    /// Emits `strings.Name(receiver)`, recording the import.
    fn call_std(&mut self, package: &'static str, function: &str, receiver: &str) -> String {
        self.imports.insert(package);
        format!("{package}.{function}({receiver})")
    }

    /// Emits a mutating method's expansion as an assignment back to the receiver.
    ///
    /// Go's slices and maps are values, so the method's result has to be written
    /// back to the name it came from. That needs the receiver's concrete type:
    /// the helper returns `interface{}`, and assigning that to a `[]int` is a
    /// compile error, so the type assertion is spelled here.
    fn emit_mutation(
        &mut self,
        span: Span,
        method: &str,
        receiver: &str,
        receiver_type: &Option<String>,
        expansion: String,
        from_interface: bool,
    ) -> String {
        if !self.in_statement {
            unsupported(
                self.diagnostics,
                span,
                format!(
                    "`.{method}`, which mutates the receiver and returns nothing, used where a \
                     value is expected"
                ),
            );
            return expansion;
        }
        let Some(ty) = receiver_type else {
            unsupported(
                self.diagnostics,
                span,
                format!("`.{method}` on a value whose type this backend cannot name"),
            );
            return expansion;
        };
        // A reflection helper hands back an `interface{}`, so the value goes
        // back into the collection's own type. An expansion Go already typed
        // needs no assertion, and an unparenthesised one would be read as a
        // type assertion on the wrong value.
        let assigned = if from_interface {
            format!("({expansion}).({ty})")
        } else {
            expansion
        };
        self.out.writeln(&format!("{receiver} = {assigned}"));
        "".to_string()
    }

    /// Emits a method whose whole effect is a statement.
    fn emit_statement_only(&mut self, span: Span, method: &str, statement: String) -> String {
        if !self.in_statement {
            unsupported(
                self.diagnostics,
                span,
                format!("`.{method}` in expression position, which returns nothing in the source"),
            );
            return "nil".to_string();
        }
        self.out.writeln(&statement);
        "".to_string()
    }

    /// A name that is not yet in scope, for a binding the expansion introduces.
    fn fresh_name(&mut self, stem: &str) -> String {
        let mut candidate = stem.to_string();
        let mut counter = 1;
        while self.locals.contains(&candidate) || self.globals.contains(&candidate) {
            candidate = format!("{stem}{counter}");
            counter += 1;
        }
        candidate
    }

    fn emit_binary(&mut self, span: Span, op: BinaryOp, lhs: &Expr, rhs: &Expr) -> String {
        // `//` and `**` have no Go operator, and Go's own `/` disagrees with
        // Python's `//` for negative operands, so both go through a helper that
        // is emitted only when a program uses them.
        match op {
            BinaryOp::FloorDiv => {
                // Go's integer division truncates and a generic helper cannot
                // take floats, so a float operand needs its own.
                let floating = matches!(&lhs.ty, Type::Float) || matches!(&rhs.ty, Type::Float);
                let helper = if floating {
                    self.use_helper("gsetFloatFloorDiv")
                } else {
                    self.use_helper("gsetFloorDiv")
                };
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                return format!("{helper}({lhs}, {rhs})");
            }
            BinaryOp::Pow => return self.emit_power(span, lhs, rhs),
            BinaryOp::Div => return self.emit_true_division(span, lhs, rhs),
            _ => {}
        }
        if let Some((promote_left, promote_right)) = numeric_promotion(lhs, rhs) {
            let lhs = self.emit_operand(lhs, promote_left);
            let rhs = self.emit_operand(rhs, promote_right);
            if let Some(symbol) = binary_symbol(op) {
                return format!("({lhs} {symbol} {rhs})");
            }
        }
        // An operand the source never typed is an `interface{}` in Go, and Go
        // has no operator over `interface{}`: the compiler rejects the
        // expression outright. Python's arithmetic rules — which kinds of
        // operand combine, and what the result's type is — are a runtime
        // question here, so it goes to a helper that asks it.
        if self.go_type_of(lhs).is_none() || self.go_type_of(rhs).is_none() {
            if let Some(helper) = dynamic_arithmetic_helper(op) {
                let helper = self.use_helper(helper);
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                return format!("{helper}({lhs}, {rhs})");
            }
            if let Some(symbol) = binary_symbol(op) {
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                unsupported(
                    self.diagnostics,
                    span,
                    "this operator on a value whose type the source never stated",
                );
                return format!("({lhs} /* unsupported */ {symbol} {rhs})");
            }
        }
        let lhs = self.emit_expr(lhs);
        let rhs = self.emit_expr(rhs);
        match binary_symbol(op) {
            Some(symbol) => format!("({lhs} {symbol} {rhs})"),
            None => {
                unsupported(self.diagnostics, span, "this binary operator");
                format!("({lhs} /* unsupported */ {rhs})")
            }
        }
    }

    /// Emits `/`, which is Python's true division and not Go's integer one.
    ///
    /// `3 / 2` is `1.5` in Python and `1` in Go, so an integral operand is
    /// promoted rather than emitted as-is: silently truncating is the one
    /// answer a transpiler must never give.
    fn emit_true_division(&mut self, span: Span, lhs: &Expr, rhs: &Expr) -> String {
        if numeric_rank(&lhs.ty).is_some() && numeric_rank(&rhs.ty).is_some() {
            let promoted_left = numeric_rank(&lhs.ty) == Some(1);
            let promoted_right = numeric_rank(&rhs.ty) == Some(1);
            let left = self.emit_operand(lhs, promoted_left);
            let right = self.emit_operand(rhs, promoted_right);
            return format!("({left} / {right})");
        }
        if self.go_type_of(lhs).is_some() && self.go_type_of(rhs).is_some() {
            unsupported(
                self.diagnostics,
                span,
                "a division of two non-numeric values",
            );
            let left = self.emit_expr(lhs);
            let right = self.emit_expr(rhs);
            return format!("({left} / {right})");
        }
        let helper = self.use_helper("gsetDivide");
        let left = self.emit_expr(lhs);
        let right = self.emit_expr(rhs);
        format!("{helper}({left}, {right})")
    }

    /// Emits `a ** b`.
    ///
    /// Python's `**` returns an int when both operands are ints and a float
    /// otherwise, and Go has neither an integer power operator nor a uniform
    /// one, so the operand types decide which is emitted. An untyped operand
    /// cannot be resolved here: `gset-semantic` runs before this and would have
    /// inferred a type, so reaching `Unknown` means the source really did
    /// compute a value of unknown type.
    fn emit_power(&mut self, span: Span, lhs: &Expr, rhs: &Expr) -> String {
        let known_int = matches!(&lhs.ty, Type::Int(_)) && matches!(&rhs.ty, Type::Int(_));
        let known_float = matches!(&lhs.ty, Type::Float) || matches!(&rhs.ty, Type::Float);
        let unknown = matches!(&lhs.ty, Type::Unknown(_)) || matches!(&rhs.ty, Type::Unknown(_));
        if known_float {
            self.imports.insert("math");
            let lhs = self.emit_expr(lhs);
            let rhs = self.emit_expr(rhs);
            return format!("math.Pow({lhs}, {rhs})");
        }
        // An exponent that is not a non-negative literal may be negative, and
        // Python's answer to a negative exponent is a float while Go's loop
        // would answer 1, so only a known non-negative one stays integral.
        let maybe_negative = !matches!(literal_bound(rhs), Some(bound) if bound >= 0);
        if (known_int || unknown) && !maybe_negative {
            let helper = self.use_helper("gsetIntPow");
            let lhs = self.emit_expr(lhs);
            let rhs = self.emit_expr(rhs);
            return format!("{helper}({lhs}, {rhs})");
        }
        if known_int || known_float || unknown {
            let helper = self.use_helper("gsetFloatPow");
            let lhs = self.emit_expr(lhs);
            let rhs = self.emit_expr(rhs);
            return format!("{helper}({lhs}, {rhs})");
        }
        unsupported(
            self.diagnostics,
            span,
            "a `**` whose operand types are neither int nor float",
        );
        "0".to_string()
    }

    fn emit_compare(&mut self, span: Span, op: ComparisonOp, lhs: &Expr, rhs: &Expr) -> String {
        // `in` and `not in` are containment, not comparison, and Go has no
        // operator for either at any element type.
        match op {
            ComparisonOp::In | ComparisonOp::NotIn => {
                // `item in collection` reads the collection second, and the
                // helper takes the collection first, so the operands swap here.
                let helper = self.use_helper("gsetContains");
                let item = self.emit_expr(lhs);
                let collection = self.emit_expr(rhs);
                let call = format!("{helper}({collection}, {item})");
                return if op == ComparisonOp::In {
                    call
                } else {
                    format!("!{call}")
                };
            }
            _ => {}
        }
        // `is` asks whether two names hold the same object, which is a question
        // about identity rather than value: `x is None` is the only spelling
        // Go can answer, and the rest stays a reported gap rather than a
        // `==` that would compare values and quietly disagree.
        if matches!(op, ComparisonOp::Is | ComparisonOp::IsNot)
            && !matches!(rhs.kind, ExprKind::Literal(Literal::Null))
        {
            unsupported(
                self.diagnostics,
                span,
                "`is` between two values that are not None",
            );
            let symbol = if op == ComparisonOp::Is { "==" } else { "!=" };
            let lhs = self.emit_expr(lhs);
            let rhs = self.emit_expr(rhs);
            return format!("({lhs} {symbol} {rhs})");
        }
        if let Some(equality) = self.emit_equality(op, lhs, rhs) {
            return equality;
        }
        if let Some(ordering) = self.emit_ordering(op, lhs, rhs) {
            return ordering;
        }
        let lhs = self.emit_expr(lhs);
        let rhs = self.emit_expr(rhs);
        match comparison_symbol(op) {
            Some(symbol) => format!("({lhs} {symbol} {rhs})"),
            None => {
                unsupported(self.diagnostics, span, "this comparison");
                format!("({lhs} /* unsupported */ {rhs})")
            }
        }
    }

    /// Emits `==` and `!=` when Go's own operator does not answer Python's
    /// question.
    ///
    /// Go compares an interface holding a slice by panicking, and an `int`
    /// against a `float64` as unequal, where Python compares containers by
    /// value and 1 to 1.0 as equal. Both answers only exist at runtime, so a
    /// comparison with an operand the source never typed asks a helper.
    fn emit_equality(&mut self, op: ComparisonOp, lhs: &Expr, rhs: &Expr) -> Option<String> {
        if !matches!(
            op,
            ComparisonOp::Eq | ComparisonOp::Ne | ComparisonOp::Is | ComparisonOp::IsNot
        ) {
            return None;
        }
        if let Some((promote_left, promote_right)) = numeric_promotion(lhs, rhs) {
            let left = self.emit_operand(lhs, promote_left);
            let right = self.emit_operand(rhs, promote_right);
            let symbol = comparison_symbol(op)?;
            return Some(format!("({left} {symbol} {right})"));
        }
        if !needs_value_equality(lhs) && !needs_value_equality(rhs) {
            return None;
        }
        let helper = self.use_helper("gsetEqual");
        let left = self.emit_expr(lhs);
        let right = self.emit_expr(rhs);
        let call = format!("{helper}({left}, {right})");
        Some(if matches!(op, ComparisonOp::Eq | ComparisonOp::Is) {
            call
        } else {
            format!("!{call}")
        })
    }

    /// Emits `<`, `<=`, `>` and `>=` when the operands do not share a Go type.
    ///
    /// Python orders across its numeric tower and Go has no operator between
    /// `int` and `float64`, so either the promotion is written out or, when an
    /// operand's type is unknown, the ordering is decided at runtime.
    fn emit_ordering(&mut self, op: ComparisonOp, lhs: &Expr, rhs: &Expr) -> Option<String> {
        let symbol = comparison_symbol(op)?;
        if !matches!(
            op,
            ComparisonOp::Lt | ComparisonOp::Le | ComparisonOp::Gt | ComparisonOp::Ge
        ) {
            return None;
        }
        if let Some((promote_left, promote_right)) = numeric_promotion(lhs, rhs) {
            let left = self.emit_operand(lhs, promote_left);
            let right = self.emit_operand(rhs, promote_right);
            return Some(format!("({left} {symbol} {right})"));
        }
        if self.go_type_of(lhs).is_some() && self.go_type_of(rhs).is_some() {
            return None;
        }
        let helper = self.use_helper("gsetCompare");
        let left = self.emit_expr(lhs);
        let right = self.emit_expr(rhs);
        Some(format!("{helper}({left}, {right}) {symbol} 0"))
    }

    /// The type an expression has before inference is consulted.
    ///
    /// A parameter the source annotated is stated, not inferred, so it counts
    /// even when the use of it has no type of its own yet.
    fn static_type_of(&self, expr: &Expr) -> Type {
        if let ExprKind::Path(path) = &expr.kind
            && path.is_bare()
            && let Some(declared) = self.param_types.get(path.segments[0].as_ref())
        {
            return declared.clone();
        }
        expr.ty.clone()
    }

    /// Emits an operand, promoting it to `float64` when Python's numeric tower
    /// requires it and Go's types do not allow it.
    fn emit_operand(&mut self, expr: &Expr, promote: bool) -> String {
        let value = self.emit_expr(expr);
        if promote {
            format!("float64({value})")
        } else {
            value
        }
    }

    /// Binds the names of a destructuring pattern from one pair value.
    ///
    /// A Go `range` over pairs yields the pair whole, so the names have to be
    /// taken from it one index at a time. The pair itself is materialised into a
    /// name first, because an expression cannot be indexed twice in a `:=`.
    fn pair_bindings(
        &mut self,
        stem: &str,
        names: &[std::sync::Arc<str>],
    ) -> (String, Vec<String>) {
        let pair = self.fresh_name(stem);
        self.locals.insert(pair.clone());
        let bindings = names
            .iter()
            .enumerate()
            .flat_map(|(index, name)| {
                self.locals.insert(name.to_string());
                // Go refuses a declared-and-unused variable and Python does not,
                // and whether the body uses the name is not the emitter's to
                // decide, so the binding is always discarded explicitly.
                [format!("{name} := {pair}[{index}]"), format!("_ = {name}")]
            })
            .collect();
        (pair, bindings)
    }

    /// Emits a slice bound, which the runtime helper reads as a plain int.
    fn emit_bound(&mut self, bound: &Expr) -> String {
        self.emit_expr(bound)
    }

    fn emit_slice(
        &mut self,
        span: Span,
        target_expr: &Expr,
        start: Option<&Expr>,
        end: Option<&Expr>,
        inclusive: bool,
    ) -> String {
        // A slice of a value the source never typed has no Go operand form at
        // all — `interface{}` cannot be sliced — so the bounds and the length
        // are resolved at runtime instead.
        if self.go_type_of(target_expr).is_none() {
            let helper = self.use_helper("gsetSlice");
            let target = self.emit_expr(target_expr);
            let start = match start {
                Some(start) => self.emit_bound(start),
                None => "0".to_string(),
            };
            let end = match end {
                Some(end) if inclusive => match literal_bound(end) {
                    Some(bound) => (bound + 1).to_string(),
                    None => {
                        unsupported(
                            self.diagnostics,
                            span,
                            "a slice with a computed inclusive bound",
                        );
                        self.emit_expr(end)
                    }
                },
                Some(end) => self.emit_bound(end),
                // The helper treats a bound past the end as the length, so an
                // omitted one is just an impossible one.
                None => format!("len({target})"),
            };
            return format!("{helper}({target}, {start}, {end})");
        }
        // An omitted bound has to become a real Go operand, and `len(target)`
        // repeats `target`. Repeating it is only sound when evaluating it twice
        // cannot differ, so anything with a call or an index in it is reported.
        let repeatable = is_repeatable(target_expr);
        if !repeatable {
            unsupported(
                self.diagnostics,
                target_expr.span,
                "a slice of a value that would have to be evaluated twice",
            );
        }
        let target = self.emit_expr(target_expr);
        let start = match start {
            Some(start) => self.emit_negative_index(span, target_expr, &target, start),
            None => "0".to_string(),
        };
        let end = match end {
            None => format!("len({target})"),
            Some(end) if inclusive => {
                // Python's inclusive end is one past Go's exclusive one. Only a
                // literal bound can be shifted here; anything else needs a
                // temporary, which expression position cannot hold.
                if let ExprKind::Literal(Literal::Int(text)) = &end.kind
                    && let Ok(bound) = text.parse::<i64>()
                {
                    (bound + 1).to_string()
                } else {
                    unsupported(
                        self.diagnostics,
                        span,
                        "a slice with a computed inclusive bound",
                    );
                    self.emit_expr(end)
                }
            }
            Some(end) => self.emit_expr(end),
        };
        format!("{target}[{start}:{end}]")
    }

    fn emit_set_literal(&mut self, elements: &[Expr]) -> String {
        // A Go set is a map whose value carries no information.
        let element_type = sequence_element_type(elements);
        if elements.is_empty() {
            return format!("map[{element_type}]struct{{}}{{}}");
        }
        let rendered: Vec<String> = elements
            .iter()
            .map(|element| {
                let element = self.emit_expr(element);
                format!("{element}: {{}}")
            })
            .collect();
        format!("map[{element_type}]struct{{}}{{{}}}", rendered.join(", "))
    }

    fn emit_map_literal(&mut self, entries: &[NamedArg]) -> String {
        // The IR's mappings are string-keyed, which the frontend flags when a
        // source key is not a string, so the key here is the source's key.
        let mut value_type: Option<String> = None;
        for entry in entries {
            let this = literal_element_type(&entry.value);
            match &value_type {
                None => value_type = Some(this),
                Some(existing) if *existing == this => {}
                Some(_) => {
                    value_type = Some("interface{}".to_string());
                }
            }
        }
        let value_type = value_type.unwrap_or_else(|| "interface{}".to_string());
        if entries.is_empty() {
            return format!("map[string]{value_type}{{}}");
        }
        let rendered: Vec<String> = entries
            .iter()
            .map(|entry| {
                let value = self.emit_expr(&entry.value);
                format!("{}: {value}", go_string(&entry.name))
            })
            .collect();
        format!("map[string]{value_type}{{{}}}", rendered.join(", "))
    }

    fn emit_lambda(&mut self, params: &[Pattern], body: &Expr, span: Span) -> String {
        let rendered: Vec<String> = params
            .iter()
            .map(|pattern| match pattern.single_binding() {
                Some(name) => format!("{name} interface{{}}"),
                None => "_ interface{}".to_string(),
            })
            .collect();
        let body = self.emit_expr(body);
        let _ = span;
        format!(
            "func({}) interface{{}} {{ return {body} }}",
            rendered.join(", ")
        )
    }

    fn emit_sequence_literal(&mut self, elements: &[Expr]) -> String {
        let element_type = sequence_element_type(elements);
        if elements.is_empty() {
            return format!("[]{element_type}{{}}");
        }
        let rendered: Vec<String> = elements
            .iter()
            .map(|element| self.emit_expr(element))
            .collect();
        format!("[]{element_type}{{{}}}", rendered.join(", "))
    }

    /// Emits an interpolated string.
    ///
    /// Python's f-string becomes one `fmt.Sprintf` rather than a chain of `+`
    /// concatenations. Concatenation would be equivalent, but it is not
    /// `gofmt`-stable: the formatter removes the spaces around a `+` that sits
    /// inside a call, so the emitter's output would differ from its input after
    /// a formatting pass. One call is also closer to what the source says.
    fn emit_format(
        &mut self,
        segments: &[String],
        arguments: &[Expr],
        format: Option<&str>,
        span: Span,
    ) -> String {
        if format.is_some_and(|format| !format.is_empty()) {
            unsupported(
                self.diagnostics,
                span,
                "a format specifier, which needs a formatting call this backend does not yet emit",
            );
        }
        if arguments.is_empty() {
            // No interpolation, so the segments are the whole string and a
            // format verb would only add a way to be wrong.
            let text: String = segments.concat();
            return go_string(&text);
        }
        let mut template = String::new();
        for (index, segment) in segments.iter().enumerate() {
            // A literal percent has to survive as a percent, or a string
            // containing one would print the next argument twice.
            template.push_str(&segment.replace('%', "%%"));
            if arguments.get(index).is_some() {
                template.push_str("%v");
            }
        }
        let rendered: Vec<String> = arguments.iter().map(|arg| self.emit_expr(arg)).collect();
        self.imports.insert("fmt");
        format!(
            "fmt.Sprintf({}, {})",
            go_string(&template),
            rendered.join(", ")
        )
    }
}

/// A function the emitted program may call that Go does not provide.
///
/// Helpers are emitted on demand. A program that never floors a division must
/// not carry the floor helper: an unused function is not a Go compile error,
/// but an unused *import* is, and emitting a helper set unconditionally would
/// force every program to import `reflect` and `strings`.
struct Helper {
    /// The name a program refers to it by.
    name: &'static str,
    /// The emitted signature, without the opening brace.
    signature: &'static str,
    /// Packages it needs, which are added to the program's imports.
    imports: &'static [&'static str],
    /// Helpers it calls, which must be emitted alongside it.
    deps: &'static [&'static str],
    /// Its body, one source line per entry.
    body: &'static [&'static str],
}

/// Every helper this backend can emit.
fn helpers() -> &'static [Helper] {
    &[
        Helper {
            name: "gsetFloorDiv",
            signature: "func gsetFloorDiv[T ~int | ~int64](a, b T) T",
            imports: &[],
            deps: &[],
            body: &[
                "// Go truncates towards zero and Python floors towards negative",
                "// infinity, so a division that does not divide evenly is one",
                "// too large whenever the operands have different signs.",
                "q := a / b",
                "if (a%b != 0) && ((a < 0) != (b < 0)) {",
                "\tq--",
                "}",
                "return q",
            ],
        },
        Helper {
            name: "gsetFloatFloorDiv",
            signature: "func gsetFloatFloorDiv(a, b float64) float64",
            imports: &["math"],
            deps: &[],
            body: &["return math.Floor(a / b)"],
        },
        Helper {
            name: "gsetFloatPow",
            signature: "func gsetFloatPow(base, exponent float64) float64",
            imports: &["math"],
            deps: &[],
            body: &[
                "// Python keeps a negative exponent exact in a float; Go's `pow`",
                "// with integer arguments would truncate, and Go has no `**` at",
                "// all, so both the negative and the float cases land here.",
                "return math.Pow(base, exponent)",
            ],
        },
        Helper {
            name: "gsetIntPow",
            signature: "func gsetIntPow(base, exponent int64) int64",
            imports: &[],
            deps: &[],
            body: &[
                "// A negative exponent would silently come out as 1 here, which is",
                "// not Python's answer, so the emitter only sends non-negative",
                "// exponents and sends everything else to `gsetFloatPow`.",
                "result := int64(1)",
                "for i := int64(0); i < exponent; i++ {",
                "\tresult *= base",
                "}",
                "return result",
            ],
        },
        Helper {
            name: "gsetConcat",
            signature: "func gsetConcat(left, right interface{}) (interface{}, bool)",
            imports: &["reflect"],
            deps: &["gsetIsSlice"],
            body: &[
                "// Python concatenates two sequences of the same kind and nothing else: a",
                "// string with a string, a list with a list. Go's `+` does both without",
                "// checking that the two sides agree, so the check is made here, along with",
                "// a copy that keeps each element's own type — a `[]int` stays a `[]int`",
                "// until something reads it as a list of values.",
                "if leftText, ok := left.(string); ok {",
                "\trightText, rightOk := right.(string)",
                "\treturn leftText + rightText, rightOk",
                "}",
                "leftValue := reflect.ValueOf(left)",
                "rightValue := reflect.ValueOf(right)",
                "if !gsetIsSlice(leftValue) || !gsetIsSlice(rightValue) {",
                "\treturn nil, false",
                "}",
                "if leftValue.Type() != rightValue.Type() {",
                "\treturn nil, false",
                "}",
                "combined := make([]interface{}, 0, leftValue.Len()+rightValue.Len())",
                "for index := 0; index < leftValue.Len(); index++ {",
                "\tcombined = append(combined, leftValue.Index(index).Interface())",
                "}",
                "for index := 0; index < rightValue.Len(); index++ {",
                "\tcombined = append(combined, rightValue.Index(index).Interface())",
                "}",
                "return combined, true",
            ],
        },
        Helper {
            name: "gsetIsSlice",
            signature: "func gsetIsSlice(value reflect.Value) bool",
            imports: &[],
            deps: &[],
            body: &[
                "if !value.IsValid() {",
                "\treturn false",
                "}",
                "kind := value.Kind()",
                "return kind == reflect.Slice || kind == reflect.Array",
            ],
        },
        Helper {
            name: "gsetRepeat",
            signature: "func gsetRepeat(value interface{}, count int) (interface{}, bool)",
            imports: &["reflect", "strings"],
            deps: &["gsetIsSlice"],
            body: &[
                "// Python repeats a sequence by an integer count and defines nothing else,",
                "// and a negative count is the empty sequence rather than an error.",
                "if text, ok := value.(string); ok {",
                "\tif count < 0 {",
                "\t\treturn \"\", true",
                "\t}",
                "\tvar builder strings.Builder",
                "\tfor i := 0; i < count; i++ {",
                "\t\tbuilder.WriteString(text)",
                "\t}",
                "\treturn builder.String(), true",
                "}",
                "reflected := reflect.ValueOf(value)",
                "if !gsetIsSlice(reflected) {",
                "\treturn nil, false",
                "}",
                "if count < 0 {",
                "\tcount = 0",
                "}",
                "repeated := make([]interface{}, 0, reflected.Len()*count)",
                "for i := 0; i < count; i++ {",
                "\tfor index := 0; index < reflected.Len(); index++ {",
                "\t\trepeated = append(repeated, reflected.Index(index).Interface())",
                "\t}",
                "}",
                "return repeated, true",
            ],
        },
        Helper {
            name: "gsetWhole",
            signature: "func gsetWhole(value interface{}) (int, bool)",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// The value as a Go `int` when it is one of Python's integral types. A",
                "// `bool` is an `int` in Python's numeric tower and a distinct type in Go,",
                "// so `True + 1` has to come from here too.",
                "switch typed := value.(type) {",
                "case int:",
                "\treturn typed, true",
                "case uint:",
                "\treturn int(typed), true",
                "case int8:",
                "\treturn int(typed), true",
                "case int16:",
                "\treturn int(typed), true",
                "case int32:",
                "\treturn int(typed), true",
                "case int64:",
                "\treturn int(typed), true",
                "case uint8:",
                "\treturn int(typed), true",
                "case uint16:",
                "\treturn int(typed), true",
                "case uint32:",
                "\treturn int(typed), true",
                "case uint64:",
                "\treturn int(typed), true",
                "case bool:",
                "\tif typed {",
                "\t\treturn 1, true",
                "\t}",
                "\treturn 0, true",
                "}",
                "reflected := reflect.ValueOf(value)",
                "if reflected.IsValid() {",
                "\tswitch reflected.Kind() {",
                "\tcase reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:",
                "\t\treturn int(reflected.Int()), true",
                "\tcase reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:",
                "\t\treturn int(reflected.Uint()), true",
                "\t}",
                "}",
                "return 0, false",
            ],
        },
        Helper {
            name: "gsetAdd",
            signature: "func gsetAdd(left, right interface{}) interface{}",
            imports: &["fmt"],
            deps: &["gsetWhole", "gsetNumber", "gsetConcat"],
            body: &[
                "// Python's `+` is one operator over three kinds of operand: numbers add,",
                "// two sequences of the same kind concatenate, and nothing else is defined",
                "// at all. Go's `+` is three operators with none of those rules, so an",
                "// operand the source never typed comes here instead.",
                "if leftWhole, ok := gsetWhole(left); ok {",
                "\tif rightWhole, rightOk := gsetWhole(right); rightOk {",
                "\t\treturn leftWhole + rightWhole",
                "\t}",
                "}",
                "if leftNumber, ok := gsetNumber(left); ok {",
                "\tif rightNumber, rightOk := gsetNumber(right); rightOk {",
                "\t\treturn leftNumber + rightNumber",
                "\t}",
                "}",
                "if combined, ok := gsetConcat(left, right); ok {",
                "\treturn combined",
                "}",
                "panic(fmt.Sprintf(\"gset: unsupported operand types for +: %T and %T\", left, right))",
            ],
        },
        Helper {
            name: "gsetSub",
            signature: "func gsetSub(left, right interface{}) interface{}",
            imports: &["fmt"],
            deps: &["gsetWhole", "gsetNumber"],
            body: &[
                "if leftWhole, ok := gsetWhole(left); ok {",
                "\tif rightWhole, rightOk := gsetWhole(right); rightOk {",
                "\t\treturn leftWhole - rightWhole",
                "\t}",
                "}",
                "if leftNumber, ok := gsetNumber(left); ok {",
                "\tif rightNumber, rightOk := gsetNumber(right); rightOk {",
                "\t\treturn leftNumber - rightNumber",
                "\t}",
                "}",
                "panic(fmt.Sprintf(\"gset: unsupported operand types for -: %T and %T\", left, right))",
            ],
        },
        Helper {
            name: "gsetMul",
            signature: "func gsetMul(left, right interface{}) interface{}",
            imports: &["fmt"],
            deps: &["gsetWhole", "gsetNumber", "gsetRepeat"],
            body: &[
                "// `*` is the other operator Python overloads onto sequences: a string or",
                "// a list times an integer repeats, and which side the sequence is on",
                "// matters, because only a sequence may be repeated.",
                "if count, ok := gsetWhole(right); ok {",
                "\tif repeated, repeatOk := gsetRepeat(left, count); repeatOk {",
                "\t\treturn repeated",
                "\t}",
                "\tif leftNumber, numberOk := gsetNumber(left); numberOk {",
                "\t\treturn float64(count) * leftNumber",
                "\t}",
                "}",
                "if count, ok := gsetWhole(left); ok {",
                "\tif repeated, repeatOk := gsetRepeat(right, count); repeatOk {",
                "\t\treturn repeated",
                "\t}",
                "\tif rightNumber, numberOk := gsetNumber(right); numberOk {",
                "\t\treturn float64(count) * rightNumber",
                "\t}",
                "}",
                "if leftWhole, ok := gsetWhole(left); ok {",
                "\tif rightWhole, rightOk := gsetWhole(right); rightOk {",
                "\t\treturn leftWhole * rightWhole",
                "\t}",
                "}",
                "if leftNumber, ok := gsetNumber(left); ok {",
                "\tif rightNumber, rightOk := gsetNumber(right); rightOk {",
                "\t\treturn leftNumber * rightNumber",
                "\t}",
                "}",
                "panic(fmt.Sprintf(\"gset: unsupported operand types for *: %T and %T\", left, right))",
            ],
        },
        Helper {
            name: "gsetMod",
            signature: "func gsetMod(left, right interface{}) interface{}",
            imports: &["fmt", "math"],
            deps: &["gsetWhole", "gsetNumber"],
            body: &[
                "// Go's `%` takes the sign of the dividend and Python's takes the sign of",
                "// the divisor, so an integer remainder that comes out with the wrong sign",
                "// is corrected rather than returned.",
                "if leftWhole, ok := gsetWhole(left); ok {",
                "\tif rightWhole, rightOk := gsetWhole(right); rightOk {",
                "\t\tif rightWhole == 0 {",
                "\t\t\tpanic(\"gset: integer modulo by zero\")",
                "\t\t}",
                "\t\tremainder := leftWhole % rightWhole",
                "\t\tif remainder != 0 && ((remainder < 0) != (rightWhole < 0)) {",
                "\t\t\tremainder += rightWhole",
                "\t\t}",
                "\t\treturn remainder",
                "\t}",
                "}",
                "if leftNumber, ok := gsetNumber(left); ok {",
                "\tif rightNumber, rightOk := gsetNumber(right); rightOk {",
                "\t\treturn math.Mod(leftNumber, rightNumber)",
                "\t}",
                "}",
                "panic(fmt.Sprintf(\"gset: unsupported operand types for %%: %T and %T\", left, right))",
            ],
        },
        Helper {
            name: "gsetFloat",
            signature: "func gsetFloat(value interface{}) float64",
            imports: &["fmt", "strconv"],
            deps: &["gsetNumber"],
            body: &[
                "// Python's `float` converts a number, converts a `bool` to 1.0 or 0.0,",
                "// and parses a string. Go's `float64(...)` rejects a string at compile time,",
                "// so a value the source never typed is asked about here instead.",
                "if number, ok := gsetNumber(value); ok {",
                "\treturn number",
                "}",
                "if flag, ok := value.(bool); ok {",
                "\tif flag {",
                "\t\treturn 1",
                "\t}",
                "\treturn 0",
                "}",
                "if text, ok := value.(string); ok {",
                "\tparsed, err := strconv.ParseFloat(text, 64)",
                "\tif err != nil {",
                "\t\tpanic(fmt.Sprintf(\"gset: float(%q): %v\", text, err))",
                "\t}",
                "\treturn parsed",
                "}",
                "panic(fmt.Sprintf(\"gset: float() of a %T\", value))",
            ],
        },
        Helper {
            name: "gsetInt",
            signature: "func gsetInt(value interface{}) int",
            imports: &["fmt", "strconv"],
            deps: &["gsetNumber"],
            body: &[
                "// Python's `int` truncates towards zero, converts a `bool` to 1 or 0, and",
                "// parses a string, while Go's `int(...)` accepts neither a float nor a",
                "// string at all.",
                "if number, ok := gsetNumber(value); ok {",
                "\treturn int(number)",
                "}",
                "if flag, ok := value.(bool); ok {",
                "\tif flag {",
                "\t\treturn 1",
                "\t}",
                "\treturn 0",
                "}",
                "if text, ok := value.(string); ok {",
                "\tparsed, err := strconv.Atoi(text)",
                "\tif err != nil {",
                "\t\tpanic(fmt.Sprintf(\"gset: int(%q): %v\", text, err))",
                "\t}",
                "\treturn parsed",
                "}",
                "panic(fmt.Sprintf(\"gset: int() of a %T\", value))",
            ],
        },
        Helper {
            name: "gsetJoin",
            signature: "func gsetJoin(separator, items interface{}) string",
            imports: &["fmt", "reflect", "strings"],
            deps: &[],
            body: &[
                "// `separator.join(items)` renders each element as a string, which",
                "// is what Python does and what a []string cannot be trusted to",
                "// do, because the source never said the elements were strings.",
                "value := reflect.ValueOf(items)",
                "parts := make([]string, 0, value.Len())",
                "for index := 0; index < value.Len(); index++ {",
                "\tparts = append(parts, fmt.Sprint(value.Index(index).Interface()))",
                "}",
                "return strings.Join(parts, fmt.Sprint(separator))",
            ],
        },
        Helper {
            name: "gsetCount",
            signature: "func gsetCount(collection, item interface{}) int",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "total := 0",
                "for index := 0; index < value.Len(); index++ {",
                "\tif reflect.DeepEqual(value.Index(index).Interface(), item) {",
                "\t\ttotal++",
                "\t}",
                "}",
                "return total",
            ],
        },
        Helper {
            name: "gsetPairs",
            signature: "func gsetPairs(collection interface{}) [][2]interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// `for k, v in d.items()` needs the pairs to be addressable so the",
                "// two names can be bound from them, and a Go map yields keys only.",
                "// A list of tuples is the same request in Go's own types, so one",
                "// helper answers both rather than making the caller know which.",
                "value := reflect.ValueOf(collection)",
                "pairs := make([][2]interface{}, 0, value.Len())",
                "switch value.Kind() {",
                "case reflect.Map:",
                "\tfor _, key := range value.MapKeys() {",
                "\t\tpairs = append(pairs, [2]interface{}{key.Interface(), value.MapIndex(key).Interface()})",
                "\t}",
                "case reflect.Slice, reflect.Array:",
                "\tfor index := 0; index < value.Len(); index++ {",
                "\t\telement := value.Index(index)",
                "\t\tif element.Kind() == reflect.Interface {",
                "\t\t\telement = element.Elem()",
                "\t\t}",
                "\t\tif element.Kind() != reflect.Slice && element.Kind() != reflect.Array {",
                "\t\t\tpanic(\"gset: a destructuring loop binding over something that is not a collection of pairs\")",
                "\t\t}",
                "\t\tpairs = append(pairs, [2]interface{}{element.Index(0).Interface(), element.Index(1).Interface()})",
                "\t}",
                "}",
                "return pairs",
            ],
        },
        Helper {
            name: "gsetLen",
            signature: "func gsetLen(collection interface{}) int",
            imports: &["reflect"],
            deps: &[],
            body: &["return reflect.ValueOf(collection).Len()"],
        },
        Helper {
            name: "gsetIter",
            signature: "func gsetIter(collection interface{}) []interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// Go's `range` cannot walk an `interface{}`, and iterating a",
                "// mapping yields its keys, which is what Python does too.",
                "value := reflect.ValueOf(collection)",
                "items := make([]interface{}, 0, value.Len())",
                "if value.Kind() == reflect.Map {",
                "\tfor _, key := range value.MapKeys() {",
                "\t\titems = append(items, key.Interface())",
                "\t}",
                "\treturn items",
                "}",
                "for index := 0; index < value.Len(); index++ {",
                "\titems = append(items, value.Index(index).Interface())",
                "}",
                "return items",
            ],
        },
        Helper {
            name: "gsetIndex",
            signature: "func gsetIndex(collection interface{}, index interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// A negative index counts from the end, as it does in Python, and",
                "// both a slice and a map can be indexed with one call.",
                "value := reflect.ValueOf(collection)",
                "position := 0",
                "switch wanted := index.(type) {",
                "case int:",
                "\tposition = wanted",
                "case int64:",
                "\tposition = int(wanted)",
                "}",
                "if position < 0 {",
                "\tposition += value.Len()",
                "}",
                "if value.Kind() == reflect.Map {",
                "\treturn value.MapIndex(reflect.ValueOf(index)).Interface()",
                "}",
                "return value.Index(position).Interface()",
            ],
        },
        Helper {
            name: "gsetSlice",
            signature: "func gsetSlice(collection interface{}, start, end int) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// A negative bound counts from the end, as it does in Python, and",
                "// an end past the length is the length. Go rejects both outright,",
                "// so the arithmetic has to happen here where the length is known.",
                "value := reflect.ValueOf(collection)",
                "length := value.Len()",
                "if start < 0 {",
                "\tstart += length",
                "}",
                "if end < 0 {",
                "\tend += length",
                "}",
                "if end > length {",
                "\tend = length",
                "}",
                "if start < 0 {",
                "\tstart = 0",
                "}",
                "if end < start {",
                "\tend = start",
                "}",
                "return value.Slice(start, end).Interface()",
            ],
        },
        Helper {
            name: "gsetUpdate",
            signature: "func gsetUpdate(collection, other interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "incoming := reflect.ValueOf(other)",
                "for _, key := range incoming.MapKeys() {",
                "\tvalue.SetMapIndex(key, incoming.MapIndex(key))",
                "}",
                "return value.Interface()",
            ],
        },
        Helper {
            name: "gsetLines",
            signature: "func gsetLines(text string) []string",
            imports: &["strings"],
            deps: &[],
            body: &[
                "trimmed := strings.TrimRight(text, \"\\n\")",
                "if trimmed == \"\" {",
                "\treturn []string{}",
                "}",
                "return strings.Split(trimmed, \"\\n\")",
            ],
        },
        Helper {
            name: "gsetIndexOf",
            signature: "func gsetIndexOf(collection, item interface{}) int",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "for index := 0; index < value.Len(); index++ {",
                "\tif reflect.DeepEqual(value.Index(index).Interface(), item) {",
                "\t\treturn index",
                "\t}",
                "}",
                "// Python raises `ValueError` here; Go has no error in an",
                "// expression, so the sentinel every caller can test for is -1.",
                "return -1",
            ],
        },
        Helper {
            name: "gsetGet",
            signature: "func gsetGet(collection, key interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "if value.Kind() != reflect.Map {",
                "\treturn nil",
                "}",
                "found := value.MapIndex(reflect.ValueOf(key))",
                "if !found.IsValid() {",
                "\treturn nil",
                "}",
                "return found.Interface()",
            ],
        },
        Helper {
            name: "gsetGetOr",
            signature: "func gsetGetOr(collection, key, fallback interface{}) interface{}",
            imports: &[],
            deps: &["gsetGet"],
            body: &[
                "if found := gsetGet(collection, key); found != nil {",
                "\treturn found",
                "}",
                "return fallback",
            ],
        },
        Helper {
            name: "gsetKeys",
            signature: "func gsetKeys(collection interface{}) []interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "keys := make([]interface{}, 0, value.Len())",
                "for _, key := range value.MapKeys() {",
                "\tkeys = append(keys, key.Interface())",
                "}",
                "return keys",
            ],
        },
        Helper {
            name: "gsetValues",
            signature: "func gsetValues(collection interface{}) []interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "values := make([]interface{}, 0, value.Len())",
                "for _, key := range value.MapKeys() {",
                "\tvalues = append(values, value.MapIndex(key).Interface())",
                "}",
                "return values",
            ],
        },
        Helper {
            name: "gsetItems",
            signature: "func gsetItems(collection interface{}) [][2]interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// A pair, so `for pair in d.items()` and `d.items()[0][0]` both",
                "// work; a Go map cannot be ranged over as pairs at all. The pair",
                "// is a Go pair rather than a slice, so what a clause binds from it",
                "// is indexable without an assertion.",
                "value := reflect.ValueOf(collection)",
                "items := make([][2]interface{}, 0, value.Len())",
                "for _, key := range value.MapKeys() {",
                "\titems = append(items, [2]interface{}{key.Interface(), value.MapIndex(key).Interface()})",
                "}",
                "return items",
            ],
        },
        Helper {
            name: "gsetExtend",
            signature: "func gsetExtend(collection, items interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "incoming := reflect.ValueOf(items)",
                "element := value.Type().Elem()",
                "total := value.Len() + incoming.Len()",
                "grown := reflect.MakeSlice(value.Type(), total, total)",
                "reflect.Copy(grown.Slice(0, value.Len()), value)",
                "for index := 0; index < incoming.Len(); index++ {",
                "\titem := incoming.Index(index)",
                "\tif !item.Type().AssignableTo(element) {",
                "\t\tif !item.Type().ConvertibleTo(element) {",
                "\t\t\tpanic(\"gset: an extension has an element type the collection cannot hold\")",
                "\t\t}",
                "\t\titem = item.Convert(element)",
                "\t}",
                "\tgrown.Index(value.Len() + index).Set(item)",
                "}",
                "return grown.Interface()",
            ],
        },
        Helper {
            name: "gsetInsert",
            signature: "func gsetInsert(collection interface{}, index int, item interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "total := value.Len() + 1",
                "grown := reflect.MakeSlice(value.Type(), total, total)",
                "reflect.Copy(grown.Slice(0, index), value.Slice(0, index))",
                "element := reflect.ValueOf(item)",
                "if !element.Type().AssignableTo(value.Type().Elem()) {",
                "\tpanic(\"gset: an inserted value has a type the collection cannot hold\")",
                "}",
                "grown.Index(index).Set(element)",
                "reflect.Copy(grown.Slice(index+1, total), value.Slice(index, value.Len()))",
                "return grown.Interface()",
            ],
        },
        Helper {
            name: "gsetRemove",
            signature: "func gsetRemove(collection interface{}, item interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "for index := 0; index < value.Len(); index++ {",
                "\tif reflect.DeepEqual(value.Index(index).Interface(), item) {",
                "\t\ttrimmed := reflect.MakeSlice(value.Type(), value.Len()-1, value.Len()-1)",
                "\t\treflect.Copy(trimmed.Slice(0, index), value.Slice(0, index))",
                "\t\treflect.Copy(trimmed.Slice(index, trimmed.Len()), value.Slice(index+1, value.Len()))",
                "\t\treturn trimmed.Interface()",
                "\t}",
                "}",
                "return value.Interface()",
            ],
        },
        Helper {
            name: "gsetReverse",
            signature: "func gsetReverse(collection interface{}) interface{}",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "reversed := reflect.MakeSlice(value.Type(), value.Len(), value.Len())",
                "for index := 0; index < value.Len(); index++ {",
                "\treversed.Index(value.Len()-1-index).Set(value.Index(index))",
                "}",
                "return reversed.Interface()",
            ],
        },
        Helper {
            name: "gsetSort",
            signature: "func gsetSort(collection interface{}) []interface{}",
            imports: &["reflect", "sort"],
            deps: &["gsetLess"],
            body: &[
                "// Python's sort is stable, so equal keys keep the order they came",
                "// in; `sort.Slice` is not, which would make a program that sorts",
                "// two fields print something different from run to run.",
                "value := reflect.ValueOf(collection)",
                "sort.SliceStable(value.Interface(), func(left, right int) bool {",
                "\treturn gsetLess(value.Index(left).Interface(), value.Index(right).Interface())",
                "})",
                "// A dynamic sort cannot return the collection's own type, and an",
                "// `interface{}` cannot be ranged over, so what it hands back is a",
                "// slice of its elements.",
                "items := make([]interface{}, 0, value.Len())",
                "for index := 0; index < value.Len(); index++ {",
                "\titems = append(items, value.Index(index).Interface())",
                "}",
                "return items",
            ],
        },
        Helper {
            name: "gsetLess",
            signature: "func gsetLess(left, right interface{}) bool",
            imports: &[],
            deps: &["gsetCompare"],
            body: &[
                "// Python sorts values of one type, and the source's element type is",
                "// not in the program text, so the comparison is decided by the",
                "// dynamic type and an incomparable pair keeps its input order.",
                "return gsetCompare(left, right) < 0",
            ],
        },
        Helper {
            name: "gsetCompare",
            signature: "func gsetCompare(left, right interface{}) int",
            imports: &["fmt", "strings"],
            deps: &["gsetNumber"],
            body: &[
                "// Python compares by value across its numeric tower, so an int and",
                "// a float order against each other rather than failing. Go has no",
                "// operator between `int` and `float64` at all, so the promotion",
                "// has to happen here.",
                "if leftNumber, leftOk := gsetNumber(left); leftOk {",
                "\tif rightNumber, rightOk := gsetNumber(right); rightOk {",
                "\t\tif leftNumber < rightNumber {",
                "\t\t\treturn -1",
                "\t\t}",
                "\t\tif leftNumber > rightNumber {",
                "\t\t\treturn 1",
                "\t\t}",
                "\t\treturn 0",
                "\t}",
                "}",
                "if leftText, ok := left.(string); ok {",
                "\tif rightText, ok := right.(string); ok {",
                "\t\treturn strings.Compare(leftText, rightText)",
                "\t}",
                "}",
                "// A pair of values Python cannot order is a `TypeError` there, and",
                "// a sort that quietly kept the input order would report success",
                "// for a program that cannot run.",
                "panic(fmt.Sprintf(\"gset: '%T' and '%T' cannot be ordered\", left, right))",
            ],
        },
        Helper {
            name: "gsetEqual",
            signature: "func gsetEqual(left, right interface{}) bool",
            imports: &["reflect"],
            deps: &["gsetNumber"],
            body: &[
                "// Python's `==` compares containers by value and numbers across the",
                "// tower, where Go compares an interface holding a slice by panic",
                "// and an int against a float64 as unequal.",
                "if leftNumber, ok := gsetNumber(left); ok {",
                "\tif rightNumber, ok := gsetNumber(right); ok {",
                "\t\treturn leftNumber == rightNumber",
                "\t}",
                "}",
                "if leftText, ok := left.(string); ok {",
                "\tif rightText, ok := right.(string); ok {",
                "\t\treturn leftText == rightText",
                "\t}",
                "\treturn false",
                "}",
                "if left == nil || right == nil {",
                "\treturn left == right",
                "}",
                "leftValue := reflect.ValueOf(left)",
                "rightValue := reflect.ValueOf(right)",
                "if leftValue.Type() != rightValue.Type() {",
                "\treturn false",
                "}",
                "switch leftValue.Kind() {",
                "case reflect.Slice, reflect.Array:",
                "\tif leftValue.Kind() == reflect.Slice && leftValue.IsNil() || rightValue.Kind() == reflect.Slice && rightValue.IsNil() {",
                "\t\treturn leftValue.IsNil() && rightValue.IsNil()",
                "\t}",
                "\treturn reflect.DeepEqual(left, right)",
                "case reflect.Map:",
                "\tif leftValue.IsNil() || rightValue.IsNil() {",
                "\t\treturn leftValue.IsNil() && rightValue.IsNil()",
                "\t}",
                "\treturn reflect.DeepEqual(left, right)",
                "}",
                "return left == right",
            ],
        },
        Helper {
            name: "gsetDivide",
            signature: "func gsetDivide(left, right interface{}) interface{}",
            imports: &["fmt"],
            deps: &["gsetNumber"],
            body: &[
                "// Python's `/` is true division: `3 / 2` is `1.5`, where Go's own",
                "// division truncates. An operand the source never typed can only be",
                "// classified at runtime.",
                "leftNumber, leftOk := gsetNumber(left)",
                "rightNumber, rightOk := gsetNumber(right)",
                "if !leftOk || !rightOk {",
                "\tpanic(fmt.Sprintf(\"gset: a division of '%T' and '%T'\", left, right))",
                "}",
                "if rightNumber == 0 {",
                "\tpanic(\"gset: division by zero\")",
                "}",
                "return leftNumber / rightNumber",
            ],
        },
        Helper {
            name: "gsetSum",
            signature: "func gsetSum[T ~int | ~int64 | ~float32 | ~float64](collection []T) T",
            imports: &[],
            deps: &[],
            body: &[
                "// A typed sum stays typed, so `sum(xs)` can be returned where an",
                "// `int` is expected rather than arriving as an `interface{}` that",
                "// the caller has to assert.",
                "var total T",
                "for _, item := range collection {",
                "\ttotal += item",
                "}",
                "return total",
            ],
        },
        Helper {
            name: "gsetSumDynamic",
            signature: "func gsetSumDynamic(collection interface{}) interface{}",
            imports: &["fmt", "reflect"],
            deps: &["gsetNumber"],
            body: &[
                "// Python's `sum` starts from an integer zero, so an empty",
                "// collection sums to `0` rather than to nothing, and a value",
                "// that is not a number is a `TypeError` there.",
                "value := reflect.ValueOf(collection)",
                "\ttotal := float64(0)",
                "\tfor index := 0; index < value.Len(); index++ {",
                "\t\titem, ok := gsetNumber(value.Index(index).Interface())",
                "\t\tif !ok {",
                "\t\t\tpanic(fmt.Sprintf(\"gset: a sum over '%T'\", value.Index(index).Interface()))",
                "\t\t}",
                "\t\ttotal += item",
                "\t}",
                "\tif total == float64(int64(total)) {",
                "\t\treturn int(total)",
                "\t}",
                "\treturn total",
            ],
        },
        Helper {
            name: "gsetNumber",
            signature: "func gsetNumber(value interface{}) (float64, bool)",
            imports: &[],
            deps: &[],
            body: &[
                "// A `bool` is an `int` in Python's numeric tower and a distinct",
                "// type in Go, so `True + 1` has to come from the same place.",
                "switch typed := value.(type) {",
                "case int:",
                "\treturn float64(typed), true",
                "case int8:",
                "\treturn float64(typed), true",
                "case int16:",
                "\treturn float64(typed), true",
                "case int32:",
                "\treturn float64(typed), true",
                "case int64:",
                "\treturn float64(typed), true",
                "case uint:",
                "\treturn float64(typed), true",
                "case uint8:",
                "\treturn float64(typed), true",
                "case uint16:",
                "\treturn float64(typed), true",
                "case uint32:",
                "\treturn float64(typed), true",
                "case uint64:",
                "\treturn float64(typed), true",
                "case float32:",
                "\treturn float64(typed), true",
                "case float64:",
                "\treturn typed, true",
                "}",
                "return 0, false",
            ],
        },
        Helper {
            name: "gsetPop",
            signature: "func gsetPop(collection interface{}) (interface{}, interface{})",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "value := reflect.ValueOf(collection)",
                "last := value.Len() - 1",
                "return value.Index(last).Interface(), value.Slice(0, last).Interface()",
            ],
        },
        Helper {
            name: "gsetTruthy",
            signature: "func gsetTruthy(value interface{}) bool",
            imports: &["reflect"],
            deps: &[],
            body: &[
                "// Python's truthiness is a runtime property of the value, and a Go",
                "// condition is a static type. Emitting `if value` is only correct",
                "// for a bool, so everything else asks here.",
                "if value == nil {",
                "\treturn false",
                "}",
                "reflected := reflect.ValueOf(value)",
                "switch reflected.Kind() {",
                "case reflect.Bool:",
                "\treturn reflected.Bool()",
                "case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:",
                "\treturn reflected.Int() != 0",
                "case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:",
                "\treturn reflected.Uint() != 0",
                "case reflect.Float32, reflect.Float64:",
                "\treturn reflected.Float() != 0",
                "case reflect.String:",
                "\treturn reflected.Len() != 0",
                "case reflect.Slice, reflect.Map, reflect.Array:",
                "\treturn reflected.Len() != 0",
                "case reflect.Chan, reflect.Func, reflect.Pointer, reflect.Interface:",
                "\treturn !reflected.IsNil()",
                "}",
                "return true",
            ],
        },
        Helper {
            name: "gsetOr",
            signature: "func gsetOr(left, right interface{}) interface{}",
            imports: &[],
            deps: &["gsetTruthy"],
            body: &[
                "// `or` yields an operand, not a bool, which is why Go's `||` cannot",
                "// stand in for it once either operand is not a bool.",
                "if gsetTruthy(left) {",
                "\treturn left",
                "}",
                "return right",
            ],
        },
        Helper {
            name: "gsetAnd",
            signature: "func gsetAnd(left, right interface{}) interface{}",
            imports: &[],
            deps: &["gsetTruthy"],
            body: &[
                "if gsetTruthy(left) {",
                "\treturn right",
                "}",
                "return left",
            ],
        },
        Helper {
            name: "gsetContains",
            signature: "func gsetContains(collection, item interface{}) bool",
            imports: &["reflect", "strings"],
            deps: &[],
            body: &[
                "// `in` is a protocol over every container Python has and one",
                "// operator Go lacks, so it is decided by kind rather than by",
                "// a static type the source never gave us.",
                "if collection == nil {",
                "\treturn false",
                "}",
                "value := reflect.ValueOf(collection)",
                "switch value.Kind() {",
                "case reflect.String:",
                "\tother, ok := item.(string)",
                "\tif !ok {",
                "\t\treturn false",
                "\t}",
                "\treturn strings.Contains(value.String(), other)",
                "case reflect.Slice, reflect.Array:",
                "\tfor index := 0; index < value.Len(); index++ {",
                "\t\tif reflect.DeepEqual(value.Index(index).Interface(), item) {",
                "\t\t\treturn true",
                "\t\t}",
                "\t}",
                "\treturn false",
                "case reflect.Map:",
                "\tfor _, key := range value.MapKeys() {",
                "\t\tif reflect.DeepEqual(key.Interface(), item) {",
                "\t\t\treturn true",
                "\t\t}",
                "\t}",
                "\treturn false",
                "\tdefault:",
                "\treturn false",
                "}",
            ],
        },
    ]
}

// ------------------------------------------------------------- free helpers

/// The Go type of a value whose type the source fixed, if it has one.
///
/// `Unknown` means the source left it dynamic, and naming a concrete type for
/// it is the guess this backend refuses to make.
fn concrete_type(expr: &Expr) -> Option<String> {
    named_type(&expr.ty)
}

/// Whether Python's `==` on this value is a question only the runtime can answer.
///
/// Go compares an interface holding a slice or a map by panicking, and tells an
/// `int` and a `float64` apart where Python calls 1 and 1.0 equal, so a value
/// the source never typed — or one Go cannot compare by value — needs a helper.
fn needs_value_equality(expr: &Expr) -> bool {
    concrete_type(expr).is_none()
        || matches!(
            expr.ty,
            Type::List { .. }
                | Type::Tuple(_)
                | Type::Map(_)
                | Type::OrderedMap { .. }
                | Type::Set(_)
                | Type::Bytes
        )
}

/// Which operand of a numeric expression has to be promoted to a float.
///
/// Python adds an `int` to a `float` and orders the two; Go rejects the
/// expression outright. The promotion is written out here because both operand
/// types are already known, and `float64` is a conversion rather than a call.
fn numeric_promotion(lhs: &Expr, rhs: &Expr) -> Option<(bool, bool)> {
    match (numeric_rank(&lhs.ty), numeric_rank(&rhs.ty)) {
        (Some(1), Some(2)) => Some((true, false)),
        (Some(2), Some(1)) => Some((false, true)),
        _ => None,
    }
}

/// Records the Go type of every value `statements` binds a name to.
///
/// A binding's type is what `gset-semantic` inferred for its value, rendered as
/// Go spells it, so two spellings of one Go type count once and only genuinely
/// different types are collected.
fn collect_bind_types_statement(statement: &Stmt, types: &mut HashMap<String, BTreeSet<String>>) {
    fn record(target: &Pattern, ty: Type, types: &mut HashMap<String, BTreeSet<String>>) {
        let rendered = go_type(&ty);
        for name in target.bound_names() {
            types
                .entry(name.to_string())
                .or_default()
                .insert(rendered.clone());
        }
    }
    match statement {
        Stmt::Assign { target, value, .. } => record(target, value.ty.clone(), types),
        Stmt::Decl(decl) => record(
            &decl.pattern,
            decl.ty.clone().unwrap_or(Type::UNKNOWN),
            types,
        ),
        _ => {}
    }
    // An assignment is an expression in Python, so `total += item` inside a
    // loop binds a name as surely as a statement does.
    if let Stmt::Expr(expr)
    | Stmt::Return {
        value: Some(expr), ..
    } = statement
    {
        collect_bind_types_expr(expr, types);
    }
    // A binding nested in a block binds the same name, and Go fixes the
    // variable's type at its first assignment whatever block that is in.
    match statement {
        Stmt::Block(block) => {
            for nested in &block.statements {
                collect_bind_types_statement(nested, types);
            }
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            for nested in &then_branch.statements {
                collect_bind_types_statement(nested, types);
            }
            match else_branch.as_deref() {
                Some(Else::Block(block)) => {
                    for nested in &block.statements {
                        collect_bind_types_statement(nested, types);
                    }
                }
                Some(Else::If(nested)) => collect_bind_types_statement(nested, types),
                None => {}
            }
        }
        Stmt::While {
            body, else_body, ..
        }
        | Stmt::ForIn {
            body, else_body, ..
        } => {
            for nested in &body.statements {
                collect_bind_types_statement(nested, types);
            }
            if let Some(else_body) = else_body {
                for nested in &else_body.statements {
                    collect_bind_types_statement(nested, types);
                }
            }
        }
        Stmt::For { body, .. } | Stmt::DoWhile { body, .. } | Stmt::With { body, .. } => {
            for nested in &body.statements {
                collect_bind_types_statement(nested, types);
            }
        }
        _ => {}
    }
}

fn collect_bind_types_expr(expr: &Expr, types: &mut HashMap<String, BTreeSet<String>>) {
    if let ExprKind::Assign { target, value, .. } = &expr.kind {
        let rendered = go_type(&value.ty);
        for name in target.bound_names() {
            types
                .entry(name.to_string())
                .or_default()
                .insert(rendered.clone());
        }
    }
    for child in expr.children() {
        collect_bind_types_expr(child, types);
    }
}

/// The names `statements` read.
///
/// A name read anywhere in the function counts as read, which is what Go's own
/// rule asks: it looks at the declaration and any later use, not at whether the
/// use happens on the path that assigned it.
fn collect_reads_statement(statement: &Stmt, reads: &mut HashSet<String>) {
    match statement {
        Stmt::Decl(decl) => {
            if let Some(value) = &decl.value {
                collect_reads_expr(value, reads);
            }
        }
        Stmt::Expr(expr) => collect_reads_expr(expr, reads),
        Stmt::Assign { target, value, .. } => {
            // `a[i] = v` reads `a` and `i`; only `a = v` writes a name.
            if let PatternKind::Location(location) = &target.kind {
                collect_reads_expr(location, reads);
            }
            collect_reads_expr(value, reads);
        }
        Stmt::Return {
            value: Some(value), ..
        } => collect_reads_expr(value, reads),
        Stmt::Block(block) => collect_reads_block(block, reads),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_reads_expr(condition, reads);
            collect_reads_block(then_branch, reads);
            collect_reads_else(else_branch.as_deref(), reads);
        }
        Stmt::While {
            condition,
            body,
            else_body,
            ..
        } => {
            collect_reads_expr(condition, reads);
            collect_reads_block(body, reads);
            if let Some(else_body) = else_body {
                collect_reads_block(else_body, reads);
            }
        }
        Stmt::For { body, .. } => collect_reads_block(body, reads),
        Stmt::ForIn {
            iterable,
            body,
            else_body,
            ..
        } => {
            collect_reads_expr(iterable, reads);
            collect_reads_block(body, reads);
            if let Some(else_body) = else_body {
                collect_reads_block(else_body, reads);
            }
        }
        Stmt::DoWhile { body, .. } => collect_reads_block(body, reads),
        Stmt::With { body, .. } => collect_reads_block(body, reads),
        _ => {}
    }
}

fn collect_reads_block(block: &Block, reads: &mut HashSet<String>) {
    for statement in &block.statements {
        collect_reads_statement(statement, reads);
    }
}

fn collect_reads_else(else_branch: Option<&Else>, reads: &mut HashSet<String>) {
    match else_branch {
        Some(Else::Block(block)) => collect_reads_block(block, reads),
        Some(Else::If(nested)) => collect_reads_statement(nested, reads),
        None => {}
    }
}

fn collect_reads_expr(expr: &Expr, reads: &mut HashSet<String>) {
    if let ExprKind::Path(path) = &expr.kind
        && path.is_bare()
        && let Some(head) = path.segments.first()
    {
        reads.insert(head.to_string());
    }
    if let ExprKind::StructLit { path, .. } = &expr.kind
        && let Some(head) = path.segments.first()
    {
        reads.insert(head.to_string());
    }
    for child in expr.children() {
        collect_reads_expr(child, reads);
    }
}

/// The names `statements` bind on every path through them.
///
/// A name bound by an assignment binds from there on, a name bound inside an
/// `if` binds afterwards only when every path through that `if` binds it, and
/// a name bound in a loop body does not bind at all, because the body may not
/// run. Nested blocks bind nothing outside themselves, since Go — and
/// Python — scope a name to the block that assigns it.
fn definitely_bound(statements: &[Stmt]) -> HashSet<String> {
    let mut bound = HashSet::new();
    for statement in statements {
        match statement {
            Stmt::Assign { target, .. } => {
                for name in target.bound_names() {
                    bound.insert(name.to_string());
                }
            }
            Stmt::Decl(decl) => {
                for name in decl.pattern.bound_names() {
                    bound.insert(name.to_string());
                }
            }
            Stmt::Block(block) => {
                bound.extend(definitely_bound(&block.statements));
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                let mut paths = definitely_bound(&then_branch.statements);
                let other = match else_branch.as_deref() {
                    Some(Else::Block(block)) => definitely_bound(&block.statements),
                    Some(Else::If(nested)) => {
                        definitely_bound(std::slice::from_ref(nested.as_ref()))
                    }
                    // Without an else, the condition may be false and bind
                    // nothing at all.
                    None => HashSet::new(),
                };
                paths = paths.intersection(&other).cloned().collect();
                bound.extend(paths);
            }
            _ => {}
        }
    }
    bound
}

/// The type each name is assigned, taking the last assignment when there are
/// several.
///
/// The types here come from `gset-semantic`, which ran before this emitter and
/// is a statement of what the source computes, not a guess made here.
fn assignment_types(statements: &[Stmt]) -> HashMap<String, Type> {
    let mut types = HashMap::new();
    for statement in statements {
        match statement {
            Stmt::Assign { target, value, .. } => {
                for name in target.bound_names() {
                    types.insert(name.to_string(), value.ty.clone());
                }
            }
            Stmt::Decl(decl) => {
                for name in decl.pattern.bound_names() {
                    types.insert(name.to_string(), decl.ty.clone().unwrap_or(Type::UNKNOWN));
                }
            }
            Stmt::Block(block) => {
                types.extend(assignment_types(&block.statements));
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                types.extend(assignment_types(&then_branch.statements));
                match else_branch.as_deref() {
                    Some(Else::Block(block)) => {
                        types.extend(assignment_types(&block.statements));
                    }
                    Some(Else::If(nested)) => {
                        types.extend(assignment_types(std::slice::from_ref(nested.as_ref())));
                    }
                    None => {}
                }
            }
            Stmt::While {
                body, else_body, ..
            }
            | Stmt::ForIn {
                body, else_body, ..
            } => {
                types.extend(assignment_types(&body.statements));
                if let Some(else_body) = else_body {
                    types.extend(assignment_types(&else_body.statements));
                }
            }
            Stmt::For { body, .. } | Stmt::DoWhile { body, .. } | Stmt::With { body, .. } => {
                types.extend(assignment_types(&body.statements));
            }
            _ => {}
        }
    }
    types
}

/// The helper that answers an arithmetic operation on an operand the source
/// never typed, if this backend has one.
///
/// A bitwise operator is not among them: Python's `&` is a whole different
/// thing on a set, a bytes object and a bool than it is on an integer, and
/// guessing which one the source meant is the answer a transpiler must not
/// give.
fn dynamic_arithmetic_helper(op: BinaryOp) -> Option<&'static str> {
    match op {
        BinaryOp::Add => Some("gsetAdd"),
        BinaryOp::Sub => Some("gsetSub"),
        BinaryOp::Mul => Some("gsetMul"),
        BinaryOp::Rem => Some("gsetMod"),
        // `//`, `/` and `**` are answered before this point: each already
        // chooses between an int and a float form of its own.
        _ => None,
    }
}

/// The two numeric families Go keeps apart: integral and floating point.
fn numeric_rank(ty: &Type) -> Option<u8> {
    match ty {
        Type::Int(_) | Type::Char => Some(1),
        Type::Float => Some(2),
        _ => None,
    }
}

/// The Go type of `ty`, unless the source left it dynamic.
///
/// A type that renders as `interface{}` counts as dynamic even when inference
/// named it: a tuple of unlike elements has no Go spelling, so it reaches Go as
/// an `interface{}` that cannot be indexed, sliced or ranged over directly.
fn named_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Unknown(_) | Type::Null | Type::Union(_) | Type::Generic(_) | Type::Never => None,
        ty => {
            let rendered = go_type(ty);
            (!rendered.is_empty() && rendered != "interface{}").then_some(rendered)
        }
    }
}

/// Whether a value is a mapping or a set, so `remove` is a `delete`.
///
/// Decided from the inferred type rather than from the method name: Python's
/// `remove` is a `delete` on a set or dict and a slice rebuild on a list, and
/// the two are not interchangeable in Go.
fn is_mapping_like(expr: &Expr) -> bool {
    matches!(
        expr.ty,
        Type::Map(_) | Type::OrderedMap { .. } | Type::Set(_)
    )
}

/// The integer a bound literal stands for, including a unary minus.
fn literal_bound(expr: &Expr) -> Option<i64> {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(text)) => text.parse().ok(),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        } => literal_bound(operand).map(|bound| -bound),
        _ => None,
    }
}

/// Whether an expression can be rendered twice without changing anything.
///
/// A translation that needs a value in two places — a length for a shifted
/// index, a default for an omitted slice bound — may only do it for a value
/// whose second evaluation is the same as its first.
fn is_repeatable(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Path(_)
            | ExprKind::Literal(_)
            | ExprKind::Index { .. }
            | ExprKind::Field { .. }
            | ExprKind::Slice { .. }
    )
}

/// `depth` tabs, for indenting a line that continues a statement.
fn indent(depth: usize) -> String {
    "\t".repeat(depth)
}

fn pattern_name(pattern: &Pattern) -> String {
    pattern
        .single_binding()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "_".to_string())
}

/// Maps an IR type to a Go type.
///
/// `Unknown` becomes `interface{}`, never a concrete type: guessing here is the
/// bug the whole design avoids.
fn go_type(ty: &Type) -> String {
    match ty {
        Type::Unknown(_) | Type::Never | Type::Null | Type::Union(_) | Type::Generic(_) => {
            "interface{}".to_string()
        }
        Type::Void => String::new(),
        Type::Bool => "bool".to_string(),
        Type::Int(_) => "int".to_string(),
        Type::Float => "float64".to_string(),
        Type::Str | Type::StrLit => "string".to_string(),
        Type::Bytes => "[]byte".to_string(),
        Type::Char => "rune".to_string(),
        Type::List { element, .. } => format!("[]{}", go_type(element)),
        Type::Tuple(elements) => {
            let mut kinds: Vec<String> = elements.iter().map(go_type).collect();
            kinds.dedup();
            match kinds.as_slice() {
                [only] => format!("[]{only}"),
                _ => "[]interface{}".to_string(),
            }
        }
        Type::Map(value) => format!("map[string]{}", go_type(value)),
        Type::Set(element) => format!("map[{}]struct{{}}", go_type(element)),
        Type::OrderedMap { key, value, .. } => {
            format!("map[{}]{}", go_type(key), go_type(value))
        }
        // An optional value is either the value or the absent one, and Go's way
        // to say that is `interface{}` holding the value or `nil`. Rendering the
        // inner type instead would turn `Optional[int]` into an `int` that
        // cannot hold the `None` the source can return.
        Type::Option(_) => "interface{}".to_string(),
        Type::Function {
            params,
            ret,
            variadic,
            ..
        } => {
            let mut param_types: Vec<String> = params.iter().map(go_type).collect();
            if *variadic && let Some(last) = param_types.pop() {
                param_types.push(format!("...{last}"));
            }
            let ret = go_type(ret);
            if ret.is_empty() {
                format!("func({})", param_types.join(", "))
            } else {
                format!("func({}) {ret}", param_types.join(", "))
            }
        }
        Type::Named(name) => name.to_string(),
        // Generics are erased: the IR records the application, but emitting a
        // Go instantiation is `gset-semantic`'s job once it resolves the
        // declaration.
        Type::Applied { base, .. } => base.to_string(),
    }
}

fn go_literal(literal: &Literal) -> String {
    match literal {
        Literal::Int(text) | Literal::Float(text) => text.clone(),
        Literal::Str(text) => go_string(text),
        Literal::Bytes(text) => format!("[]byte({})", go_string(text)),
        Literal::Char(text) => format!("'{}'", text.trim_matches('\'')),
        Literal::Bool(true) => "true".to_string(),
        Literal::Bool(false) => "false".to_string(),
        Literal::Null => "nil".to_string(),
    }
}

/// Quotes `text` as a Go interpreted string.
///
/// The text is the source's inner content with its escapes intact, so a
/// backslash is left alone and only a bare double quote is escaped. Re-encoding
/// every escape would turn a source `\n` into `\\n`, which the Go emitter's
/// `%q` formatting did to JavaScript output.
fn go_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The digits of a decimal integer literal, if `text` is one.
///
/// Bases other than ten are skipped: their width is visible in the digits, and
/// Python's arbitrary precision does not make `0xFFFFFFFFFFFFFFFFF` a value a
/// 64-bit target holds either, but reporting it is the frontends job because it
/// is a source-level fact rather than a target-width one.
fn plain_decimal(text: &str) -> Option<&str> {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(digits)
}

fn binary_symbol(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::FloorDiv | BinaryOp::Pow => return None,
    })
}

fn compound_symbol(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::FloorDiv | BinaryOp::Pow => return None,
    })
}

fn comparison_symbol(op: ComparisonOp) -> Option<&'static str> {
    Some(match op {
        ComparisonOp::Eq | ComparisonOp::Is => "==",
        ComparisonOp::Ne | ComparisonOp::IsNot => "!=",
        ComparisonOp::Lt => "<",
        ComparisonOp::Le => "<=",
        ComparisonOp::Gt => ">",
        ComparisonOp::Ge => ">=",
        ComparisonOp::In | ComparisonOp::NotIn => return None,
    })
}

/// The Go type a comprehension produces for one element.
///
/// The inferred type wins because it knows about a value the source computed;
/// literals are the fallback for a program that never gave the element a type,
/// and anything else stays `interface{}`.
fn element_go_type(expr: &Expr) -> String {
    named_type(&expr.ty).unwrap_or_else(|| literal_element_type(expr))
}

/// The Go element type for a sequence literal, inferred from its elements.
///
/// A homogeneous literal gets a concrete slice type; anything mixed or
/// non-literal gets `interface{}`. This is inference over literals only, not a
/// guess about a value the source left untyped.
fn sequence_element_type(elements: &[Expr]) -> String {
    let mut kind: Option<String> = None;
    for element in elements {
        let this = literal_element_type(element);
        match &kind {
            None => kind = Some(this),
            Some(existing) if *existing == this => {}
            Some(_) => return "interface{}".to_string(),
        }
    }
    kind.unwrap_or_else(|| "interface{}".to_string())
}

/// The Go element type of a sequence value, if its type names one.
fn sequence_element_type_of(ty: &Type) -> Option<String> {
    match ty {
        Type::List { element, .. } | Type::Set(element) => {
            let rendered = go_type(element);
            (!rendered.is_empty() && rendered != "interface{}").then_some(rendered)
        }
        _ => None,
    }
}

/// Whether a Go type is one of the numeric families a typed sum can add.
fn numeric_go_type(go_type_name: &str) -> Option<()> {
    matches!(
        go_type_name,
        "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "float32"
            | "float64"
    )
    .then_some(())
}

/// The Go type one element of a literal sequence has.
///
/// A nested sequence is a sequence itself, so a list of lists keeps its shape
/// as `[][]int` rather than collapsing to `[]interface{}` — the type a clause
/// that ranges over it is inferred from has to be the type it was emitted with,
/// or the loop binds an `interface{}` the next expression cannot walk.
fn literal_element_type(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(_)) => "int".to_string(),
        ExprKind::Literal(Literal::Float(_)) => "float64".to_string(),
        ExprKind::Literal(Literal::Str(_)) => "string".to_string(),
        ExprKind::Literal(Literal::Bytes(_)) => "[]byte".to_string(),
        ExprKind::Literal(Literal::Bool(_)) => "bool".to_string(),
        ExprKind::List { elements } | ExprKind::Tuple { elements } => {
            format!("[]{}", sequence_element_type(elements))
        }
        ExprKind::Set { elements } => {
            format!("map[{}]struct{{}}", sequence_element_type(elements))
        }
        ExprKind::Map { entries } => {
            // The IR's mappings are string-keyed, so only the value side has
            // more than one type to reconcile.
            let values: Vec<Expr> = entries.iter().map(|entry| entry.value.clone()).collect();
            let key = named_type(&expr.ty)
                .map(|ty| {
                    ty.trim_start_matches("map[")
                        .split(']')
                        .next()
                        .unwrap_or("string")
                        .to_string()
                })
                .unwrap_or_else(|| "string".to_string());
            format!("map[{key}]{}", sequence_element_type(&values))
        }
        _ => named_type(&expr.ty).unwrap_or_else(|| "interface{}".to_string()),
    }
}

/// The arguments of a Python `range(...)` call, if `iterable` is one.
fn range_arguments(iterable: &Expr) -> Option<RangeArgs<'_>> {
    let ExprKind::Call { callee, args, .. } = &iterable.kind else {
        return None;
    };
    let ExprKind::Path(path) = &callee.kind else {
        return None;
    };
    if !path.is_bare() || path.segments[0].as_ref() != "range" {
        return None;
    }
    match args.len() {
        1 => Some(RangeArgs {
            start: None,
            stop: &args[0],
            step: None,
            span: iterable.span,
        }),
        2 => Some(RangeArgs {
            start: Some(&args[0]),
            stop: &args[1],
            step: None,
            span: iterable.span,
        }),
        3 => Some(RangeArgs {
            start: Some(&args[0]),
            stop: &args[1],
            step: Some(&args[2]),
            span: iterable.span,
        }),
        _ => None,
    }
}

/// A parsed `range(...)`.
struct RangeArgs<'a> {
    start: Option<&'a Expr>,
    stop: &'a Expr,
    step: Option<&'a Expr>,
    span: Span,
}

impl RangeArgs<'_> {
    fn header(&self, index: &str, emitter: &mut Emitter<'_>) -> String {
        let start = match self.start {
            Some(start) => emitter.emit_expr(start),
            None => "0".to_string(),
        };
        let stop = emitter.emit_expr(self.stop);
        match self.step {
            // A literal positive step keeps the idiomatic `i += step` form.
            Some(step) if matches!(step.kind, ExprKind::Literal(Literal::Int(_))) => {
                let step = emitter.emit_expr(step);
                format!("{index} := {start}; {index} < {stop}; {index} += {step}")
            }
            None => format!("{index} := {start}; {index} < {stop}; {index}++"),
            Some(_) => {
                // A computed step would need a temporary for the step and a
                // direction that depends on its sign; reported rather than
                // assuming it is positive.
                unsupported(
                    emitter.diagnostics,
                    self.span,
                    "a `range` with a computed step",
                );
                format!("{index} := {start}; {index} < {stop}; {index}++")
            }
        }
    }
}

/// Whether a function has a `return` that carries a value.
fn function_returns_value(function: &Function) -> bool {
    if function.ret.is_some() {
        return true;
    }
    let mut found = false;
    scan_returns(&function.body, &mut found);
    found
}

fn scan_returns(block: &Block, found: &mut bool) {
    for statement in &block.statements {
        match statement {
            Stmt::Return { value: Some(_), .. } => *found = true,
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                scan_returns(then_branch, found);
                match else_branch.as_deref() {
                    Some(Else::Block(block)) => scan_returns(block, found),
                    Some(Else::If(nested)) => {
                        let block = Block::new(vec![nested.as_ref().clone()], nested.span());
                        scan_returns(&block, found);
                    }
                    None => {}
                }
            }
            Stmt::While { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::For { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::With { body, .. }
            | Stmt::Block(body) => scan_returns(body, found),
            Stmt::Try {
                body,
                handlers,
                finally,
                ..
            } => {
                scan_returns(body, found);
                for handler in handlers {
                    scan_returns(&handler.body, found);
                }
                if let Some(finally) = finally {
                    scan_returns(finally, found);
                }
            }
            _ => {}
        }
    }
}

/// The Go return type for a function, if it has one.
fn function_return_type(function: &Function, returns_value: bool) -> Option<String> {
    match &function.ret {
        Some(Type::Void) => None,
        Some(ty) => Some(go_type(ty)),
        // With no declared return type, a function that returns values is typed
        // `interface{}`: the honest Go spelling of "the source is untyped". A
        // function with no value returns is void in Go.
        None if returns_value => Some("interface{}".to_string()),
        None => None,
    }
}

/// Whether control cannot reach the end of a block without leaving the function.
///
/// Only an unconditional exit counts. A loop can run zero times and a `break`
/// leaves it early, so neither proves anything, and an `if` proves it only when
/// both arms do: Python falls off the end of an `if` whose other arm returns.
fn always_returns(block: &Block) -> bool {
    block.statements.iter().any(|statement| match statement {
        Stmt::Return { .. } | Stmt::Throw { .. } => true,
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => always_returns(then_branch) && else_returns(else_branch),
        Stmt::Block(inner) => always_returns(inner),
        _ => false,
    })
}

/// Whether every branch of an `else` leaves the function.
fn else_returns(else_branch: &Else) -> bool {
    match else_branch {
        Else::Block(block) => always_returns(block),
        Else::If(nested) => matches!(nested.as_ref(), Stmt::If { then_branch, else_branch, .. }
            if always_returns(then_branch) && else_branch.as_deref().is_some_and(else_returns)),
    }
}

/// The value a Go function returns for a Python `None` that fell off the end.
///
/// Go has no `None`, so a function that can end without returning needs its
/// type's zero value: `nil` for the reference types and the dynamic `interface{}`
/// a caller compares against `nil`, and the literal zero for the rest.
fn zero_value(go_type_name: &str) -> &'static str {
    match go_type_name {
        "bool" => "false",
        "string" => "\"\"",
        "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32"
        | "uint64" | "byte" | "rune" | "float32" | "float64" => "0",
        _ => "nil",
    }
}

/// The source-level text of a name or attribute chain.
///
/// Unlike [`Emitter::emit_expr`] this never substitutes a placeholder for a
/// reported name, so it can be quoted inside a diagnostic comment without
/// nesting one comment inside another.
fn dotted_text(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Path(path) => path
            .segments
            .iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>()
            .join("."),
        ExprKind::Field { target, field } => format!("{}.{field}", dotted_text(target)),
        _ => String::new(),
    }
}

/// The leftmost name an expression starts from.
///
/// `os.path.join` is a field access on a field access on a path, and only the
/// leftmost segment says which module a name came from — which is the question
/// a dropped import raises.
fn root_name(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Path(path) => path.segments.first().map(|segment| segment.as_ref()),
        ExprKind::Field { target, .. } => root_name(target),
        _ => None,
    }
}

/// The names Python provides without an import.
///
/// Used to tell two failures apart at a call site: a builtin this backend has
/// no answer for is a gap in the backend, while a name that is not a builtin and
/// not bound by the module is a mistake in the source. Both report, but they
/// are not the same defect and a diagnostic that says "this builtin" for a name
/// the program never defined would point at the wrong thing.
fn python_builtins() -> &'static [&'static str] {
    &[
        "abs",
        "aiter",
        "all",
        "anext",
        "any",
        "ascii",
        "bin",
        "bool",
        "breakpoint",
        "bytearray",
        "bytes",
        "callable",
        "chr",
        "classmethod",
        "compile",
        "complex",
        "delattr",
        "dict",
        "dir",
        "divmod",
        "enumerate",
        "eval",
        "exec",
        "filter",
        "float",
        "format",
        "frozenset",
        "getattr",
        "globals",
        "hasattr",
        "hash",
        "help",
        "hex",
        "id",
        "input",
        "int",
        "isinstance",
        "issubclass",
        "iter",
        "len",
        "list",
        "locals",
        "map",
        "max",
        "min",
        "next",
        "object",
        "oct",
        "open",
        "ord",
        "pow",
        "print",
        "property",
        "range",
        "repr",
        "reversed",
        "round",
        "set",
        "setattr",
        "slice",
        "sorted",
        "staticmethod",
        "str",
        "sum",
        "super",
        "tuple",
        "type",
        "vars",
        "zip",
    ]
}

fn describe_item(item: &Item) -> &'static str {
    match item {
        Item::Class(_) => "a class",
        Item::Record(_) => "a record",
        Item::Enum(_) => "an enum",
        Item::Interface(_) => "an interface",
        Item::TypeAlias(_) => "a type alias",
        _ => "this declaration",
    }
}

fn describe_stmt(statement: &Stmt) -> &'static str {
    match statement {
        Stmt::For { .. } => "a C-style `for`",
        Stmt::DoWhile { .. } => "a `do`/`while`",
        Stmt::Switch { .. } => "a `match`/`switch`",
        Stmt::Try { .. } => "a `try`",
        Stmt::With { .. } => "a `with`",
        Stmt::Delete { .. } => "a `delete`",
        Stmt::Yield { .. } => "a `yield`",
        _ => "this statement",
    }
}

fn describe_expr(kind: &ExprKind) -> &'static str {
    match kind {
        ExprKind::Slice { .. } => "a slice",
        ExprKind::Set { .. } => "a set literal",
        ExprKind::Map { .. } => "a map literal",
        ExprKind::StructLit { .. } => "a struct literal",
        ExprKind::Comprehension { .. } => "a comprehension",
        ExprKind::Let { .. } => "a scoped binding",
        ExprKind::Assign { .. } => "an assignment expression",
        ExprKind::Await { .. } => "an `await`",
        ExprKind::Cast { .. } => "a cast",
        ExprKind::TypeAssert { .. } => "a type assertion",
        ExprKind::Range { .. } => "a range value",
        ExprKind::Unpack { .. } => "an unpacked value",
        ExprKind::Conditional { .. } => "a conditional expression",
        _ => "this expression",
    }
}

fn describe_method(method: &str) -> String {
    format!("the `.{method}` method")
}
