//! GenericWalker — language-agnostic recursive tree-sitter walk producing a CallGraph with
//! control-flow decision nodes (if/for/while/match). Language parsers configure it via
//! WalkerConfig and call `walk_tree`.

use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

use crate::crawler::{CallGraph, FlowEdge};

/// Separator between a decision-node's opaque ID and its human-readable display label.
/// `\x1F` (ASCII Unit Separator) never appears in source identifiers, so it's a safe
/// delimiter inside HashMap keys.  Consumers split on this char to get the display text.
pub const DECISION_SEP: char = '\x1F';

// ── Configuration ─────────────────────────────────────────────────────────────

/// Describes how to interpret one language's AST nodes for call-graph extraction.
/// All `Option` fields default to disabled when `None`.
pub struct WalkerConfig {
    // --- Named function nodes -----------------------------------------------
    /// Node kinds that represent named function definitions
    /// (e.g. `"function_item"`, `"function_definition"`, `"function_declaration"`).
    pub function_kinds: &'static [&'static str],
    pub function_name_field: &'static str,

    // --- Anonymous function nodes -------------------------------------------
    /// Node kinds whose name must be resolved from a parent node
    /// (arrow functions, function expressions).
    pub anon_function_kinds: &'static [&'static str],
    /// Parent node kinds that carry the name (e.g. `"variable_declarator"`).
    pub anon_parent_kinds: &'static [&'static str],
    /// Field on the parent that holds the name identifier (usually `"name"`).
    pub anon_parent_name_field: &'static str,

    // --- Call nodes ---------------------------------------------------------
    pub call_kind: &'static str,
    pub call_function_field: &'static str,
    /// When the function field node has this kind, extract the method name from
    /// `method_name_field` instead of the full text (avoids `self.foo` noise).
    pub method_receiver_kind: &'static str,
    pub method_name_field: &'static str,
    /// Optional: scoped/path calls (e.g. Rust's `scoped_identifier`).
    pub path_call_kind: Option<&'static str>,
    pub path_name_field: Option<&'static str>,

    // --- if / elif / else ---------------------------------------------------
    pub if_kind: Option<&'static str>,
    /// Field on the if node holding the condition expression.
    pub if_condition_field: Option<&'static str>,
    /// Field on the if node holding the true branch body.
    pub if_then_field: Option<&'static str>,
    /// Field on the if node holding the else branch (field-based, e.g. Rust / TypeScript).
    pub if_else_field: Option<&'static str>,
    /// For Python-style elif: child node kind (not a named field on the if node).
    pub elif_clause_kind: Option<&'static str>,
    pub elif_condition_field: Option<&'static str>,
    pub elif_body_field: Option<&'static str>,
    /// For Python-style else: child node kind.
    pub else_clause_kind: Option<&'static str>,
    pub else_body_field: Option<&'static str>,

    // --- Loops --------------------------------------------------------------
    /// Node kinds for for-loops (include `for_statement`, `for_of_statement`, etc.).
    pub for_kinds: &'static [&'static str],
    /// Node kinds for while-loops.
    pub while_kinds: &'static [&'static str],
    pub loop_body_field: Option<&'static str>,
    /// Field on while-loop nodes that holds the condition (usually `"condition"`).
    /// Only used for `while_kinds`; for-loop nodes always produce a bare `"for"` label.
    pub while_condition_field: Option<&'static str>,

    // --- match (Rust-style) -------------------------------------------------
    pub match_kind: Option<&'static str>,
    pub match_value_field: Option<&'static str>,
    /// The body field holding the block of arms (e.g. `"body"` → `match_block`).
    pub match_body_field: Option<&'static str>,
    pub match_arm_kind: Option<&'static str>,
    /// The node kind whose text is used as the arm label.
    pub match_pattern_kind: Option<&'static str>,

    // --- Test scope filtering -----------------------------------------------
    /// Optional: AST node kind that represents a module/namespace container
    /// (e.g. `"mod_item"` for Rust).  When a node of this kind has a name that
    /// matches any entry in `test_mod_names`, its entire subtree is skipped so
    /// that test-only functions don't appear in the call graph.
    pub test_mod_kind: Option<&'static str>,
    /// Field on the test-module node that holds its name identifier.
    pub test_mod_name_field: &'static str,
    /// Module names to treat as test scopes (e.g. `&["tests", "test"]`).
    pub test_mod_names: &'static [&'static str],

    // --- Builtins -----------------------------------------------------------
    pub builtins: &'static phf::Set<&'static str>,

    // --- Comments (for @flow override) --------------------------------------
    /// AST node kind for single-line comments (e.g. `"line_comment"` for Rust,
    /// `"comment"` for Python / TypeScript). `None` disables `@flow` scanning.
    ///
    /// When a function is preceded immediately by a comment containing `@flow`,
    /// its name is added to the force-include set and the builtins filter is
    /// bypassed for calls to that name.
    pub comment_kind: Option<&'static str>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

/// Pre-scan the AST for function definitions whose immediately-preceding named
/// sibling is a comment containing `@flow`.  Those function names bypass the
/// builtins filter so the user can explicitly include a name that would otherwise
/// be filtered (e.g. a user-defined `new` or `get` that shadows a builtin).
fn collect_force_includes(config: &WalkerConfig, root: Node, code: &str) -> HashSet<String> {
    let mut includes = HashSet::new();
    let Some(comment_kind) = config.comment_kind else {
        return includes;
    };

    fn scan(
        node: Node,
        code: &str,
        config: &WalkerConfig,
        comment_kind: &str,
        includes: &mut HashSet<String>,
    ) {
        let kind = node.kind();

        // Skip test module subtrees.
        if let Some(tmk) = config.test_mod_kind {
            if kind == tmk {
                let name = node
                    .child_by_field_name(config.test_mod_name_field)
                    .and_then(|n| code.get(n.start_byte()..n.end_byte()))
                    .map(str::trim)
                    .unwrap_or("");
                if config.test_mod_names.contains(&name) {
                    return;
                }
            }
        }

        let is_fn = config.function_kinds.contains(&kind)
            || config.anon_function_kinds.contains(&kind);

        if is_fn {
            // The immediately preceding named sibling must be a `@flow` comment.
            let has_flow = node
                .prev_named_sibling()
                .map(|prev| {
                    prev.kind() == comment_kind
                        && code
                            .get(prev.start_byte()..prev.end_byte())
                            .map_or(false, |t| t.contains("@flow"))
                })
                .unwrap_or(false);

            if has_flow {
                if let Some(name_node) = node.child_by_field_name(config.function_name_field) {
                    let name = code
                        .get(name_node.start_byte()..name_node.end_byte())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        includes.insert(name);
                    }
                }
            }
        }

        for i in 0..node.child_count() {
            scan(node.child(i).unwrap(), code, config, comment_kind, includes);
        }
    }

    scan(root, code, config, comment_kind, &mut includes);
    includes
}

// ── Walker ────────────────────────────────────────────────────────────────────

/// Walk the AST and return both the call graph and a map of bare function
/// name → 1-indexed start line (used by the router to build `SourceMap`).
pub fn walk_tree(config: &WalkerConfig, root: Node, code: &str) -> (CallGraph, HashMap<String, u32>) {
    let force_include = collect_force_includes(config, root, code);
    let mut flow_map = CallGraph::new();
    let mut line_map: HashMap<String, u32> = HashMap::new();
    let mut counter: u32 = 0;

    #[derive(Clone, Debug)]
    struct Scope {
        id: String,
        label: Option<String>,
    }
    let mut scope_stack: Vec<Scope> = Vec::new();

    fn get_text(node: Node, code: &str) -> String {
        code.get(node.start_byte()..node.end_byte())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn walk(
        node: Node,
        code: &str,
        cfg: &WalkerConfig,
        stack: &mut Vec<Scope>,
        map: &mut CallGraph,
        ctr: &mut u32,
        force_include: &HashSet<String>,
        lines: &mut HashMap<String, u32>,
    ) {
        let kind = node.kind();

        // ── Test module skip ────────────────────────────────────────────────
        // e.g. `#[cfg(test)] mod tests { ... }` in Rust — skip the whole subtree.
        if let Some(tmk) = cfg.test_mod_kind {
            if kind == tmk {
                let name = node
                    .child_by_field_name(cfg.test_mod_name_field)
                    .map(|n| get_text(n, code))
                    .unwrap_or_default();
                if cfg.test_mod_names.contains(&name.as_str()) {
                    return;
                }
            }
        }

        // ── Named function ──────────────────────────────────────────────────
        if cfg.function_kinds.contains(&kind) {
            let name = node
                .child_by_field_name(cfg.function_name_field)
                .map(|n| get_text(n, code))
                .unwrap_or_else(|| "<anon_fn>".to_string());
            lines.insert(name.clone(), node.start_position().row as u32 + 1);
            stack.push(Scope { id: name.clone(), label: None });
            for i in 0..node.child_count() {
                walk(node.child(i).unwrap(), code, cfg, stack, map, ctr, force_include, lines);
            }
            map.entry(name).or_default();
            stack.pop();
            return;
        }

        // ── Anonymous function (arrow_function, function_expression) ────────
        if cfg.anon_function_kinds.contains(&kind) {
            let name = node
                .parent()
                .and_then(|p| {
                    if cfg.anon_parent_kinds.contains(&p.kind()) {
                        p.child_by_field_name(cfg.anon_parent_name_field)
                            .map(|n| get_text(n, code))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "<anon_fn>".to_string());
            lines.insert(name.clone(), node.start_position().row as u32 + 1);
            stack.push(Scope { id: name.clone(), label: None });
            for i in 0..node.child_count() {
                walk(node.child(i).unwrap(), code, cfg, stack, map, ctr, force_include, lines);
            }
            map.entry(name).or_default();
            stack.pop();
            return;
        }

        // ── if / elif / else ────────────────────────────────────────────────
        if cfg.if_kind.map_or(false, |k| k == kind) {
            *ctr += 1;
            let cond_text = cfg
                .if_condition_field
                .and_then(|f| node.child_by_field_name(f))
                .map(|c| truncate(&get_text(c, code), 40))
                .unwrap_or_default();
            let display =
                if cond_text.is_empty() { "if".to_string() } else { format!("if {}", cond_text) };
            let decision_id = format!("_decision_{}{}{}", ctr, DECISION_SEP, display);

            if let Some(parent_id) = stack.last().map(|s| s.id.clone()) {
                map.entry(parent_id).or_default().push(FlowEdge {
                    target: decision_id.clone(),
                    label: None,
                });
            }
            // Condition is evaluated in the parent scope.
            if let Some(f) = cfg.if_condition_field {
                if let Some(cond) = node.child_by_field_name(f) {
                    walk(cond, code, cfg, stack, map, ctr, force_include, lines);
                }
            }
            // True branch.
            if let Some(f) = cfg.if_then_field {
                if let Some(then_node) = node.child_by_field_name(f) {
                    stack.push(Scope { id: decision_id.clone(), label: Some("True".to_string()) });
                    walk(then_node, code, cfg, stack, map, ctr, force_include, lines);
                    stack.pop();
                }
            }
            // False branch — field-based (Rust, TypeScript).
            if let Some(f) = cfg.if_else_field {
                if let Some(else_node) = node.child_by_field_name(f) {
                    stack.push(Scope {
                        id: decision_id.clone(),
                        label: Some("False".to_string()),
                    });
                    walk(else_node, code, cfg, stack, map, ctr, force_include, lines);
                    stack.pop();
                }
            }
            // Elif clauses — child-node-based (Python).
            if let Some(elif_kind) = cfg.elif_clause_kind {
                let mut elif_n: u32 = 0;
                for i in 0..node.child_count() {
                    let child = node.child(i).unwrap();
                    if child.kind() == elif_kind {
                        elif_n += 1;
                        if let Some(f) = cfg.elif_condition_field {
                            if let Some(cond) = child.child_by_field_name(f) {
                                walk(cond, code, cfg, stack, map, ctr, force_include, lines);
                            }
                        }
                        let label = if elif_n == 1 {
                            "Elif".to_string()
                        } else {
                            format!("Elif_{}", elif_n)
                        };
                        if let Some(f) = cfg.elif_body_field {
                            if let Some(body) = child.child_by_field_name(f) {
                                stack.push(Scope { id: decision_id.clone(), label: Some(label) });
                                walk(body, code, cfg, stack, map, ctr, force_include, lines);
                                stack.pop();
                            }
                        }
                    }
                }
            }
            // Else clause — child-node-based (Python).
            if let Some(else_kind) = cfg.else_clause_kind {
                for i in 0..node.child_count() {
                    let child = node.child(i).unwrap();
                    if child.kind() == else_kind {
                        if let Some(f) = cfg.else_body_field {
                            if let Some(body) = child.child_by_field_name(f) {
                                stack.push(Scope {
                                    id: decision_id.clone(),
                                    label: Some("False".to_string()),
                                });
                                walk(body, code, cfg, stack, map, ctr, force_include, lines);
                                stack.pop();
                            }
                        }
                        break;
                    }
                }
            }
            map.entry(decision_id).or_default();
            return;
        }

        // ── for / while loops ───────────────────────────────────────────────
        if cfg.for_kinds.contains(&kind) || cfg.while_kinds.contains(&kind) {
            *ctr += 1;
            let is_while = cfg.while_kinds.contains(&kind);
            let cond_text = if is_while {
                cfg.while_condition_field
                    .and_then(|f| node.child_by_field_name(f))
                    .map(|c| truncate(&get_text(c, code), 40))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let loop_label = if is_while { "while" } else { "for" };
            let display = if cond_text.is_empty() {
                loop_label.to_string()
            } else {
                format!("{} {}", loop_label, cond_text)
            };
            let decision_id = format!("_decision_{}{}{}", ctr, DECISION_SEP, display);

            if let Some(parent_id) = stack.last().map(|s| s.id.clone()) {
                map.entry(parent_id).or_default().push(FlowEdge {
                    target: decision_id.clone(),
                    label: None,
                });
            }
            if let Some(f) = cfg.loop_body_field {
                if let Some(body) = node.child_by_field_name(f) {
                    stack.push(Scope { id: decision_id.clone(), label: Some("Loop".to_string()) });
                    walk(body, code, cfg, stack, map, ctr, force_include, lines);
                    stack.pop();
                }
            }
            map.entry(decision_id).or_default();
            return;
        }

        // ── match (Rust-style) ──────────────────────────────────────────────
        if cfg.match_kind.map_or(false, |k| k == kind) {
            *ctr += 1;
            let value_text = cfg
                .match_value_field
                .and_then(|f| node.child_by_field_name(f))
                .map(|c| truncate(&get_text(c, code), 40))
                .unwrap_or_default();
            let display = if value_text.is_empty() {
                "match".to_string()
            } else {
                format!("match {}", value_text)
            };
            let decision_id = format!("_decision_{}{}{}", ctr, DECISION_SEP, display);

            if let Some(parent_id) = stack.last().map(|s| s.id.clone()) {
                map.entry(parent_id).or_default().push(FlowEdge {
                    target: decision_id.clone(),
                    label: None,
                });
            }
            // Matched value is walked in the parent scope.
            if let Some(f) = cfg.match_value_field {
                if let Some(val) = node.child_by_field_name(f) {
                    walk(val, code, cfg, stack, map, ctr, force_include, lines);
                }
            }
            // Arms inside the match body node.
            let arms_parent = cfg
                .match_body_field
                .and_then(|f| node.child_by_field_name(f))
                .unwrap_or(node);
            if let Some(arm_kind) = cfg.match_arm_kind {
                for i in 0..arms_parent.child_count() {
                    let child = arms_parent.child(i).unwrap();
                    if child.kind() == arm_kind {
                        let mut label: Option<String> = None;
                        if let Some(pat_kind) = cfg.match_pattern_kind {
                            for k in 0..child.child_count() {
                                let sub = child.child(k).unwrap();
                                if sub.kind() == pat_kind {
                                    let text = get_text(sub, code);
                                    if !text.is_empty() {
                                        label = Some(text);
                                    }
                                }
                            }
                        }
                        let label = label.or_else(|| Some("_".to_string()));
                        stack.push(Scope { id: decision_id.clone(), label });
                        for k in 0..child.child_count() {
                            let sub = child.child(k).unwrap();
                            let is_pattern = cfg
                                .match_pattern_kind
                                .map_or(false, |pk| sub.kind() == pk);
                            if !is_pattern {
                                walk(sub, code, cfg, stack, map, ctr, force_include, lines);
                            }
                        }
                        stack.pop();
                    }
                }
            }
            map.entry(decision_id).or_default();
            return;
        }

        // ── call expression ─────────────────────────────────────────────────
        if kind == cfg.call_kind {
            let callee = match node.child_by_field_name(cfg.call_function_field) {
                None => return,
                Some(func) => {
                    let fk = func.kind();
                    if fk == cfg.method_receiver_kind {
                        func.child_by_field_name(cfg.method_name_field)
                            .map(|n| get_text(n, code))
                            .unwrap_or_else(|| get_text(func, code))
                    } else if cfg.path_call_kind.map_or(false, |pk| pk == fk) {
                        let nf = cfg.path_name_field.unwrap_or("name");
                        func.child_by_field_name(nf)
                            .map(|n| get_text(n, code))
                            .unwrap_or_else(|| get_text(func, code))
                    } else {
                        get_text(func, code)
                    }
                }
            };
            // Builtins filter: skip unless the name is force-included via `@flow`.
            let filtered = cfg.builtins.contains(callee.as_str())
                && !force_include.contains(&callee);
            if !callee.is_empty() && !filtered {
                if let Some(scope) = stack.last() {
                    map.entry(scope.id.clone()).or_default().push(FlowEdge {
                        target: callee,
                        label: scope.label.clone(),
                    });
                }
            }
            // Recurse to capture calls in arguments.
            for i in 0..node.child_count() {
                walk(node.child(i).unwrap(), code, cfg, stack, map, ctr, force_include, lines);
            }
            return;
        }

        // ── default: walk children ──────────────────────────────────────────
        for i in 0..node.child_count() {
            walk(node.child(i).unwrap(), code, cfg, stack, map, ctr, force_include, lines);
        }
    }

    walk(root, code, config, &mut scope_stack, &mut flow_map, &mut counter, &force_include, &mut line_map);
    (flow_map, line_map)
}
