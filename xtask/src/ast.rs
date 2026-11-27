//! Streaming reader for `clang -Xclang -ast-dump=json`.
//!
//! Clang omits `file` (and `line`) from a location when it equals the previously
//! *printed* location, in document order, across every `loc`/`range` object at
//! any depth. A reader that only looks at top-level nodes therefore attributes
//! decls to the wrong file. This reader tracks the current file and line through
//! every location in order, without materialising the multi-hundred-megabyte tree.

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc as Rc;

#[derive(Clone, Debug, Default)]
pub struct Loc {
    pub file: Option<Rc<str>>,
    pub line: u32,
    pub offset: u64,
    pub tok_len: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Range {
    pub begin: Loc,
    pub end: Loc,
}

#[derive(Debug, Default)]
pub struct FnFact {
    pub name: String,
    pub file: String,
    pub storage_static: bool,
    pub range: Range,
    pub qual_type: String,
    pub params: Vec<String>,
    pub has_body: bool,
    /// Every function referenced (called or address-taken) in the body.
    pub refs: BTreeSet<String>,
    pub local_statics: Vec<VarFact>,
}

#[derive(Clone, Debug, Default)]
pub struct VarFact {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub qual_type: String,
    pub storage_class: Option<String>,
}

#[derive(Debug, Default)]
pub struct TuFacts {
    /// All top-level function decls/defs, in order.
    pub functions: Vec<FnFact>,
    /// Top-level variable decls/defs.
    pub vars: Vec<VarFact>,
    /// Names of record/enum/typedef decls by defining file.
    pub type_files: BTreeMap<String, BTreeSet<String>>,
    /// Subset of `type_files` that are enums or typedefs of enums.
    pub enum_types: BTreeMap<String, BTreeSet<String>>,
}

struct FnCtx {
    depth: usize,
    params: Vec<String>,
    has_body: bool,
    refs: BTreeSet<String>,
    local_statics: Vec<VarFact>,
}

pub struct Walker {
    cur_file: Option<Rc<str>>,
    cur_line: u32,
    depth: usize,
    fn_stack: Vec<FnCtx>,
    pub facts: TuFacts,
}

impl Walker {
    pub fn new() -> Self {
        Walker {
            cur_file: None,
            cur_line: 0,
            depth: 0,
            fn_stack: Vec::new(),
            facts: TuFacts::default(),
        }
    }

    fn file_str(loc: &Loc) -> String {
        loc.file.as_deref().unwrap_or("").to_owned()
    }
}

/// Parses one translation-unit dump.
pub fn walk<R: std::io::Read>(reader: R) -> anyhow::Result<TuFacts> {
    let mut de =
        serde_json::Deserializer::from_reader(std::io::BufReader::with_capacity(1 << 20, reader));
    de.disable_recursion_limit();
    let mut walker = Walker::new();
    NodeSeed { w: &mut walker }.deserialize(&mut de)?;
    de.end()?;
    Ok(walker.facts)
}

struct NodeSeed<'a> {
    w: &'a mut Walker,
}

struct NodeSeqSeed<'a> {
    w: &'a mut Walker,
}

struct LocSeed<'a> {
    w: &'a mut Walker,
}

struct RangeSeed<'a> {
    w: &'a mut Walker,
}

#[derive(serde::Deserialize)]
struct TypeInfo {
    #[serde(rename = "qualType")]
    qual_type: String,
}

#[derive(serde::Deserialize)]
struct RefDecl {
    kind: Option<String>,
    name: Option<String>,
}

impl<'de> DeserializeSeed<'de> for NodeSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for NodeSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("clang AST node")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let w = self.w;
        w.depth += 1;
        let depth = w.depth;
        let mut kind = String::new();
        let mut name: Option<String> = None;
        let mut loc = Loc::default();
        let mut range = Range::default();
        let mut storage: Option<String> = None;
        let mut qual_type = String::new();
        let mut referenced: Option<RefDecl> = None;
        let mut pushed_fn = false;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "kind" => kind = map.next_value()?,
                "name" => name = Some(map.next_value()?),
                "loc" => loc = map.next_value_seed(LocSeed { w: &mut *w })?,
                "range" => range = map.next_value_seed(RangeSeed { w: &mut *w })?,
                "storageClass" => storage = Some(map.next_value()?),
                "type" => qual_type = map.next_value::<TypeInfo>()?.qual_type,
                "referencedDecl" => referenced = Some(map.next_value()?),
                "inner" | "array_filler" => {
                    if kind == "FunctionDecl" && !pushed_fn {
                        w.fn_stack.push(FnCtx {
                            depth,
                            params: Vec::new(),
                            has_body: false,
                            refs: BTreeSet::new(),
                            local_statics: Vec::new(),
                        });
                        pushed_fn = true;
                    }
                    map.next_value_seed(NodeSeqSeed { w: &mut *w })?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        let decl_loc = loc.clone();
        match kind.as_str() {
            "FunctionDecl" => {
                let ctx = if pushed_fn { w.fn_stack.pop() } else { None };
                if depth == 2 {
                    let (params, has_body, refs, local_statics) = match ctx {
                        Some(c) => (c.params, c.has_body, c.refs, c.local_statics),
                        None => (Vec::new(), false, BTreeSet::new(), Vec::new()),
                    };
                    w.facts.functions.push(FnFact {
                        name: name.unwrap_or_default(),
                        file: Walker::file_str(&decl_loc),
                        storage_static: storage.as_deref() == Some("static"),
                        range,
                        qual_type,
                        params,
                        has_body,
                        refs,
                        local_statics,
                    });
                }
            }
            "ParmVarDecl" => {
                if let Some(top) = w.fn_stack.last_mut()
                    && top.depth + 1 == depth
                {
                    top.params.push(qual_type);
                }
            }
            "CompoundStmt" => {
                if let Some(top) = w.fn_stack.last_mut()
                    && top.depth + 1 == depth
                {
                    top.has_body = true;
                }
            }
            "VarDecl" => {
                let fact = VarFact {
                    name: name.unwrap_or_default(),
                    file: Walker::file_str(&decl_loc),
                    line: decl_loc.line,
                    qual_type,
                    storage_class: storage,
                };
                if depth == 2 {
                    w.facts.vars.push(fact);
                } else if fact.storage_class.as_deref() == Some("static")
                    && let Some(top) = w.fn_stack.last_mut()
                {
                    top.local_statics.push(fact);
                }
            }
            "DeclRefExpr" => {
                if let Some(r) = referenced
                    && r.kind.as_deref() == Some("FunctionDecl")
                    && let Some(n) = r.name
                {
                    // Attribute to every enclosing function definition.
                    for ctx in &mut w.fn_stack {
                        ctx.refs.insert(n.clone());
                    }
                }
            }
            "RecordDecl" | "EnumDecl" | "TypedefDecl" => {
                if depth == 2
                    && let Some(n) = name
                {
                    let file = Walker::file_str(&decl_loc);
                    if kind == "EnumDecl" || qual_type.starts_with("enum ") {
                        w.facts
                            .enum_types
                            .entry(file.clone())
                            .or_default()
                            .insert(n.clone());
                    }
                    w.facts.type_files.entry(file).or_default().insert(n);
                }
            }
            _ => {}
        }
        w.depth -= 1;
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for NodeSeqSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for NodeSeqSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("array of clang AST nodes")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let w = self.w;
        while seq.next_element_seed(NodeSeed { w: &mut *w })?.is_some() {}
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for LocSeed<'_> {
    type Value = Loc;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Loc, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for LocSeed<'_> {
    type Value = Loc;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("clang source location")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Loc, A::Error> {
        let w = self.w;
        let mut offset = None;
        let mut tok_len = 0;
        let mut expansion: Option<Loc> = None;
        let mut bare = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "offset" => {
                    offset = Some(map.next_value()?);
                    bare = true;
                }
                "file" => {
                    let f: String = map.next_value()?;
                    w.cur_file = Some(Rc::from(f));
                }
                "line" => w.cur_line = map.next_value()?,
                "tokLen" => tok_len = map.next_value()?,
                "spellingLoc" => {
                    map.next_value_seed(LocSeed { w: &mut *w })?;
                }
                "expansionLoc" => expansion = Some(map.next_value_seed(LocSeed { w: &mut *w })?),
                // `includedFrom.file` does not update clang's last-printed file.
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        if let Some(exp) = expansion {
            return Ok(exp);
        }
        if bare {
            return Ok(Loc {
                file: w.cur_file.clone(),
                line: w.cur_line,
                offset: offset.unwrap_or(0),
                tok_len,
            });
        }
        Ok(Loc::default())
    }
}

impl<'de> DeserializeSeed<'de> for RangeSeed<'_> {
    type Value = Range;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Range, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for RangeSeed<'_> {
    type Value = Range;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("clang source range")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Range, A::Error> {
        let w = self.w;
        let mut r = Range::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "begin" => r.begin = map.next_value_seed(LocSeed { w: &mut *w })?,
                "end" => r.end = map.next_value_seed(LocSeed { w: &mut *w })?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(r)
    }
}
