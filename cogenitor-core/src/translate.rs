// Array of strict keywords (currently in use)
const STRICT_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

// Array of reserved keywords (for future use)
const RESERVED_KEYWORDS: &[&str] = &[
    "abstract", "become", "box", "do", "final", "gen", "macro", "override", "priv", "typeof",
    "unsized", "virtual", "yield",
];

fn capitalize(s: &str) -> String {
    modify_first_char(s, char::to_uppercase)
}

fn decapitalize(s: &str) -> String {
    modify_first_char(s, char::to_lowercase)
}

fn modify_first_char<F, R>(s: &str, m: F) -> String
where
    F: FnOnce(char) -> R,
    R: Iterator<Item = char>,
{
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => m(f).collect::<String>() + c.as_str(),
    }
}
pub(crate) fn schema_to_rust_typename(schema_name: &str) -> String {
    // for now, all we do is clone..
    avoid_reserved(&capitalize(schema_name))
}

pub(crate) fn property_to_rust_fieldname(property_name: &str) -> String {
    avoid_reserved(&decapitalize(property_name))
}

fn avoid_reserved(s: &str) -> String {
    if STRICT_KEYWORDS
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .any(|e| (*e).eq(s))
    {
        s.to_string() + "_"
    } else {
        s.to_string()
    }
}
