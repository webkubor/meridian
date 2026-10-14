//! The frontend captures real wrapper calls in `fixtures/ipc-contract.json`.
//! Check those payloads against Rust source, including serde defaults, nested
//! structs and enum tags. Unknown syntax fails closed: silently skipping an
//! input type would recreate the hole this guard is meant to close.

use regex::Regex;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    wrapper: String,
    case: String,
    command: String,
    payload: Value,
}

#[derive(Clone, Debug)]
struct Field {
    name: String,
    wire: String,
    ty: String,
    optional: bool,
}

#[derive(Clone, Debug)]
enum Shape {
    Named(Vec<Field>),
    Tuple(Vec<String>),
    Unit,
}

#[derive(Clone, Debug)]
struct Definition {
    module: String,
    name: String,
    attributes: String,
    shape: Shape,
    variants: Option<BTreeMap<String, Shape>>,
}

#[derive(Default)]
struct Catalog {
    definitions: BTreeMap<String, Definition>,
    imports: BTreeMap<String, BTreeMap<String, String>>,
    commands: BTreeMap<String, (String, Vec<Field>)>,
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).expect("source directory is readable") {
        let path = entry.expect("source entry is readable").path();
        if path.is_dir() {
            files.extend(files_under(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Preserve strings (serde attributes need them) and UTF-8, blank comments.
fn strip_comments(source: &str) -> String {
    let mut result = source.as_bytes().to_vec();
    let mut i = 0;
    while i < result.len() {
        if result[i..].starts_with(b"//") {
            while i < result.len() && result[i] != b'\n' {
                result[i] = b' ';
                i += 1;
            }
        } else if result[i..].starts_with(b"/*") {
            let mut depth = 1;
            result[i..i + 2].fill(b' ');
            i += 2;
            while depth > 0 {
                assert!(i < result.len(), "unclosed block comment");
                if result[i..].starts_with(b"/*") {
                    depth += 1;
                    result[i..i + 2].fill(b' ');
                    i += 2;
                } else if result[i..].starts_with(b"*/") {
                    depth -= 1;
                    result[i..i + 2].fill(b' ');
                    i += 2;
                } else {
                    if result[i] != b'\n' {
                        result[i] = b' ';
                    }
                    i += 1;
                }
            }
        } else if result[i] == b'"' {
            i += 1;
            while i < result.len() {
                match result[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
        } else {
            i += 1;
        }
    }
    String::from_utf8(result).expect("comment removal preserves UTF-8")
}

fn close_delimiter(text: &str, open: usize, left: u8, right: u8) -> usize {
    let bytes = text.as_bytes();
    assert_eq!(bytes[open], left);
    let mut depth = 0;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => break,
                        _ => i += 1,
                    }
                }
            }
            byte if byte == left => depth += 1,
            byte if byte == right => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unclosed delimiter in {text:?}");
}

fn split_top_level(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let (mut i, mut start) = (0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'[' => i = close_delimiter(text, i, b'[', b']'),
            b'{' => i = close_delimiter(text, i, b'{', b'}'),
            b'(' => i = close_delimiter(text, i, b'(', b')'),
            b'<' => i = close_delimiter(text, i, b'<', b'>'),
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b',' => {
                parts.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(text[start..].trim());
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

fn leading_attributes(mut text: &str) -> (String, &str) {
    let mut attributes = String::new();
    text = text.trim();
    while text.starts_with("#[") {
        let end = close_delimiter(text, 1, b'[', b']');
        attributes.push_str(&text[..=end]);
        text = text[end + 1..].trim_start();
    }
    (attributes, text)
}

fn serde_items(attributes: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut rest = attributes;
    while let Some((_, after)) = rest.split_once("#[serde(") {
        let end = after.find(")]").expect("serde attribute closes");
        result.extend(
            split_top_level(&after[..end])
                .into_iter()
                .map(str::to_owned),
        );
        rest = &after[end + 2..];
    }
    result
}

fn serde_value(attributes: &str, key: &str) -> Option<String> {
    serde_items(attributes).into_iter().find_map(|item| {
        let (name, value) = item.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"').to_string())
    })
}

fn serde_flag(attributes: &str, key: &str) -> bool {
    serde_items(attributes).iter().any(|item| {
        item == key
            || item
                .split_once('=')
                .is_some_and(|(name, _)| name.trim() == key)
    })
}

// Serde deliberately uses different case rules for fields and enum variants.
fn rename(name: &str, rule: Option<&str>, variant: bool) -> String {
    let pascal = || {
        name.split('_')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_ascii_uppercase().to_string() + chars.as_str()
                })
            })
            .collect::<String>()
    };
    let snake = || {
        name.chars()
            .enumerate()
            .flat_map(|(i, c)| {
                if i > 0 && c.is_ascii_uppercase() {
                    vec!['_', c.to_ascii_lowercase()]
                } else {
                    vec![c.to_ascii_lowercase()]
                }
            })
            .collect::<String>()
    };
    match rule.unwrap_or("") {
        "" => name.to_string(),
        "camelCase" => {
            let value = if variant { name.to_string() } else { pascal() };
            value[..1].to_ascii_lowercase() + &value[1..]
        }
        "PascalCase" => {
            if variant {
                name.to_string()
            } else {
                pascal()
            }
        }
        "snake_case" => {
            if variant {
                snake()
            } else {
                name.to_string()
            }
        }
        "SCREAMING_SNAKE_CASE" => {
            if variant { snake() } else { name.to_string() }.to_ascii_uppercase()
        }
        "kebab-case" => if variant { snake() } else { name.to_string() }.replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => if variant { snake() } else { name.to_string() }
            .replace('_', "-")
            .to_ascii_uppercase(),
        "lowercase" => {
            if variant {
                name.to_ascii_lowercase()
            } else {
                name.to_string()
            }
        }
        "UPPERCASE" => name.to_ascii_uppercase(),
        other => panic!("unsupported serde rename rule: {other}"),
    }
}

fn fields(body: &str, rule: Option<&str>, default: bool) -> Vec<Field> {
    split_top_level(body)
        .into_iter()
        .map(|chunk| {
            let (attributes, text) = leading_attributes(chunk);
            for unsupported in [
                "flatten",
                "skip",
                "skip_deserializing",
                "alias",
                "deserialize_with",
            ] {
                assert!(
                    !serde_flag(&attributes, unsupported),
                    "extend IPC scanner for #[serde({unsupported})]"
                );
            }
            let text = text
                .strip_prefix("pub ")
                .or_else(|| text.strip_prefix("pub(crate) "))
                .unwrap_or(text);
            let (name, ty) = text
                .split_once(':')
                .unwrap_or_else(|| panic!("unparsed field: {chunk}"));
            let name = name.trim().trim_start_matches("r#").to_string();
            let ty = ty.split_whitespace().collect::<String>();
            Field {
                wire: serde_value(&attributes, "rename")
                    .unwrap_or_else(|| rename(&name, rule, false)),
                name,
                optional: default
                    || serde_flag(&attributes, "default")
                    || inner(&ty, "Option").is_some(),
                ty,
            }
        })
        .collect()
}

fn shape(text: &str, rule: Option<&str>, default: bool) -> Shape {
    let text = text.trim();
    match text.as_bytes().first() {
        Some(b'{') => Shape::Named(fields(
            &text[1..close_delimiter(text, 0, b'{', b'}')],
            rule,
            default,
        )),
        Some(b'(') => Shape::Tuple(
            split_top_level(&text[1..close_delimiter(text, 0, b'(', b')')])
                .iter()
                .map(|ty| ty.trim_start_matches("pub ").trim().to_string())
                .collect(),
        ),
        Some(b';') | None => Shape::Unit,
        other => panic!("unsupported input declaration {other:?}: {text}"),
    }
}

fn absolute_path(module: &str, path: &str) -> String {
    let mut parts = module.split("::").map(str::to_owned).collect::<Vec<_>>();
    let mut path = path;
    if let Some(rest) = path.strip_prefix("crate::") {
        return rest.to_string();
    }
    if let Some(rest) = path.strip_prefix("self::") {
        return format!("{module}::{rest}");
    }
    while let Some(rest) = path.strip_prefix("super::") {
        parts.pop();
        path = rest;
    }
    parts.push(path.to_string());
    parts.join("::")
}

fn parse_imports(module: &str, source: &str) -> BTreeMap<String, String> {
    fn add(module: &str, prefix: &str, tree: &str, result: &mut BTreeMap<String, String>) {
        if let Some(open) = tree.find('{') {
            let parent = format!("{prefix}{}", tree[..open].trim());
            let close = close_delimiter(tree, open, b'{', b'}');
            for child in split_top_level(&tree[open + 1..close]) {
                add(module, &parent, child, result);
            }
        } else {
            let tree = tree.trim();
            if tree == "*" || tree == "self" {
                return;
            }
            let (path, alias) = tree
                .split_once(" as ")
                .map_or((tree, None), |(p, a)| (p, Some(a.trim())));
            let path = format!("{prefix}{path}");
            let name = alias.unwrap_or_else(|| path.rsplit("::").next().unwrap());
            result.insert(name.to_string(), absolute_path(module, &path));
        }
    }
    let mut result = BTreeMap::new();
    for captures in Regex::new(r"(?m)^(?:pub )?use\s+([^;]+);")
        .unwrap()
        .captures_iter(source)
    {
        add(module, "", &captures[1], &mut result);
    }
    result
}

impl Catalog {
    fn load() -> Self {
        let mut catalog = Self::default();
        for path in files_under(&src_dir()) {
            let relative = path.strip_prefix(src_dir()).unwrap().with_extension("");
            let module = relative
                .iter()
                .map(|part| part.to_str().unwrap())
                .filter(|part| *part != "mod")
                .collect::<Vec<_>>()
                .join("::");
            catalog.add_source(&module, &fs::read_to_string(path).expect("source readable"));
        }
        catalog
    }

    fn add_source(&mut self, module: &str, raw: &str) {
        let source = strip_comments(raw);
        self.imports
            .insert(module.to_string(), parse_imports(module, &source));
        let declarations = Regex::new(
            r"(?m)^((?:#\[[^\n]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?(struct|enum)\s+(\w+)\s*([\{(;])",
        )
        .unwrap();
        for captures in declarations.captures_iter(&source) {
            let attributes = captures[1].to_string();
            let name = captures[3].to_string();
            let start = captures.get(4).unwrap().start();
            let rule = serde_value(&attributes, "rename_all");
            let default = serde_flag(&attributes, "default");
            // Only serde input vocabulary is relevant. Plain engine structures
            // can contain function pointers, lifetimes and other non-wire types.
            if !attributes.contains("Deserialize") {
                continue;
            }
            let (shape, variants) = if &captures[2] == "struct" {
                (shape(&source[start..], rule.as_deref(), default), None)
            } else {
                let end = close_delimiter(&source, start, b'{', b'}');
                let mut variants = BTreeMap::new();
                for chunk in split_top_level(&source[start + 1..end]) {
                    let (attrs, declaration) = leading_attributes(chunk);
                    let end_name = declaration
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .unwrap_or(declaration.len());
                    let name = &declaration[..end_name];
                    let wire = serde_value(&attrs, "rename")
                        .unwrap_or_else(|| rename(name, rule.as_deref(), true));
                    let field_rule = serde_value(&attrs, "rename_all")
                        .or_else(|| serde_value(&attributes, "rename_all_fields"));
                    variants.insert(
                        wire,
                        shape(&declaration[end_name..], field_rule.as_deref(), false),
                    );
                }
                (Shape::Unit, Some(variants))
            };
            let key = format!("{module}::{name}");
            assert!(self
                .definitions
                .insert(
                    key,
                    Definition {
                        module: module.to_string(),
                        name,
                        attributes,
                        shape,
                        variants
                    }
                )
                .is_none());
        }
        if module.starts_with("commands::") {
            self.add_commands(module, &source);
        }
    }

    fn add_commands(&mut self, module: &str, source: &str) {
        let functions = Regex::new(
            r"#\[tauri::command(?:\(([^]]*)\))?\]\s*pub\s+(?:async\s+)?fn\s+(\w+)(?:<[^>]+>)?\s*\(",
        )
        .unwrap();
        for captures in functions.captures_iter(source) {
            let name = captures[2].to_string();
            let open = captures.get(0).unwrap().end() - 1;
            let close = close_delimiter(source, open, b'(', b')');
            let rule = if captures
                .get(1)
                .is_some_and(|options| options.as_str().contains("snake_case"))
            {
                "snake_case"
            } else {
                "camelCase"
            };
            let args = fields(&source[open + 1..close], Some(rule), false)
                .into_iter()
                .filter(|field| {
                    !matches!(
                        base(&field.ty),
                        "State" | "AppHandle" | "Window" | "WebviewWindow" | "Webview"
                    )
                })
                .collect();
            assert!(self
                .commands
                .insert(name, (module.to_string(), args))
                .is_none());
        }
        let macros = Regex::new(r"(?m)^tauri_command!\s*\(\s*(\w+)\s*,\s*([^,]+),").unwrap();
        for captures in macros.captures_iter(source) {
            let fields = vec![Field {
                name: "args".into(),
                wire: "args".into(),
                ty: captures[2].trim().into(),
                optional: false,
            }];
            assert!(self
                .commands
                .insert(captures[1].into(), (module.into(), fields))
                .is_none());
        }
    }

    fn resolve(&self, module: &str, ty: &str) -> Option<&Definition> {
        let local = format!("{module}::{ty}");
        let key = if self.definitions.contains_key(&local) {
            local
        } else if ty.contains("::") {
            absolute_path(module, ty)
        } else {
            self.imports.get(module)?.get(ty)?.clone()
        };
        self.definitions.get(&key)
    }
}

fn base(ty: &str) -> &str {
    ty.split('<').next().unwrap().rsplit("::").next().unwrap()
}

fn inner<'a>(ty: &'a str, outer: &str) -> Option<&'a str> {
    if base(ty) != outer {
        return None;
    }
    ty.split_once('<')?.1.strip_suffix('>')
}

#[derive(Default)]
struct Check {
    errors: Vec<String>,
    optional: BTreeSet<String>,
    supplied: BTreeSet<String>,
}

impl Check {
    fn object(
        &mut self,
        catalog: &Catalog,
        module: &str,
        owner: &str,
        fields: &[Field],
        path: &str,
        value: &Value,
    ) {
        let Some(object) = value.as_object() else {
            self.errors
                .push(format!("{path}: expected object for {owner}, got {value}"));
            return;
        };
        for field in fields {
            let child_path = format!("{path}.{}", field.wire);
            if field.optional {
                self.optional.insert(child_path.clone());
            }
            if let Some(child) = object.get(&field.wire) {
                self.supplied.insert(child_path.clone());
                self.value(catalog, module, &field.ty, &child_path, child);
            } else if !field.optional {
                self.errors.push(format!(
                    "{child_path}: missing required {owner}.{}",
                    field.name
                ));
            }
        }
        for key in object.keys() {
            if !fields.iter().any(|field| field.wire == *key) {
                self.errors.push(format!(
                    "{path}.{key}: unknown key for {owner}; expected {}",
                    fields
                        .iter()
                        .map(|field| field.wire.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }

    fn value(&mut self, catalog: &Catalog, module: &str, ty: &str, path: &str, value: &Value) {
        if let Some(inner) = inner(ty, "Option") {
            if !value.is_null() {
                self.value(catalog, module, inner, path, value);
            }
            return;
        }
        if let Some(inner) = inner(ty, "Vec") {
            if let Some(items) = value.as_array() {
                for item in items {
                    self.value(catalog, module, inner, &format!("{path}[]"), item);
                }
            } else {
                self.errors
                    .push(format!("{path}: expected array for {ty}, got {value}"));
            }
            return;
        }
        let valid_scalar = match ty {
            "String" | "str" => Some(value.is_string()),
            "bool" => Some(value.is_boolean()),
            "u8" | "u16" | "u32" | "u64" | "usize" => Some(value.as_u64().is_some()),
            "i8" | "i16" | "i32" | "i64" | "isize" => Some(value.as_i64().is_some()),
            "f32" | "f64" => Some(value.is_number()),
            _ => None,
        };
        if let Some(valid) = valid_scalar {
            if !valid {
                self.errors
                    .push(format!("{path}: expected {ty}, got {value}"));
            }
            return;
        }
        // The fixture carries a deterministic marker for a live Tauri Channel.
        if base(ty) == "Channel" {
            if value != &serde_json::json!({"$channel": true}) {
                self.errors
                    .push(format!("{path}: expected a captured Channel, got {value}"));
            }
            return;
        }
        let Some(definition) = catalog.resolve(module, ty) else {
            self.errors.push(format!("{path}: scanner cannot resolve {module}::{ty}; extend it rather than skipping the type"));
            return;
        };
        if let Some(variants) = &definition.variants {
            self.enum_value(catalog, definition, variants, path, value);
        } else {
            self.shape(catalog, definition, &definition.shape, path, value);
        }
    }

    fn shape(
        &mut self,
        catalog: &Catalog,
        definition: &Definition,
        shape: &Shape,
        path: &str,
        value: &Value,
    ) {
        match shape {
            Shape::Named(fields) => self.object(
                catalog,
                &definition.module,
                &definition.name,
                fields,
                path,
                value,
            ),
            Shape::Tuple(types) if types.len() == 1 => {
                self.value(catalog, &definition.module, &types[0], path, value)
            }
            Shape::Tuple(types) => {
                if let Some(values) = value
                    .as_array()
                    .filter(|values| values.len() == types.len())
                {
                    for (index, (ty, value)) in types.iter().zip(values).enumerate() {
                        self.value(
                            catalog,
                            &definition.module,
                            ty,
                            &format!("{path}[{index}]"),
                            value,
                        );
                    }
                } else {
                    self.errors.push(format!(
                        "{path}: expected {} tuple fields, got {value}",
                        types.len()
                    ));
                }
            }
            Shape::Unit => {
                if !value.is_null() {
                    self.errors
                        .push(format!("{path}: expected unit value, got {value}"));
                }
            }
        }
    }

    fn enum_value(
        &mut self,
        catalog: &Catalog,
        definition: &Definition,
        variants: &BTreeMap<String, Shape>,
        path: &str,
        value: &Value,
    ) {
        if serde_flag(&definition.attributes, "untagged") {
            for shape in variants.values() {
                let mut trial = Self::default();
                trial.shape(catalog, definition, shape, path, value);
                if trial.errors.is_empty() {
                    self.optional.extend(trial.optional);
                    self.supplied.extend(trial.supplied);
                    return;
                }
            }
            self.errors.push(format!(
                "{path}: no untagged {} variant accepts {value}",
                definition.name
            ));
            return;
        }
        let tag = serde_value(&definition.attributes, "tag");
        let content = serde_value(&definition.attributes, "content");
        let variant = match &tag {
            Some(tag) => value.get(tag).and_then(Value::as_str),
            None => value.as_str().or_else(|| {
                value
                    .as_object()
                    .filter(|object| object.len() == 1)
                    .and_then(|object| object.keys().next().map(String::as_str))
            }),
        };
        let Some((variant, shape)) =
            variant.and_then(|variant| variants.get(variant).map(|shape| (variant, shape)))
        else {
            self.errors.push(format!(
                "{path}: {value} is not a {} variant; expected {}",
                definition.name,
                variants.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
            return;
        };
        match (&tag, &content, shape) {
            (Some(tag), Some(content), _) => {
                let object = value.as_object().expect("tag was read from an object");
                for key in object.keys().filter(|key| *key != tag && *key != content) {
                    self.errors.push(format!(
                        "{path}.{key}: unknown {} enum key",
                        definition.name
                    ));
                }
                if matches!(shape, Shape::Unit) {
                    if object.get(content).is_some_and(|value| !value.is_null()) {
                        self.errors.push(format!(
                            "{path}.{content}: unit variant {variant} has no content"
                        ));
                    }
                } else if let Some(payload) = object.get(content) {
                    self.shape(
                        catalog,
                        definition,
                        shape,
                        &format!("{path}.{content}"),
                        payload,
                    );
                } else {
                    self.errors.push(format!(
                        "{path}.{content}: missing {}::{variant} content",
                        definition.name
                    ));
                }
            }
            (Some(tag), None, _) => {
                let mut object = value.as_object().unwrap().clone();
                object.remove(tag);
                if matches!(shape, Shape::Unit) {
                    if !object.is_empty() {
                        self.errors.push(format!(
                            "{path}: unknown keys for unit {}::{variant}: {object:?}",
                            definition.name
                        ));
                    }
                } else {
                    self.shape(catalog, definition, shape, path, &Value::Object(object));
                }
            }
            (None, None, Shape::Unit) if value.is_string() => {}
            (None, None, _) => {
                if let Some(payload) = value.get(variant) {
                    self.shape(
                        catalog,
                        definition,
                        shape,
                        &format!("{path}.{variant}"),
                        payload,
                    );
                } else {
                    self.errors
                        .push(format!("{path}: missing external {variant} payload"));
                }
            }
            _ => panic!("serde content requires a tag"),
        }
    }
}

fn fixture() -> Vec<Sample> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ipc-contract.json");
    let samples: Vec<Sample> =
        serde_json::from_str(&fs::read_to_string(path).expect("fixture is readable"))
            .expect("fixture is valid");
    assert!(!samples.is_empty(), "fixture has no bridge samples");
    samples
}

fn registered() -> BTreeSet<String> {
    let source = strip_comments(&fs::read_to_string(src_dir().join("lib.rs")).unwrap());
    let body = source
        .split_once("generate_handler![")
        .unwrap()
        .1
        .split_once("])")
        .unwrap()
        .0;
    Regex::new(r"commands::(\w+)")
        .unwrap()
        .captures_iter(body)
        .map(|captures| captures[1].to_string())
        .collect()
}

#[test]
fn fixture_commands_and_scanned_commands_are_registered() {
    let catalog = Catalog::load();
    let registered = registered();
    assert_eq!(
        catalog.commands.keys().cloned().collect::<BTreeSet<_>>(),
        registered,
        "scanner must account for every registered Rust command"
    );
    for sample in fixture() {
        assert!(
            registered.contains(&sample.command),
            "{} ({}) calls unregistered {}",
            sample.wrapper,
            sample.case,
            sample.command
        );
    }
}

#[test]
fn typescript_ipc_payloads_match_rust_args_and_enums() {
    let catalog = Catalog::load();
    let mut failures = Vec::new();
    let mut optional = BTreeSet::new();
    let mut supplied = BTreeSet::new();
    for sample in fixture() {
        let (module, fields) = catalog
            .commands
            .get(&sample.command)
            .unwrap_or_else(|| panic!("{} has no scanned command", sample.command));
        let payload = if sample.payload.is_null() {
            Value::Object(Map::new())
        } else {
            sample.payload
        };
        let mut check = Check::default();
        check.object(
            &catalog,
            module,
            &sample.command,
            fields,
            &sample.command,
            &payload,
        );
        optional.extend(check.optional);
        supplied.extend(check.supplied);
        failures.extend(
            check
                .errors
                .into_iter()
                .map(|error| format!("{} [{}]: {error}", sample.wrapper, sample.case)),
        );
    }
    let uncovered = optional.difference(&supplied).cloned().collect::<Vec<_>>();
    println!("IPC coverage note: {} optional/defaulted Rust field(s) never sent by these TS samples:\n  {}", uncovered.len(), uncovered.join("\n  "));
    assert!(
        failures.is_empty(),
        "IPC contract mismatch(es) ({}):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn scanner_agrees_with_real_remote_command_deserialization() {
    // Two unrelated RemoteFetchMode types exist. Confirm the scanner follows
    // the command's import and that enum spelling is real serde behaviour.
    let catalog = Catalog::load();
    for sample in fixture()
        .into_iter()
        .filter(|sample| matches!(sample.command.as_str(), "remote_fetch" | "remote_pull"))
    {
        let args = sample
            .payload
            .get("args")
            .expect("remote arguments")
            .clone();
        let serde_accepts = match sample.command.as_str() {
            "remote_fetch" => {
                serde_json::from_value::<meridian::commands::RemoteFetch>(args).is_ok()
            }
            "remote_pull" => serde_json::from_value::<meridian::commands::RemotePull>(args).is_ok(),
            _ => unreachable!(),
        };
        let (module, fields) = &catalog.commands[&sample.command];
        let mut check = Check::default();
        check.object(
            &catalog,
            module,
            &sample.command,
            fields,
            &sample.command,
            &sample.payload,
        );
        assert_eq!(
            check.errors.is_empty(),
            serde_accepts,
            "{} [{}]: {:#?}",
            sample.wrapper,
            sample.case,
            check.errors
        );
    }
}

#[test]
fn scanner_detects_unknown_nested_keys_missing_fields_and_enum_values() {
    let mut catalog = Catalog::default();
    catalog.add_source(
        "domain::demo",
        r#"
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Id(String);
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode { #[default] FastForwardOnly, Merge }
#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options { pub keep_index: bool, pub mode: Mode }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub id: Id,
    pub remote_branch: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub maybe: Option<String>,
    #[serde(default)]
    pub opts: Options,
}
"#,
    );
    let mut good = Check::default();
    good.value(
        &catalog,
        "domain::demo",
        "Args",
        "args",
        &serde_json::json!({"id": "repo", "remoteBranch": "main", "opts": {}}),
    );
    assert!(good.errors.is_empty(), "{:#?}", good.errors);
    let mut bad = Check::default();
    bad.value(&catalog, "domain::demo", "Args", "args", &serde_json::json!({"id": "repo", "remote_branch": "main", "opts": {"keep_index": true, "mode": "ffOnly"}}));
    assert_eq!(bad.errors.len(), 4, "{:#?}", bad.errors);
    assert!(bad
        .errors
        .iter()
        .any(|error| error.contains("missing required Args.remote_branch")));
    assert!(bad
        .errors
        .iter()
        .any(|error| error.contains("args.remote_branch: unknown key")));
    assert!(bad
        .errors
        .iter()
        .any(|error| error.contains("args.opts.keep_index: unknown key")));
    assert!(bad
        .errors
        .iter()
        .any(|error| error.contains("fastForwardOnly, merge")));
}

#[test]
fn scanner_resolves_imported_names_instead_of_same_named_unrelated_types() {
    let mut catalog = Catalog::default();
    catalog.add_source(
        "domain::config",
        "#[derive(Deserialize)]\npub enum Mode { All }",
    );
    catalog.add_source(
        "engine::remote",
        "#[derive(Deserialize)]\npub struct Mode { pub prune: bool }",
    );
    catalog.add_source(
        "commands::remote",
        r#"
use crate::engine::remote::{self, Mode};
#[derive(Deserialize)]
pub struct Args { pub mode: Mode }
#[tauri::command]
pub async fn run(app: AppHandle, args: Args) {}
"#,
    );
    let mut check = Check::default();
    check.value(
        &catalog,
        "commands::remote",
        "Args",
        "args",
        &serde_json::json!({"mode": "All"}),
    );
    assert_eq!(check.errors.len(), 1);
    assert!(check.errors[0].contains("expected object for Mode"));
}

#[test]
fn scanner_checks_adjacent_and_internal_enum_content_and_arrays() {
    let mut catalog = Catalog::default();
    catalog.add_source(
        "domain::demo",
        r#"
#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Revision { #[default] Head, Branch(String) }
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Sign { None, Gpg { key: Option<String> } }
#[derive(Deserialize)]
pub struct Args { pub revisions: Vec<Revision>, pub sign: Option<Sign> }
"#,
    );
    let mut good = Check::default();
    good.value(&catalog, "domain::demo", "Args", "args", &serde_json::json!({"revisions": [{"kind": "head"}, {"kind": "branch", "value": "main"}], "sign": {"kind": "gpg"}}));
    assert!(good.errors.is_empty(), "{:#?}", good.errors);
    let mut bad = Check::default();
    bad.value(&catalog, "domain::demo", "Args", "args", &serde_json::json!({"revisions": [{"kind": "branch"}], "sign": {"kind": "gpg", "wrong": true}}));
    assert_eq!(bad.errors.len(), 2, "{:#?}", bad.errors);
}
