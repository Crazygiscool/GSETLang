fn dump(node: tree_sitter::Node, source: &str, field: Option<&str>, depth: usize) {
    let indent = "  ".repeat(depth);
    let label = match field {
        Some(name) => format!("{name}: "),
        None => String::new(),
    };
    let mut text = &source[node.byte_range()];
    if text.len() > 30 {
        text = &text[..30];
    }
    println!(
        "{indent}{label}{} named={} {:?}",
        node.kind(),
        node.is_named(),
        text.replace('\n', "\\n")
    );

    // Drive the cursor directly: `children()` holds a mutable borrow for the
    // whole iterator, which rules out calling `field_name()` inside a closure.
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child_field = cursor.field_name();
            dump(cursor.node(), source, child_field, depth + 1);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let path = argv.get(1).expect("usage: dump <file>");
    let source = std::fs::read_to_string(path).expect("read");
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("load python grammar");
    let tree = parser.parse(&source, None).expect("parse");
    dump(tree.root_node(), &source, None, 0);
}
