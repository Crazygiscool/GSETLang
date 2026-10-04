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

use std::collections::HashSet;

use gset_ir::{
    BinaryOp, Block, ComparisonOp, Else, Expr, ExprKind, Function, Import, Item, Literal,
    LogicalOp, Module, NamedArg, Pattern, PatternKind, SourceMap, Span, Stmt, Type, UnaryOp,
    VarDecl,
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
        let mut body = CodeWriter::new(self.indentation());
        let (needs_fmt, text) = {
            let mut emitter = Emitter::new(&mut body, diagnostics);
            emitter.emit(module);
            (emitter.needs_fmt, body.into_string())
        };

        writer.writeln("package main");
        writer.blank();
        if needs_fmt {
            writer.writeln("import \"fmt\"");
            writer.blank();
        }
        writer.write(&text);
    }
}

/// One emission run.
struct Emitter<'a> {
    out: &'a mut CodeWriter,
    diagnostics: &'a mut DiagnosticBag,
    /// Whether the emitted program calls into `fmt`.
    needs_fmt: bool,
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
    /// Whether the current function returns a value, which decides whether a
    /// bare `return` becomes `return nil`.
    returns_value: bool,
}

impl<'a> Emitter<'a> {
    fn new(out: &'a mut CodeWriter, diagnostics: &'a mut DiagnosticBag) -> Self {
        Emitter {
            out,
            diagnostics,
            needs_fmt: false,
            main_statements: Vec::new(),
            globals: HashSet::new(),
            locals: HashSet::new(),
            returns_value: false,
        }
    }

    // -------------------------------------------------------------- module

    fn emit(&mut self, module: &Module) {
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

        self.emit_main();
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
        let signature = match ret {
            Some(ret) => format!("func {}{params} {ret} {{", function.name),
            None => format!("func {}{params} {{", function.name),
        };
        self.out.blank();

        let outer_locals = std::mem::take(&mut self.locals);
        let outer_returns = self.returns_value;
        self.returns_value = returns_value;

        self.out.writeln(&signature);
        self.out.indent();
        self.bind_params(function);
        self.emit_block_body(&function.body);
        self.out.dedent();
        self.out.writeln("}");

        self.locals = outer_locals;
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
        for pattern in &function.params {
            for name in pattern.bound_names() {
                self.locals.insert(name.to_string());
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
            } => {
                if else_body.is_some() {
                    unsupported(self.diagnostics, *span, "a loop `else` clause");
                }
                let condition = self.emit_condition(condition);
                self.out.writeln(&format!("for {condition} {{"));
                self.out.indent();
                self.emit_block_body(body);
                self.out.dedent();
                self.out.writeln("}");
            }
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
                match value {
                    Some(value) => self.out.writeln(&format!("var {name} {ty} = {value}")),
                    None => self.out.writeln(&format!("var {name} {ty}")),
                }
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
                    Some(value) => self.out.writeln(&format!("{name} := {value}")),
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

    fn emit_assign(&mut self, target: &Pattern, value: &Expr) {
        match &target.kind {
            PatternKind::Bind => {
                let name = pattern_name(target);
                let declared = self.locals.contains(&name) || self.globals.contains(&name);
                self.locals.insert(name.clone());
                let value = self.emit_expr(value);
                if declared {
                    self.out.writeln(&format!("{name} = {value}"));
                } else {
                    self.out.writeln(&format!("{name} := {value}"));
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
            _ => rendered,
        }
    }

    fn emit_if(&mut self, condition: &Expr, then_branch: &Block, else_branch: Option<&Else>) {
        let condition = self.emit_condition(condition);
        self.out.writeln(&format!("if {condition} {{"));
        self.out.indent();
        self.emit_block_body(then_branch);
        self.out.dedent();
        self.emit_else(else_branch);
        self.out.writeln("}");
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
        span: Span,
    ) {
        if else_body.is_some() {
            unsupported(self.diagnostics, span, "a loop `else` clause");
        }
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
            self.out.writeln(&format!("for {header} {{"));
            self.out.indent();
            self.emit_block_body(body);
            self.out.dedent();
            self.out.writeln("}");
            return;
        }

        let iterable = self.emit_expr(iterable);
        match pattern.single_binding() {
            Some(name) => {
                let name = name.to_string();
                self.locals.insert(name.clone());
                self.out
                    .writeln(&format!("for _, {name} := range {iterable} {{"));
            }
            None if matches!(pattern.kind, PatternKind::Ignore) => {
                self.out.writeln(&format!("for range {iterable} {{"));
            }
            None => {
                unsupported(
                    self.diagnostics,
                    pattern.span,
                    "a destructuring loop binding",
                );
                self.out.writeln(&format!("for range {iterable} {{"));
            }
        }
        self.out.indent();
        self.emit_block_body(body);
        self.out.dedent();
        self.out.writeln("}");
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
            ExprKind::Literal(literal) => go_literal(literal),
            ExprKind::Path(path) => path
                .segments
                .iter()
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>()
                .join("."),
            ExprKind::Unary { op, operand } => {
                let operand = self.emit_expr(operand);
                match op {
                    UnaryOp::Neg => format!("-{operand}"),
                    UnaryOp::Not => format!("!{operand}"),
                    UnaryOp::BitNot => format!("^{operand}"),
                    _ => {
                        unsupported(self.diagnostics, expr.span, "this unary operator");
                        operand
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                match binary_symbol(*op) {
                    Some(symbol) => format!("({lhs} {symbol} {rhs})"),
                    None => {
                        unsupported(self.diagnostics, expr.span, "this binary operator");
                        format!("({lhs} /* unsupported */ {rhs})")
                    }
                }
            }
            ExprKind::Compare { op, lhs, rhs } => {
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                match comparison_symbol(*op) {
                    Some(symbol) => format!("({lhs} {symbol} {rhs})"),
                    None => {
                        unsupported(self.diagnostics, expr.span, "this comparison");
                        format!("({lhs} /* unsupported */ {rhs})")
                    }
                }
            }
            ExprKind::Logical { op, lhs, rhs } => {
                let lhs = self.emit_expr(lhs);
                let rhs = self.emit_expr(rhs);
                let symbol = match op {
                    LogicalOp::And => "&&",
                    LogicalOp::Or => "||",
                };
                format!("({lhs} {symbol} {rhs})")
            }
            ExprKind::Call {
                callee,
                args,
                named_args,
            } => self.emit_call(callee, args, named_args, expr.span),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
                named_args,
            } => self.emit_method_call(receiver, method, args, named_args, expr.span),
            ExprKind::Index { target, index } => {
                let target = self.emit_expr(target);
                let index = self.emit_expr(index);
                format!("{target}[{index}]")
            }
            ExprKind::Field { target, field } => {
                let target = self.emit_expr(target);
                format!("{target}.{field}")
            }
            ExprKind::List { elements } | ExprKind::Tuple { elements } => {
                self.emit_sequence_literal(elements)
            }
            ExprKind::Lambda { params, body } => self.emit_lambda(params, body, expr.span),
            ExprKind::Conditional { .. } => {
                // Go has no ternary. A faithful expansion needs an IIFE, which
                // is a later slice; reporting is better than an approximation
                // that evaluates both branches.
                unsupported(self.diagnostics, expr.span, "a conditional expression");
                "nil".to_string()
            }
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
                    self.needs_fmt = true;
                    return format!("fmt.Println({})", rendered.join(", "));
                }
                "len" => return format!("len({})", rendered.join(", ")),
                "str" => {
                    self.needs_fmt = true;
                    return format!("fmt.Sprint({})", rendered.join(", "));
                }
                "int" => return format!("int({})", rendered.join(", ")),
                "float" => return format!("float64({})", rendered.join(", ")),
                "range" => {
                    // `range` only has meaning as a loop header, which
                    // `emit_for_in` handles. Reaching here means it was used as
                    // a value.
                    unsupported(self.diagnostics, span, "`range` outside a loop header");
                    return "nil".to_string();
                }
                _ => {}
            }
        }
        let callee = self.emit_expr(callee);
        format!("{callee}({})", rendered.join(", "))
    }

    fn emit_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        named_args: &[NamedArg],
        span: Span,
    ) -> String {
        if !named_args.is_empty() {
            unsupported(self.diagnostics, span, "keyword arguments");
        }
        let receiver = self.emit_expr(receiver);
        let rendered: Vec<String> = args.iter().map(|arg| self.emit_expr(arg)).collect();
        match method {
            // Python's `xs.append(v)` mutates and returns nothing; Go's
            // `append` returns the new slice and must be assigned by the
            // caller. Emitting the call alone would drop the result, so it is
            // reported rather than silently losing the append.
            "append" => {
                unsupported(
                    self.diagnostics,
                    span,
                    "`.append` because Go's `append` returns a new slice",
                );
                format!("append({receiver}, {})", rendered.join(", "))
            }
            _ => {
                unsupported(self.diagnostics, span, describe_method(method));
                format!("{receiver}.{method}({})", rendered.join(", "))
            }
        }
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
        let mut parts: Vec<String> = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            if !segment.is_empty() {
                parts.push(go_string(segment));
            }
            if let Some(argument) = arguments.get(index) {
                self.needs_fmt = true;
                let argument = self.emit_expr(argument);
                parts.push(format!("fmt.Sprint({argument})"));
            }
        }
        if parts.is_empty() {
            return "\"\"".to_string();
        }
        parts.join(" + ")
    }
}

// ------------------------------------------------------------- free helpers

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
        Type::Option(inner) => go_type(inner),
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

/// The Go element type for a sequence literal, inferred from its elements.
///
/// A homogeneous literal gets a concrete slice type; anything mixed or
/// non-literal gets `interface{}`. This is inference over literals only, not a
/// guess about a value the source left untyped.
fn sequence_element_type(elements: &[Expr]) -> String {
    let mut kind: Option<&'static str> = None;
    for element in elements {
        let this = literal_element_type(element);
        match kind {
            None => kind = Some(this),
            Some(existing) if existing == this => {}
            Some(_) => return "interface{}".to_string(),
        }
    }
    kind.unwrap_or("interface{}").to_string()
}

fn literal_element_type(expr: &Expr) -> &'static str {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(_)) => "int",
        ExprKind::Literal(Literal::Float(_)) => "float64",
        ExprKind::Literal(Literal::Str(_)) => "string",
        ExprKind::Literal(Literal::Bytes(_)) => "[]byte",
        ExprKind::Literal(Literal::Bool(_)) => "bool",
        _ => "interface{}",
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
