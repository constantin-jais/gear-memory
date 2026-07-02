//! Dev-only CodeMap fixture generator: maps the gear-memory repository
//! itself into `tests/fixtures/gear-memory-repo-codemap.valid.json`.
//!
//! Guardrail (decision log 2026-07-02): this is test-side tooling under the
//! "Wrench parses, Gear stores" boundary — it never ships, and the product
//! keeps zero parsing capability. Output is deterministic (fixed timestamps
//! and revision, sorted symbols/edges, content hashes derived from file
//! contents) so regeneration only produces a diff when the source changed.
//!
//! Regenerate with:
//!   cargo test --test fixture_generator -- --ignored regenerate
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use gear_memory::{
    CodeEdge, CodeEdgeKind, CodeMap, CodeMapScope, CodeMapState, CodeSymbol, CodeSymbolKind,
    GearMemoryBundle, ProvenanceOperation, ProvenanceRecord, SafeMetadata, SourceRange, SourceRef,
    SourceState, SourceType,
};

const FIXED_TIMESTAMP: &str = "2026-07-02T00:00:00Z";
const SOURCE_FILES: [&str; 3] = ["src/lib.rs", "src/main.rs", "src/sqlite.rs"];
const FIXTURE_PATH: &str = "tests/fixtures/gear-memory-repo-codemap.valid.json";

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn symbol_id_for(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("sym_{sanitized}")
}

fn line_of(span: proc_macro2::Span) -> (u32, u32) {
    let start = span.start().line.max(1) as u32;
    let end = (span.end().line.max(1) as u32).max(start);
    (start, end)
}

struct ExtractedSymbol {
    name: String,
    kind: CodeSymbolKind,
    start_line: u32,
    end_line: u32,
    is_test: bool,
    called_names: BTreeSet<String>,
}

fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("test"))
}

/// Collect the identifiers of direct calls (`foo(...)`) and method calls
/// (`x.foo(...)`) inside a block — a deliberately naive, deterministic
/// approximation good enough for fixture purposes.
fn called_names_in_block(block: &syn::Block) -> BTreeSet<String> {
    use syn::visit::Visit;

    struct CallCollector {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for CallCollector {
        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = node.func.as_ref()
                && let Some(segment) = path.path.segments.last()
            {
                self.names.insert(segment.ident.to_string());
            }
            syn::visit::visit_expr_call(self, node);
        }

        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            self.names.insert(node.method.to_string());
            syn::visit::visit_expr_method_call(self, node);
        }
    }

    let mut collector = CallCollector {
        names: BTreeSet::new(),
    };
    collector.visit_block(block);
    collector.names
}

fn extract_symbols(prefix: &str, file: &syn::File) -> Vec<ExtractedSymbol> {
    use syn::spanned::Spanned;

    let mut symbols = Vec::new();

    for item in &file.items {
        match item {
            syn::Item::Fn(item_fn) => {
                let (start_line, end_line) = line_of(item_fn.span());
                let is_test = has_test_attr(&item_fn.attrs);
                symbols.push(ExtractedSymbol {
                    name: format!("{prefix}::{}", item_fn.sig.ident),
                    kind: if is_test {
                        CodeSymbolKind::Test
                    } else {
                        CodeSymbolKind::Function
                    },
                    start_line,
                    end_line,
                    is_test,
                    called_names: called_names_in_block(&item_fn.block),
                });
            }
            syn::Item::Struct(item_struct) => {
                let (start_line, end_line) = line_of(item_struct.span());
                symbols.push(ExtractedSymbol {
                    name: format!("{prefix}::{}", item_struct.ident),
                    kind: CodeSymbolKind::Type,
                    start_line,
                    end_line,
                    is_test: false,
                    called_names: BTreeSet::new(),
                });
            }
            syn::Item::Enum(item_enum) => {
                let (start_line, end_line) = line_of(item_enum.span());
                symbols.push(ExtractedSymbol {
                    name: format!("{prefix}::{}", item_enum.ident),
                    kind: CodeSymbolKind::Type,
                    start_line,
                    end_line,
                    is_test: false,
                    called_names: BTreeSet::new(),
                });
            }
            syn::Item::Trait(item_trait) => {
                let (start_line, end_line) = line_of(item_trait.span());
                symbols.push(ExtractedSymbol {
                    name: format!("{prefix}::{}", item_trait.ident),
                    kind: CodeSymbolKind::Trait,
                    start_line,
                    end_line,
                    is_test: false,
                    called_names: BTreeSet::new(),
                });
            }
            syn::Item::Impl(item_impl) => {
                let type_name = match item_impl.self_ty.as_ref() {
                    syn::Type::Path(type_path) => type_path
                        .path
                        .segments
                        .last()
                        .map(|segment| segment.ident.to_string()),
                    _ => None,
                };
                let Some(type_name) = type_name else { continue };

                for impl_item in &item_impl.items {
                    if let syn::ImplItem::Fn(method) = impl_item {
                        let (start_line, end_line) = line_of(method.span());
                        symbols.push(ExtractedSymbol {
                            name: format!("{prefix}::{type_name}::{}", method.sig.ident),
                            kind: CodeSymbolKind::Function,
                            start_line,
                            end_line,
                            is_test: false,
                            called_names: called_names_in_block(&method.block),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    symbols
}

fn build_bundle(repo_root: &Path) -> GearMemoryBundle {
    let mut source_refs = Vec::new();
    let mut all_symbols: Vec<CodeSymbol> = Vec::new();
    let mut edges: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut per_file: Vec<(String, Vec<ExtractedSymbol>)> = Vec::new();

    for relative in SOURCE_FILES {
        let content =
            fs::read_to_string(repo_root.join(relative)).expect("read tracked source file");
        let parsed = syn::parse_file(&content).expect("source file parses");

        let source_id = format!("src_{}", relative.replace(['/', '.'], "_").to_lowercase());
        source_refs.push(SourceRef {
            source_id: source_id.clone(),
            source_type: SourceType::File,
            origin_product: "gear-memory:fixture-generator".to_string(),
            uri: Some(format!("file://{relative}")),
            content_hash: sha256_hex(content.as_bytes()),
            provenance_id: "prov_fixture_generation".to_string(),
            state: SourceState::Active,
            created_at: FIXED_TIMESTAMP.to_string(),
            metadata: SafeMetadata::default(),
        });

        let module = relative
            .trim_start_matches("src/")
            .trim_end_matches(".rs")
            .to_string();
        let prefix = format!("gear_memory::{module}");

        let extracted = extract_symbols(&prefix, &parsed);
        for symbol in &extracted {
            all_symbols.push(CodeSymbol {
                symbol_id: symbol_id_for(&symbol.name),
                kind: symbol.kind.clone(),
                name: symbol.name.clone(),
                source_ref: source_id.clone(),
                range: SourceRange {
                    start_line: symbol.start_line,
                    end_line: symbol.end_line,
                },
                content_hash: sha256_hex(
                    format!(
                        "{}:{}",
                        source_refs[source_refs.len() - 1].content_hash,
                        symbol.name
                    )
                    .as_bytes(),
                ),
            });
        }
        per_file.push((source_id, extracted));
    }

    // Callable name -> symbol id (last segment), for naive call resolution.
    let mut callable_ids: BTreeMap<String, String> = BTreeMap::new();
    for symbol in &all_symbols {
        if matches!(symbol.kind, CodeSymbolKind::Function | CodeSymbolKind::Test)
            && let Some(last) = symbol.name.rsplit("::").next()
        {
            callable_ids.insert(last.to_string(), symbol.symbol_id.clone());
        }
    }

    for (_, extracted) in &per_file {
        for symbol in extracted {
            let from_id = symbol_id_for(&symbol.name);
            for called in &symbol.called_names {
                if let Some(to_id) = callable_ids.get(called)
                    && to_id != &from_id
                {
                    let kind = if symbol.is_test { "tests" } else { "calls" };
                    edges.insert((from_id.clone(), to_id.clone(), kind.to_string()));
                }
            }
        }
    }

    all_symbols.sort_by(|a, b| a.name.cmp(&b.name).then(a.symbol_id.cmp(&b.symbol_id)));
    all_symbols.dedup_by(|a, b| a.symbol_id == b.symbol_id);

    let code_edges: Vec<CodeEdge> = edges
        .into_iter()
        .map(|(from, to, kind)| CodeEdge {
            from,
            to,
            kind: if kind == "tests" {
                CodeEdgeKind::Tests
            } else {
                CodeEdgeKind::Calls
            },
        })
        .collect();

    let code_map = CodeMap {
        code_map_id: "cm_gear_memory_repo".to_string(),
        root_source_ref: "src_src_lib_rs".to_string(),
        scope: CodeMapScope {
            repo_ref: Some("constantin-jais/gear-memory".to_string()),
            revision: "git:p1-fixture".to_string(),
            paths: vec!["src/".to_string()],
        },
        parser_refs: vec!["syn:2:dev-only-fixture-generator".to_string()],
        symbols: all_symbols,
        edges: code_edges,
        state: CodeMapState::Active,
        created_at: FIXED_TIMESTAMP.to_string(),
    };

    GearMemoryBundle {
        format: "gear.memory.v0.1".to_string(),
        source_refs,
        memory_entries: vec![],
        event_log_entries: vec![],
        code_maps: vec![code_map],
        provenance_records: vec![ProvenanceRecord {
            provenance_id: "prov_fixture_generation".to_string(),
            actor_ref: "gear-memory:fixture-generator".to_string(),
            operation: ProvenanceOperation::Created,
            inputs: SOURCE_FILES.iter().map(|s| s.to_string()).collect(),
            outputs: vec!["cm_gear_memory_repo".to_string()],
            tool_ref: Some("gear-memory-fixture-generator@0.0.0".to_string()),
            timestamp: FIXED_TIMESTAMP.to_string(),
            metadata: SafeMetadata::default(),
        }],
    }
}

/// Not part of the regular suite: rewrites the committed fixture.
#[test]
#[ignore = "regenerates tests/fixtures/gear-memory-repo-codemap.valid.json"]
fn regenerate() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bundle = build_bundle(repo_root);

    bundle.validate().expect("generated bundle is valid");

    let payload = serde_json::to_string_pretty(&bundle).expect("serialize bundle");
    fs::write(repo_root.join(FIXTURE_PATH), payload + "\n").expect("write fixture");
}
