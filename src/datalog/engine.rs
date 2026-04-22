use crate::datalog::builtins;
use crate::datalog::{Fact, Predicate, Rule, SourceLocation, Term, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use xxhash_rust::xxh3::xxh3_64;

type Index = HashMap<String, Vec<usize>>;
type ReverseIndex = HashMap<Fact, usize>;
type IndexPair = (RefCell<Option<Index>>, RefCell<Option<ReverseIndex>>);

pub type ResolveCallback = Box<dyn Fn(&str, &str, &[String]) -> Result<Vec<u8>, String>>;

/// An iterator that lazily computes transitive closure results.
///
/// This iterator performs BFS incrementally, yielding results one at a time
/// instead of materializing all results upfront.
pub struct TcIterator {
    head_name: String,
    start_node: Value,
    visited: HashSet<Value>,
    queue: std::collections::VecDeque<Value>,
    fact_vec: Vec<Rc<Fact>>,
    index: Index,
    pending_results: Vec<Rc<Fact>>,
}

impl Iterator for TcIterator {
    type Item = Rc<Fact>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.pending_results.pop() {
            return Some(result);
        }

        while let Some(node) = self.queue.pop_front() {
            if let Value::String(node_str) = &node {
                if let Some(indices) = self.index.get(node_str) {
                    for &idx in indices {
                        let fact = &self.fact_vec[idx];
                        if fact.args.len() == 2 {
                            let neighbor = fact.args[1].clone();
                            if self.visited.insert(neighbor.clone()) {
                                self.queue.push_back(neighbor.clone());
                                self.pending_results.push(Rc::new(Fact {
                                    predicate: self.head_name.clone(),
                                    args: vec![self.start_node.clone(), neighbor],
                                }));
                            }
                        }
                    }

                    if let Some(result) = self.pending_results.pop() {
                        return Some(result);
                    }
                }
            }
        }
        None
    }
}

/// Adjacency list for fast TC traversal, built from edge facts without cloning strings.
/// Instead of storing strings, we store indices into the fact vector.
struct TcAdjList {
    string_to_id: HashMap<u64, u32>,  // hash(string) -> node id
    id_to_fact_ref: Vec<(usize, u8)>, // (fact_idx, arg_pos 0 or 1) for each node id
    adjacency: Vec<Vec<u32>>,         // node_id -> vec of neighbor node_ids
}

pub struct Engine {
    base_facts: HashMap<String, (Vec<Rc<Fact>>, Option<usize>)>,
    indices: Vec<IndexPair>,
    rules: HashMap<String, Vec<Rule>>,
    computed: HashMap<String, Vec<Rc<Fact>>>,
    tc_cache: HashMap<(String, Value), Vec<Rc<Fact>>>,
    resolve_cache: HashMap<String, Vec<String>>,
    resolve_callback: Option<ResolveCallback>,
    source_locations: HashMap<String, SourceLocation>,
    stratification_validated: bool,

    // Lazily initialized
    tc_adj_lists: HashMap<String, TcAdjList>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            base_facts: HashMap::new(),
            indices: Vec::new(),
            rules: HashMap::new(),
            computed: HashMap::new(),
            tc_cache: HashMap::new(),
            resolve_cache: HashMap::new(),
            resolve_callback: None,
            source_locations: HashMap::new(),
            stratification_validated: false,
            tc_adj_lists: HashMap::new(),
        }
    }

    pub fn set_resolve_callback(&mut self, callback: ResolveCallback) {
        self.resolve_callback = Some(callback);
    }

    pub fn insert_facts(&mut self, facts: Vec<Fact>) {
        let mut affected_predicates = std::collections::HashSet::new();
        let mut affected_sources = std::collections::HashSet::new();

        for fact in facts {
            let rc_fact = Rc::new(fact);

            affected_predicates.insert(rc_fact.predicate.clone());

            if !rc_fact.args.is_empty() {
                affected_sources.insert(rc_fact.args[0].clone());
            }

            let predicate = rc_fact.predicate.clone();
            let entry = self.base_facts.entry(predicate).or_insert_with(|| {
                let idx = self.indices.len();
                self.indices.push((RefCell::new(None), RefCell::new(None)));
                (Vec::new(), Some(idx))
            });
            entry.0.push(rc_fact.clone());

            if let Some(idx) = entry.1 {
                *self.indices[idx].0.borrow_mut() = None;
                *self.indices[idx].1.borrow_mut() = None;
            }
        }

        self.invalidate_dependent_predicates(&affected_predicates, &affected_sources);
    }

    pub fn retract_facts(&mut self, facts: Vec<Fact>) {
        let mut affected_predicates = std::collections::HashSet::new();
        let mut affected_sources = std::collections::HashSet::new();

        let mut by_predicate: std::collections::HashMap<String, Vec<&Fact>> =
            std::collections::HashMap::new();
        for fact in &facts {
            affected_predicates.insert(fact.predicate.clone());
            if !fact.args.is_empty() {
                affected_sources.insert(fact.args[0].clone());
            }
            by_predicate
                .entry(fact.predicate.clone())
                .or_default()
                .push(fact);
        }

        for (predicate, pred_facts) in by_predicate {
            if let Some((fact_vec, Some(idx))) = self.base_facts.get_mut(&predicate) {
                let (index_cell, reverse_cell) = &self.indices[*idx];

                Self::build_reverse_index_if_needed(fact_vec, reverse_cell);

                for fact in pred_facts {
                    let mut reverse_borrow = reverse_cell.borrow_mut();
                    if let Some(index) = reverse_borrow.as_mut() {
                        if let Some(fact_idx) = index.remove(fact) {
                            drop(reverse_borrow);

                            fact_vec.swap_remove(fact_idx);

                            if fact_idx < fact_vec.len() {
                                let mut reverse_borrow = reverse_cell.borrow_mut();
                                if let Some(index) = reverse_borrow.as_mut() {
                                    index.insert(fact_vec[fact_idx].as_ref().clone(), fact_idx);
                                }
                            }
                        }
                    }
                }

                *index_cell.borrow_mut() = None;
            }
        }

        self.invalidate_dependent_predicates(&affected_predicates, &affected_sources);
    }

    pub fn compile_rule(&mut self, rule: Rule) {
        if let Err(e) = self.validate_rule(&rule) {
            eprintln!("Error: Invalid rule: {}", e);
            eprintln!(
                "Rule: {} :- {:?}",
                rule.head.name,
                rule.body.iter().map(|p| &p.name).collect::<Vec<_>>()
            );
            std::process::exit(1);
        }

        let head_pred = rule.head.name.clone();
        self.rules.entry(head_pred.clone()).or_default().push(rule);
        self.computed.remove(&head_pred);
        self.stratification_validated = false;
    }

    pub fn record_source_location(&mut self, predicate: &str, location: SourceLocation) {
        self.source_locations
            .insert(predicate.to_string(), location);
    }

    pub fn get_source_location(&self, predicate: &str) -> Option<&SourceLocation> {
        self.source_locations.get(predicate)
    }

    pub fn materialize_source_locations(&mut self) {
        let location_facts: Vec<Fact> = self
            .source_locations
            .iter()
            .map(|(pred, loc)| {
                Fact::new(
                    "source_location",
                    vec![
                        Value::String(pred.clone()),
                        Value::String(loc.file.clone()),
                        Value::Integer(loc.line as i64),
                    ],
                )
            })
            .collect();

        self.insert_facts(location_facts);
    }

    fn validate_rule(&self, rule: &Rule) -> Result<(), String> {
        self.check_rule_safety(rule)?;
        self.check_stratification_with_new_rule(rule)?;
        self.check_arity_consistency(rule)?;
        Ok(())
    }

    fn check_arity_consistency(&self, rule: &Rule) -> Result<(), String> {
        let name = &rule.head.name;
        if let Some(existing_rules) = self.rules.get(name) {
            if let Some(first_rule) = existing_rules.first() {
                let existing_arity = first_rule.head.args.len();
                let new_arity = rule.head.args.len();
                if new_arity != existing_arity {
                    return Err(format!(
                        "Arity mismatch: predicate '{}' used with {} and {} arguments",
                        name, existing_arity, new_arity
                    ));
                }
            }
        }
        Ok(())
    }

    fn check_stratification_with_new_rule(&self, new_rule: &Rule) -> Result<(), String> {
        let new_head = &new_rule.head.name;

        for body_pred in &new_rule.body {
            if body_pred.name.starts_with("not:") {
                let negated_pred = body_pred.name.strip_prefix("not:").unwrap();

                if self.has_path_through_recursion_with_new_rule(
                    negated_pred,
                    new_head,
                    new_rule,
                    &mut std::collections::HashSet::new(),
                )? {
                    return Err(format!(
                        "Non-stratified negation: {} depends on not({}) but {} recursively depends on {}",
                        new_head, negated_pred, negated_pred, new_head
                    ));
                }
            }
        }

        for (existing_head, existing_rules) in &self.rules {
            for existing_rule in existing_rules {
                for body_pred in &existing_rule.body {
                    if body_pred.name.starts_with("not:") {
                        let negated_pred = body_pred.name.strip_prefix("not:").unwrap();

                        if self.has_path_through_recursion_with_new_rule(
                            negated_pred,
                            existing_head,
                            new_rule,
                            &mut std::collections::HashSet::new(),
                        )? {
                            return Err(format!(
                                "Non-stratified negation: {} depends on not({}) but {} recursively depends on {}",
                                existing_head, negated_pred, negated_pred, existing_head
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn validate_all_stratification(&self) -> Result<(), String> {
        for (head_pred, rules) in &self.rules {
            for rule in rules {
                for body_pred in &rule.body {
                    if body_pred.name.starts_with("not:") {
                        let negated_pred = body_pred.name.strip_prefix("not:").unwrap();

                        if self.has_path_through_recursion(
                            negated_pred,
                            head_pred,
                            &mut std::collections::HashSet::new(),
                        )? {
                            return Err(format!(
                                "Non-stratified negation: {} depends on not({}) but {} recursively depends on {}",
                                head_pred, negated_pred, negated_pred, head_pred
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn has_path_through_recursion(
        &self,
        from: &str,
        to: &str,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<bool, String> {
        if from == to {
            return Ok(true);
        }

        if visited.contains(from) {
            return Ok(false);
        }

        visited.insert(from.to_string());

        if let Some(rules) = self.rules.get(from) {
            for rule in rules {
                for body_pred in &rule.body {
                    let pred_name = if body_pred.name.starts_with("not:") {
                        body_pred.name.strip_prefix("not:").unwrap()
                    } else {
                        &body_pred.name
                    };

                    if !Self::is_special_predicate(pred_name)
                        && self.has_path_through_recursion(pred_name, to, visited)?
                    {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn has_path_through_recursion_with_new_rule(
        &self,
        from: &str,
        to: &str,
        new_rule: &Rule,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<bool, String> {
        if from == to {
            return Ok(true);
        }

        if visited.contains(from) {
            return Ok(false);
        }

        visited.insert(from.to_string());

        if from == new_rule.head.name {
            for body_pred in &new_rule.body {
                let pred_name = if body_pred.name.starts_with("not:") {
                    body_pred.name.strip_prefix("not:").unwrap()
                } else {
                    &body_pred.name
                };

                if !Self::is_special_predicate(pred_name)
                    && self.has_path_through_recursion_with_new_rule(
                        pred_name, to, new_rule, visited,
                    )?
                {
                    return Ok(true);
                }
            }
        }

        if let Some(rules) = self.rules.get(from) {
            for rule in rules {
                for body_pred in &rule.body {
                    let pred_name = if body_pred.name.starts_with("not:") {
                        body_pred.name.strip_prefix("not:").unwrap()
                    } else {
                        &body_pred.name
                    };

                    if !Self::is_special_predicate(pred_name)
                        && self.has_path_through_recursion_with_new_rule(
                            pred_name, to, new_rule, visited,
                        )?
                    {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn check_rule_safety(&self, rule: &Rule) -> Result<(), String> {
        let mut head_vars = std::collections::HashSet::new();
        for term in &rule.head.args {
            if let Term::Variable(var) = term {
                if !var.starts_with("_anon_") {
                    head_vars.insert(var.clone());
                }
            }
        }

        let mut positive_body_vars = std::collections::HashSet::new();
        for predicate in &rule.body {
            if predicate.name.starts_with("not:") {
                continue;
            }
            for term in &predicate.args {
                if let Term::Variable(var) = term {
                    if !var.starts_with("_anon_") {
                        positive_body_vars.insert(var.clone());
                    }
                }
            }
        }

        for var in &head_vars {
            if !positive_body_vars.contains(var) {
                return Err(format!(
                    "Variable '{}' appears in head but not in positive body literal (safety violation)",
                    var
                ));
            }
        }

        Ok(())
    }

    fn build_index_if_needed(
        fact_vec: &[Rc<Fact>],
        index_cell: &RefCell<Option<HashMap<String, Vec<usize>>>>,
    ) {
        if index_cell.borrow().is_none() {
            let mut index: HashMap<String, Vec<usize>> = HashMap::new();
            for (idx, fact) in fact_vec.iter().enumerate() {
                if let Some(Value::String(first_arg)) = fact.args.first() {
                    index.entry(first_arg.clone()).or_default().push(idx);
                }
            }
            *index_cell.borrow_mut() = Some(index);
        }
    }

    fn build_reverse_index_if_needed(
        fact_vec: &[Rc<Fact>],
        reverse_cell: &RefCell<Option<HashMap<Fact, usize>>>,
    ) {
        if reverse_cell.borrow().is_none() {
            let mut index = HashMap::new();
            for (idx, fact) in fact_vec.iter().enumerate() {
                index.insert(fact.as_ref().clone(), idx);
            }
            *reverse_cell.borrow_mut() = Some(index);
        }
    }

    fn invalidate_dependent_predicates(
        &mut self,
        affected: &std::collections::HashSet<String>,
        affected_sources: &std::collections::HashSet<Value>,
    ) {
        let mut to_invalidate = affected.clone();
        let mut changed = true;

        while changed {
            changed = false;
            for (head, rules) in &self.rules {
                if to_invalidate.contains(head) {
                    continue;
                }

                for rule in rules {
                    for body_pred in &rule.body {
                        // Strip "not:" prefix when checking invalidation dependencies
                        // since negated predicates depend on the base predicate
                        let pred_name = if body_pred.name.starts_with("not:") {
                            &body_pred.name[4..]
                        } else {
                            &body_pred.name
                        };
                        if to_invalidate.contains(pred_name) {
                            to_invalidate.insert(head.clone());
                            changed = true;
                            break;
                        }
                    }
                    if changed {
                        break;
                    }
                }
            }
        }

        for pred in &to_invalidate {
            self.computed.remove(pred);
        }

        // Retain TC cache entries where BOTH the predicate is NOT invalidated AND the source is NOT affected.
        // We want to remove entries if either predicate invalidated OR source affected.
        self.tc_cache.retain(|(pred, source), _| {
            !to_invalidate.contains(pred) && !affected_sources.contains(source)
        });

        for pred in &to_invalidate {
            self.tc_adj_lists.remove(pred);
        }
    }

    pub fn query(&mut self, predicate: &str, filters: &[Option<&str>]) -> Vec<Rc<Fact>> {
        if !self.stratification_validated {
            if let Err(e) = self.validate_all_stratification() {
                eprintln!("Error: Stratification validation failed: {}", e);
                std::process::exit(1);
            }
            self.stratification_validated = true;
        }

        if predicate == "source_location" && !self.base_facts.contains_key("source_location") {
            self.materialize_source_locations();
        }

        if self.is_transitive_closure_predicate(predicate) && !filters.is_empty() {
            if let Some(start_filter) = filters.first().and_then(|f| f.as_ref()) {
                return self.query_transitive_closure_lazy(predicate, start_filter, filters);
            }
        }

        if !self.rules.contains_key(predicate) {
            return self.query_base_facts(predicate, filters);
        }

        self.ensure_computed(predicate);

        if filters.iter().all(|f| f.is_none()) {
            if let Some(facts) = self.computed.get(predicate) {
                let rc_facts: Vec<Rc<Fact>> = facts.iter().map(Rc::clone).collect();
                return apply_filters(rc_facts, filters);
            }
        }

        let mut results = if filters.iter().any(|f| f.is_some()) {
            let initial_bindings = self.filters_to_bindings(predicate, filters);
            let rules = self.rules.get(predicate).cloned();
            let mut rule_results = Vec::new();
            if let Some(rules) = rules {
                for rule in &rules {
                    rule_results
                        .extend(self.evaluate_rule_with_bindings(rule, initial_bindings.clone()));
                }
            }
            rule_results
        } else {
            self.evaluate_predicate(predicate)
        };

        if let Some((fact_vec, idx_ptr)) = self.base_facts.get(predicate) {
            let first_filter = filters.first().and_then(|f| f.as_ref());
            let idx = idx_ptr.and_then(|i| {
                if first_filter.is_some() {
                    Some(i)
                } else {
                    None
                }
            });

            let base_facts: Vec<Rc<Fact>> = if let (Some(idx), Some(filter)) = (idx, first_filter) {
                let (index_cell, _) = &self.indices[idx];
                Self::build_index_if_needed(fact_vec, index_cell);
                let index_ref = index_cell.borrow();
                if let Some(ref index) = *index_ref {
                    if let Some(indices) = index.get(*filter) {
                        indices.iter().map(|&i| Rc::clone(&fact_vec[i])).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            } else {
                fact_vec.iter().map(Rc::clone).collect()
            };

            results.extend(base_facts);
        }

        if filters.iter().all(|f| f.is_none()) {
            self.computed.insert(predicate.to_string(), results.clone());
        }

        // Duplicates can occur from multiple rules deriving the same fact or if
        // a fact is both a base fact and derivable by rules
        let mut seen: std::collections::HashSet<Rc<Fact>> = std::collections::HashSet::new();
        results.retain(|rc| seen.insert(Rc::clone(rc)));

        apply_filters(results, filters)
    }

    fn filters_to_bindings(
        &self,
        predicate_name: &str,
        filters: &[Option<&str>],
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if filters.is_empty() || filters.iter().all(|f| f.is_none()) {
            return vec![Arc::new(HashMap::new())];
        }

        let rules = match self.rules.get(predicate_name) {
            Some(r) if !r.is_empty() => r,
            _ => return vec![Arc::new(HashMap::new())],
        };

        let mut binding = HashMap::new();
        for (i, filter_opt) in filters.iter().enumerate() {
            if let Some(filter_val) = filter_opt {
                if let Some(Term::Variable(var)) = rules[0].head.args.get(i) {
                    binding.insert(var.clone(), Value::String(filter_val.to_string()));
                }
            }
        }

        if binding.is_empty() {
            vec![Arc::new(HashMap::new())]
        } else {
            vec![Arc::new(binding)]
        }
    }

    fn is_transitive_closure_predicate(&self, predicate: &str) -> bool {
        if let Some(rules) = self.rules.get(predicate) {
            if rules.len() != 2 {
                return false;
            }
            let has_base = rules.iter().any(|r| {
                r.body.len() == 1 && r.body[0].name != r.head.name && r.body[0].args.len() == 2
            });
            let has_recursive = rules
                .iter()
                .any(|r| r.body.len() == 2 && r.body.iter().any(|p| p.name == r.head.name));
            return has_base && has_recursive;
        }
        false
    }

    fn query_transitive_closure_lazy(
        &mut self,
        predicate: &str,
        start_node: &str,
        filters: &[Option<&str>],
    ) -> Vec<Rc<Fact>> {
        let start_value = Value::String(start_node.to_string());
        let cache_key = (predicate.to_string(), start_value.clone());

        if let Some(cached) = self.tc_cache.get(&cache_key) {
            let rc_facts: Vec<Rc<Fact>> = cached.iter().map(Rc::clone).collect();
            return apply_filters(rc_facts, filters);
        }

        let rules = self.rules.get(predicate).unwrap().clone();
        let base_rule = rules.iter().find(|r| r.body.len() == 1).unwrap();
        let edge_pred_name = &base_rule.body[0].name.clone();

        let results = self.evaluate_tc_from_node_indexed(predicate, edge_pred_name, &start_value);
        self.tc_cache.insert(cache_key, results.clone());

        let rc_results: Vec<Rc<Fact>> = results.iter().map(Rc::clone).collect();
        apply_filters(rc_results, filters)
    }

    fn ensure_tc_adj_list(&mut self, edge_pred: &str) {
        if self.tc_adj_lists.contains_key(edge_pred) {
            return;
        }

        let Some((fact_vec, _)) = self.base_facts.get(edge_pred) else {
            return;
        };

        let capacity = fact_vec.len();
        let mut string_to_id: HashMap<u64, u32> = HashMap::with_capacity(capacity);
        let mut id_to_fact_ref: Vec<(usize, u8)> = Vec::with_capacity(capacity);

        for (fact_idx, fact) in fact_vec.iter().enumerate() {
            if fact.args.len() >= 2 {
                if let (Value::String(src), Value::String(dst)) = (&fact.args[0], &fact.args[1]) {
                    let src_hash = xxh3_64(src.as_bytes());
                    if let std::collections::hash_map::Entry::Vacant(e) = string_to_id.entry(src_hash) {
                        e.insert(id_to_fact_ref.len() as u32);
                        id_to_fact_ref.push((fact_idx, 0));
                    }
                    let dst_hash = xxh3_64(dst.as_bytes());
                    if let std::collections::hash_map::Entry::Vacant(e) = string_to_id.entry(dst_hash) {
                        e.insert(id_to_fact_ref.len() as u32);
                        id_to_fact_ref.push((fact_idx, 1));
                    }
                }
            }
        }

        let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); id_to_fact_ref.len()];
        for fact in fact_vec {
            if fact.args.len() >= 2 {
                if let (Value::String(src), Value::String(dst)) = (&fact.args[0], &fact.args[1]) {
                    let src_hash = xxh3_64(src.as_bytes());
                    let dst_hash = xxh3_64(dst.as_bytes());
                    let src_id = string_to_id[&src_hash];
                    let dst_id = string_to_id[&dst_hash];
                    adjacency[src_id as usize].push(dst_id);
                }
            }
        }

        self.tc_adj_lists.insert(
            edge_pred.to_string(),
            TcAdjList {
                string_to_id,
                id_to_fact_ref,
                adjacency,
            },
        );
    }

    fn evaluate_tc_from_node_indexed(
        &mut self,
        head_name: &str,
        edge_pred_name: &str,
        start_node: &Value,
    ) -> Vec<Rc<Fact>> {
        self.ensure_tc_adj_list(edge_pred_name);

        let Value::String(start_str) = start_node else {
            return Vec::new();
        };

        let Some(tc_adj_list) = self.tc_adj_lists.get(edge_pred_name) else {
            return Vec::new();
        };

        let start_hash = xxh3_64(start_str.as_bytes());
        let Some(&start_id) = tc_adj_list.string_to_id.get(&start_hash) else {
            return Vec::new();
        };

        let Some((fact_vec, _)) = self.base_facts.get(edge_pred_name) else {
            return Vec::new();
        };

        // BFS
        let mut visited: Vec<bool> = vec![false; tc_adj_list.id_to_fact_ref.len()];
        let mut queue: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
        queue.push_back(start_id);
        visited[start_id as usize] = true;

        let mut reachable_ids: Vec<u32> = Vec::new();

        while let Some(node) = queue.pop_front() {
            for &neighbor in &tc_adj_list.adjacency[node as usize] {
                if !visited[neighbor as usize] {
                    visited[neighbor as usize] = true;
                    reachable_ids.push(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }

        let head_name_owned = head_name.to_string();
        let mut results = Vec::with_capacity(reachable_ids.len());

        for id in reachable_ids {
            let (fact_idx, arg_pos) = tc_adj_list.id_to_fact_ref[id as usize];
            if let Value::String(dst_string) = &fact_vec[fact_idx].args[arg_pos as usize] {
                results.push(Rc::new(Fact {
                    predicate: head_name_owned.clone(),
                    args: vec![start_node.clone(), Value::String(dst_string.clone())],
                }));
            }
        }

        results
    }

    /// Creates a lazy iterator for transitive closure queries.
    ///
    /// Returns an iterator that computes transitive closure results incrementally
    /// using BFS, starting from the specified node. This is more efficient than
    /// `query()` when you only need a subset of results or want to process them
    /// one at a time.
    ///
    /// # Arguments
    ///
    /// * `predicate` - The name of the transitive closure predicate
    /// * `start_node` - The starting node for the transitive closure
    ///
    /// # Returns
    ///
    /// A `TcIterator` that yields facts of the form `predicate(start_node, reachable_node)`
    /// for each node reachable from `start_node` through the transitive closure.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use redwood::datalog::Engine;
    ///
    /// let mut db = Engine::new();
    /// // ... insert facts and compile rules ...
    ///
    /// // Get just the first 10 transitive dependencies
    /// let first_10: Vec<_> = db.query_tc_iter("transitive_deps", "//app:main")
    ///     .take(10)
    ///     .collect();
    ///
    /// // Or count all results without allocating
    /// let total_count = db.query_tc_iter("transitive_deps", "//app:main").count();
    /// ```
    pub fn query_tc_iter(
        &mut self,
        predicate: &str,
        start_node: &str,
    ) -> TcIterator {
        let start_value = Value::String(start_node.to_string());

        let rules = self.rules.get(predicate).cloned().unwrap_or_default();
        let base_rule = rules.iter().find(|r| r.body.len() == 1);

        let edge_pred_name = base_rule
            .map(|r| r.body[0].name.clone())
            .unwrap_or_default();

        let (fact_vec, index) = if let Some((facts, Some(idx))) = self.base_facts.get(&edge_pred_name) {
            let (index_cell, _) = &self.indices[*idx];
            Self::build_index_if_needed(facts, index_cell);

            let index_ref = index_cell.borrow();
            let cloned_index = if let Some(ref idx) = *index_ref {
                idx.clone()
            } else {
                HashMap::new()
            };

            (facts.clone(), cloned_index)
        } else {
            (Vec::new(), HashMap::new())
        };

        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start_value.clone());
        visited.insert(start_value.clone());

        TcIterator {
            head_name: predicate.to_string(),
            start_node: start_value,
            visited,
            queue,
            fact_vec,
            index,
            pending_results: Vec::new(),
        }
    }

    fn evaluate_predicate(&mut self, predicate: &str) -> Vec<Rc<Fact>> {
        let results = self.evaluate_fixpoint(&[predicate.to_string()]);
        results.get(predicate).cloned().unwrap_or_default()
    }

    fn evaluate_rule(&mut self, rule: &Rule) -> Vec<Rc<Fact>> {
        self.evaluate_rule_with_bindings(rule, vec![Arc::new(HashMap::new())])
    }

    fn evaluate_rule_with_bindings(
        &mut self,
        rule: &Rule,
        initial_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Rc<Fact>> {
        let mut bindings = initial_bindings;

        for predicate in &rule.body {
            bindings = self.eval_predicate_with_bindings(predicate, bindings);
            if bindings.is_empty() {
                return Vec::new();
            }
        }

        bindings
            .into_iter()
            .map(|binding| {
                let args = project_to_head(&rule.head.args, &binding);
                Rc::new(Fact::new(&rule.head.name, args))
            })
            .collect()
    }

    fn is_special_predicate(name: &str) -> bool {
        builtins::is_special_predicate(name)
    }

    fn can_check_membership(
        &self,
        pred_name: &str,
        predicate: &Predicate,
        binding: &HashMap<String, Value>,
    ) -> bool {
        let rules = match self.rules.get(pred_name) {
            Some(r) if r.len() == 1 => r,
            _ => return false,
        };

        let rule = &rules[0];

        for arg in &predicate.args {
            if let Term::Variable(var) = arg {
                if !binding.contains_key(var) {
                    return false;
                }
            }
        }

        for body_pred in &rule.body {
            if Self::is_special_predicate(&body_pred.name) {
                continue;
            }

            for arg in &body_pred.args {
                if let Term::Variable(var) = arg {
                    if !binding.contains_key(var) {
                        return false;
                    }
                }
            }

            if body_pred.name == pred_name {
                return false;
            }
        }

        true
    }

    fn check_membership(
        &mut self,
        pred_name: &str,
        _predicate: &Predicate,
        binding: &HashMap<String, Value>,
    ) -> bool {
        let rules = self.rules.get(pred_name).cloned().unwrap();
        let rule = &rules[0];

        for body_pred in &rule.body {
            if body_pred.name.starts_with("not:") {
                let inner_name = &body_pred.name[4..];
                let inner_pred = Predicate {
                    name: inner_name.to_string(),
                    args: body_pred.args.clone(),
                };
                self.ensure_computed(inner_name);
                if self.check_membership(inner_name, &inner_pred, binding) {
                    return false;
                }
                continue;
            }

            if Self::is_special_predicate(&body_pred.name) {
                let test_bindings = vec![Arc::new(binding.clone())];
                let result = match body_pred.name.as_str() {
                    "=" => builtins::eval_equality(body_pred, test_bindings),
                    "!=" => builtins::eval_inequality(body_pred, test_bindings),
                    "gt" | "lt" => builtins::eval_comparison(body_pred, test_bindings),
                    _ => test_bindings,
                };
                if result.is_empty() {
                    return false;
                }
                continue;
            }

            let body_values: Vec<Value> = body_pred
                .args
                .iter()
                .map(|arg| match arg {
                    Term::Variable(var) => binding
                        .get(var)
                        .cloned()
                        .unwrap_or_else(|| Value::String("__unbound__".to_string())),
                    Term::Constant(val) => val.clone(),
                })
                .collect();

            let check_fact = Fact {
                predicate: body_pred.name.clone(),
                args: body_values.clone(),
            };

            let exists = if self.rules.contains_key(&body_pred.name)
                && !self.computed.contains_key(&body_pred.name)
            {
                let mut check_binding = HashMap::new();
                if let Some(body_rules) = self.rules.get(&body_pred.name) {
                    if !body_rules.is_empty() {
                        let body_rule = &body_rules[0];
                        for (i, arg) in body_rule.head.args.iter().enumerate() {
                            if let Term::Variable(var) = arg {
                                if i < body_values.len() {
                                    check_binding.insert(var.clone(), body_values[i].clone());
                                }
                            }
                        }
                    }
                }

                if self.can_check_membership(&body_pred.name, body_pred, &check_binding) {
                    self.check_membership(&body_pred.name, body_pred, &check_binding)
                } else {
                    self.ensure_computed(&body_pred.name);
                    self.get_all_facts(&body_pred.name).iter().any(|rc_f| {
                        rc_f.predicate == check_fact.predicate && rc_f.args == check_fact.args
                    })
                }
            } else {
                self.get_all_facts(&body_pred.name).iter().any(|rc_f| {
                    rc_f.predicate == check_fact.predicate && rc_f.args == check_fact.args
                })
            };

            if !exists {
                return false;
            }
        }

        true
    }

    fn eval_predicate_with_bindings(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let expanded_predicate = self.expand_predicate_arity(predicate);
        let predicate = &expanded_predicate;

        if predicate.name.starts_with("not:") {
            return self.eval_negation(predicate, current_bindings);
        }

        match predicate.name.as_str() {
            "=" => builtins::eval_equality(predicate, current_bindings),
            "!=" => builtins::eval_inequality(predicate, current_bindings),
            "gt" | "lt" => builtins::eval_comparison(predicate, current_bindings),
            "concat" => builtins::eval_concat(predicate, current_bindings),
            "prefix" => builtins::eval_prefix(predicate, current_bindings),
            "suffix" => builtins::eval_suffix(predicate, current_bindings),
            "substring" => builtins::eval_substring(predicate, current_bindings),
            "contains" => builtins::eval_contains(predicate, current_bindings),
            "strip_prefix" => builtins::eval_strip_prefix(predicate, current_bindings),
            "strip_suffix" => builtins::eval_suffix(predicate, current_bindings),
            "before_char" => builtins::eval_before_char(predicate, current_bindings),
            "after_char" => builtins::eval_after_char(predicate, current_bindings),
            "parse_int" => builtins::eval_parse_int(predicate, current_bindings),
            "to_string" => builtins::eval_to_string(predicate, current_bindings),
            "add" => builtins::eval_add(predicate, current_bindings),
            "sub" => builtins::eval_sub(predicate, current_bindings),
            "mul" => builtins::eval_mul(predicate, current_bindings),
            "div" => builtins::eval_div(predicate, current_bindings),
            "mod" => builtins::eval_mod(predicate, current_bindings),
            "split" => self.eval_split(predicate, current_bindings),
            "matches_glob" => self.eval_matches_glob(predicate, current_bindings),
            "resolve" => self.eval_resolve(predicate, current_bindings),
            "count" => self.eval_count(predicate, current_bindings),
            "min" => self.eval_min(predicate, current_bindings),
            "max" => self.eval_max(predicate, current_bindings),
            "source_location" => self.eval_source_location(predicate, current_bindings),
            _ => {
                if self.rules.contains_key(&predicate.name)
                    && !self.computed.contains_key(&predicate.name)
                {
                    if !current_bindings.is_empty()
                        && self.can_check_membership(
                            &predicate.name,
                            predicate,
                            &current_bindings[0],
                        )
                    {
                        let mut new_bindings = Vec::new();
                        for binding in current_bindings {
                            if self.check_membership(&predicate.name, predicate, &binding) {
                                new_bindings.push(binding);
                            }
                        }
                        return new_bindings;
                    }
                    self.ensure_computed(&predicate.name);
                }

                let facts = self.get_all_facts(&predicate.name);
                let mut new_bindings = Vec::new();
                for binding in current_bindings {
                    for rc_fact in &facts {
                        if let Some(extended) =
                            try_extend_binding(&binding, &predicate.args, &rc_fact.args)
                        {
                            new_bindings.push(extended);
                        }
                    }
                }
                new_bindings
            }
        }
    }

    fn expand_predicate_arity(&self, predicate: &Predicate) -> Predicate {
        if let Some(rules) = self.rules.get(&predicate.name) {
            if let Some(rule) = rules.first() {
                let expected_arity = rule.head.args.len();
                let actual_arity = predicate.args.len();

                if actual_arity < expected_arity {
                    let mut expanded_args = predicate.args.clone();
                    for i in actual_arity..expected_arity {
                        expanded_args.push(Term::Variable(format!("_anon_{}", i)));
                    }
                    return Predicate {
                        name: predicate.name.clone(),
                        args: expanded_args,
                    };
                }
            }
        }

        predicate.clone()
    }

    fn eval_negation(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let inner_pred_name = predicate.name.strip_prefix("not:").unwrap();
        self.ensure_computed(inner_pred_name);
        let inner_facts = self.get_all_facts(inner_pred_name);
        let mut result = Vec::new();

        for binding in current_bindings {
            let mut matches = false;
            for rc_fact in &inner_facts {
                if try_extend_binding(&binding, &predicate.args, &rc_fact.args).is_some() {
                    matches = true;
                    break;
                }
            }
            if !matches {
                result.push(binding);
            }
        }

        result
    }

    fn eval_matches_glob(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 2 {
            eprintln!(
                "Syntax error: matches_glob requires exactly 2 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        current_bindings
            .into_iter()
            .flat_map(|binding| self.eval_matches_glob_binding(predicate, binding))
            .collect()
    }

    fn eval_matches_glob_binding(
        &mut self,
        predicate: &Predicate,
        binding: Arc<HashMap<String, Value>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let (file_term, pattern_term) = (&predicate.args[0], &predicate.args[1]);

        let file_opt = match file_term {
            Term::Variable(v) => binding.get(v).and_then(|val| match val {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            }),
            Term::Constant(Value::String(s)) => Some(s.as_str()),
            _ => return vec![],
        };

        let pattern_opt = match pattern_term {
            Term::Variable(v) => binding.get(v).and_then(|val| match val {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            }),
            Term::Constant(Value::String(s)) => Some(s.as_str()),
            _ => return vec![],
        };

        match (file_opt, pattern_opt) {
            (Some(file), Some(pattern)) => self
                .check_glob_match(file, pattern)
                .then_some(binding)
                .into_iter()
                .collect(),
            (Some(file), None) => self.find_patterns_for_file(file, &binding, pattern_term),
            (None, Some(pattern)) => self.find_files_for_pattern(pattern, &binding, file_term),
            (None, None) => self.find_all_matches(&binding, file_term, pattern_term),
        }
    }

    fn check_glob_match(&self, file: &str, pattern: &str) -> bool {
        glob::Pattern::new(pattern)
            .ok()
            .is_some_and(|p| p.matches(file))
    }

    fn find_patterns_for_file(
        &self,
        file: &str,
        binding: &Arc<HashMap<String, Value>>,
        pattern_term: &Term,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let Term::Variable(pattern_var) = pattern_term else {
            return vec![];
        };

        self.get_all_facts("source_glob")
            .iter()
            .filter_map(|fact| {
                let pattern = fact.args.get(1)?.as_string()?;
                if self.check_glob_match(file, pattern) {
                    let mut new_binding = Arc::clone(binding);
                    Arc::make_mut(&mut new_binding)
                        .insert(pattern_var.clone(), Value::String(pattern.to_string()));
                    Some(new_binding)
                } else {
                    None
                }
            })
            .collect()
    }

    fn find_files_for_pattern(
        &mut self,
        pattern: &str,
        binding: &Arc<HashMap<String, Value>>,
        file_term: &Term,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let Term::Variable(file_var) = file_term else {
            return vec![];
        };

        let Ok(glob_pattern) = glob::Pattern::new(pattern) else {
            return vec![];
        };

        // Check the cache of file_exists facts 
        let mut results: Vec<Arc<HashMap<String, Value>>> = self
            .get_all_facts("file_exists")
            .iter()
            .filter_map(|fact| {
                let file = fact.args.first()?.as_string()?;
                if glob_pattern.matches(file) {
                    let mut new_binding = Arc::clone(binding);
                    Arc::make_mut(&mut new_binding)
                        .insert(file_var.clone(), Value::String(file.to_string()));
                    Some(new_binding)
                } else {
                    None
                }
            })
            .collect();

        // If not in cache, scan the filesystem
        if results.is_empty() {
            if let Ok(paths) = glob::glob(pattern) {
                for path_result in paths.flatten() {
                    if let Some(path_str) = path_result.to_str() {
                        self.insert_facts(vec![Fact {
                            predicate: "file_exists".to_string(),
                            args: vec![Value::String(path_str.to_string())],
                        }]);
                        let mut new_binding = Arc::clone(binding);
                        Arc::make_mut(&mut new_binding)
                            .insert(file_var.clone(), Value::String(path_str.to_string()));
                        results.push(new_binding);
                    }
                }
            }
        }

        results
    }

    fn find_all_matches(
        &self,
        binding: &Arc<HashMap<String, Value>>,
        file_term: &Term,
        pattern_term: &Term,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let (Term::Variable(file_var), Term::Variable(pattern_var)) = (file_term, pattern_term)
        else {
            return vec![];
        };

        let files: Vec<String> = self
            .get_all_facts("file_exists")
            .iter()
            .filter_map(|fact| Some(fact.args.first()?.as_string()?.to_string()))
            .collect();

        let patterns: Vec<String> = self
            .get_all_facts("source_glob")
            .iter()
            .filter_map(|fact| Some(fact.args.get(1)?.as_string()?.to_string()))
            .collect();

        let mut result = Vec::new();
        for pattern_str in patterns {
            let Ok(glob_pattern) = glob::Pattern::new(&pattern_str) else {
                continue;
            };
            for file in &files {
                if glob_pattern.matches(file) {
                    let mut new_binding = Arc::clone(binding);
                    let binding_mut = Arc::make_mut(&mut new_binding);
                    binding_mut.insert(file_var.clone(), Value::String(file.clone()));
                    binding_mut.insert(pattern_var.clone(), Value::String(pattern_str.clone()));
                    result.push(new_binding);
                }
            }
        }
        result
    }

    fn eval_split(
        &self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 4 {
            eprintln!(
                "Syntax error: split requires exactly 4 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        current_bindings
            .into_iter()
            .flat_map(|binding| self.eval_split_binding(predicate, binding))
            .collect()
    }

    fn eval_split_binding(
        &self,
        predicate: &Predicate,
        binding: Arc<HashMap<String, Value>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let (string_term, delim_term, index_term, part_term) = (
            &predicate.args[0],
            &predicate.args[1],
            &predicate.args[2],
            &predicate.args[3],
        );

        let string_opt = self.extract_string_value(string_term, &binding);
        let delim_opt = self.extract_string_value(delim_term, &binding);
        let index_opt = self.extract_int_value(index_term, &binding);
        let part_opt = self.extract_string_value(part_term, &binding);

        let Some(string) = string_opt else {
            return vec![];
        };
        let Some(delim) = delim_opt else {
            return vec![];
        };

        let parts: Vec<&str> = string.split(delim).collect();

        match (index_opt, part_opt) {
            (Some(index), Some(part)) => self
                .check_split_match(&parts, index, part)
                .then_some(binding)
                .into_iter()
                .collect(),
            (Some(index), None) => self.extract_part_at_index(&parts, index, &binding, part_term),
            (None, Some(part)) => self.find_indices_for_part(&parts, part, &binding, index_term),
            (None, None) => self.generate_all_parts(&parts, &binding, index_term, part_term),
        }
    }

    fn extract_string_value<'a>(
        &self,
        term: &'a Term,
        binding: &'a HashMap<String, Value>,
    ) -> Option<&'a str> {
        match term {
            Term::Variable(v) => binding.get(v).and_then(|val| val.as_string()),
            Term::Constant(Value::String(s)) => Some(s),
            _ => None,
        }
    }

    fn extract_int_value(&self, term: &Term, binding: &HashMap<String, Value>) -> Option<i64> {
        match term {
            Term::Variable(v) => binding.get(v).and_then(|val| match val {
                Value::Integer(i) => Some(*i),
                _ => None,
            }),
            Term::Constant(Value::Integer(i)) => Some(*i),
            _ => None,
        }
    }

    fn check_split_match(&self, parts: &[&str], index: i64, part: &str) -> bool {
        if index < 0 || index >= parts.len() as i64 {
            return false;
        }
        parts[index as usize] == part
    }

    fn extract_part_at_index(
        &self,
        parts: &[&str],
        index: i64,
        binding: &Arc<HashMap<String, Value>>,
        part_term: &Term,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let Term::Variable(part_var) = part_term else {
            return vec![];
        };

        if index < 0 || index >= parts.len() as i64 {
            return vec![];
        }

        let mut new_binding = Arc::clone(binding);
        Arc::make_mut(&mut new_binding).insert(
            part_var.clone(),
            Value::String(parts[index as usize].to_string()),
        );
        vec![new_binding]
    }

    fn find_indices_for_part(
        &self,
        parts: &[&str],
        part: &str,
        binding: &Arc<HashMap<String, Value>>,
        index_term: &Term,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let Term::Variable(index_var) = index_term else {
            return vec![];
        };

        parts
            .iter()
            .enumerate()
            .filter(|(_, p)| **p == part)
            .map(|(i, _)| {
                let mut new_binding = Arc::clone(binding);
                Arc::make_mut(&mut new_binding).insert(index_var.clone(), Value::Integer(i as i64));
                new_binding
            })
            .collect()
    }

    fn generate_all_parts(
        &self,
        parts: &[&str],
        binding: &Arc<HashMap<String, Value>>,
        index_term: &Term,
        part_term: &Term,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let (Term::Variable(index_var), Term::Variable(part_var)) = (index_term, part_term) else {
            return vec![];
        };

        parts
            .iter()
            .enumerate()
            .map(|(i, part)| {
                let mut new_binding = Arc::clone(binding);
                let binding_mut = Arc::make_mut(&mut new_binding);
                binding_mut.insert(index_var.clone(), Value::Integer(i as i64));
                binding_mut.insert(part_var.clone(), Value::String(part.to_string()));
                new_binding
            })
            .collect()
    }

    fn eval_resolve(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 2 {
            eprintln!(
                "Syntax error: resolve requires exactly 2 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        current_bindings
            .into_iter()
            .flat_map(|binding| self.eval_resolve_binding(predicate, binding))
            .collect()
    }

    fn eval_resolve_binding(
        &mut self,
        predicate: &Predicate,
        binding: Arc<HashMap<String, Value>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        let (target_term, output_term) = (&predicate.args[0], &predicate.args[1]);

        let target = match target_term {
            Term::Constant(Value::String(s)) => s.as_str(),
            Term::Variable(v) => match binding.get(v) {
                Some(Value::String(s)) => s.as_str(),
                _ => return vec![],
            },
            _ => return vec![],
        };

        let lines = self.resolve_target(target);

        match output_term {
            Term::Variable(var) => lines
                .iter()
                .map(|line| {
                    let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var.clone(), Value::String(line.clone()));
                    new_binding
                })
                .collect(),
            Term::Constant(Value::String(expected)) => {
                if lines.iter().any(|line| line == expected) {
                    vec![binding]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    // Resolve a target by executing a tool with arguments specified in base attr facts.
    // This function ONLY uses base attr facts, NOT derived facts from rules to ensure
    // resolve() is total (no infinite loops) and predictable. If you need attrs derived
    // from rules, ensure those attrs are computed and inserted as base facts first.
    fn resolve_target(&mut self, target: &str) -> Vec<String> {
        if let Some(cached) = self.resolve_cache.get(target) {
            return cached.clone();
        }

        if self.resolve_callback.is_none() {
            self.resolve_cache.insert(target.to_string(), vec![]);
            return vec![];
        }

        let attr_facts = self.query_base_facts("attr", &[Some(target)]);

        let mut tool = None;
        let mut args = std::collections::BTreeMap::new();

        for fact in &attr_facts {
            if fact.args.len() >= 3 {
                if let (Value::String(t), Value::String(key), Value::String(value)) =
                    (&fact.args[0], &fact.args[1], &fact.args[2])
                {
                    if t == target {
                        if key == "tool" {
                            tool = Some(value.clone());
                        } else if let Ok(idx) = key.parse::<usize>() {
                            args.insert(idx, value.clone());
                        }
                    }
                }
            }
        }

        let Some(tool_name) = tool else {
            self.resolve_cache.insert(target.to_string(), vec![]);
            return vec![];
        };

        let arg_vec: Vec<String> = args.values().cloned().collect();

        let callback = self.resolve_callback.as_ref().unwrap();
        let stdout = match callback(target, &tool_name, &arg_vec) {
            Ok(output) => output,
            Err(_) => {
                self.resolve_cache.insert(target.to_string(), vec![]);
                return vec![];
            }
        };

        let lines: Vec<String> = String::from_utf8_lossy(&stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        self.resolve_cache.insert(target.to_string(), lines.clone());
        lines
    }

    fn eval_count(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 2 {
            eprintln!(
                "Syntax error: count requires exactly 2 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        let pred_name_term = &predicate.args[0];
        let count_term = &predicate.args[1];

        current_bindings.into_iter()
            .filter_map(|binding| {
                let pred_name = match pred_name_term {
                    Term::Constant(Value::String(s)) => s.as_str(),
                    Term::Variable(v) => {
                        match binding.get(v) {
                            Some(Value::String(s)) => s.as_str(),
                            _ => {
                                eprintln!("Type error: count first argument must be a string predicate name");
                                std::process::exit(1);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Type error: count first argument must be a string predicate name, got {:?}", pred_name_term);
                        std::process::exit(1);
                    }
                };

                self.ensure_computed(pred_name);
                let facts = self.get_all_facts(pred_name);
                let count = facts.len() as i64;

                match count_term {
                    Term::Variable(var) => {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var.clone(), Value::Integer(count));
                        Some(new_binding)
                    }
                    Term::Constant(Value::Integer(expected)) => {
                        if count == *expected {
                            Some(binding)
                        } else {
                            None
                        }
                    }
                    _ => {
                        eprintln!("Type error: count second argument must be an integer or variable, got {:?}", count_term);
                        std::process::exit(1);
                    }
                }
            })
            .collect()
    }

    fn eval_min(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 3 {
            eprintln!(
                "Syntax error: min requires exactly 3 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        let pred_name_term = &predicate.args[0];
        let arg_index_term = &predicate.args[1];
        let min_term = &predicate.args[2];

        current_bindings.into_iter()
            .filter_map(|binding| {
                let pred_name = match pred_name_term {
                    Term::Constant(Value::String(s)) => s.as_str(),
                    Term::Variable(v) => {
                        match binding.get(v) {
                            Some(Value::String(s)) => s.as_str(),
                            _ => {
                                eprintln!("Type error: min first argument must be a string predicate name");
                                std::process::exit(1);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Type error: min first argument must be a string predicate name, got {:?}", pred_name_term);
                        std::process::exit(1);
                    }
                };

                let arg_index = match arg_index_term {
                    Term::Constant(Value::Integer(i)) => *i as usize,
                    Term::Variable(v) => {
                        match binding.get(v) {
                            Some(Value::Integer(i)) => *i as usize,
                            _ => {
                                eprintln!("Type error: min second argument must be an integer index");
                                std::process::exit(1);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Type error: min second argument must be an integer index, got {:?}", arg_index_term);
                        std::process::exit(1);
                    }
                };

                self.ensure_computed(pred_name);
                let facts = self.get_all_facts(pred_name);

                let min_value = facts.iter()
                    .filter_map(|fact| {
                        if arg_index < fact.args.len() {
                            match &fact.args[arg_index] {
                                Value::Integer(i) => Some(*i),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .min();

                let min = min_value?;

                match min_term {
                    Term::Variable(var) => {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var.clone(), Value::Integer(min));
                        Some(new_binding)
                    }
                    Term::Constant(Value::Integer(expected)) => {
                        if min == *expected {
                            Some(binding)
                        } else {
                            None
                        }
                    }
                    _ => {
                        eprintln!("Type error: min third argument must be an integer or variable, got {:?}", min_term);
                        std::process::exit(1);
                    }
                }
            })
            .collect()
    }

    fn eval_max(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 3 {
            eprintln!(
                "Syntax error: max requires exactly 3 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        let pred_name_term = &predicate.args[0];
        let arg_index_term = &predicate.args[1];
        let max_term = &predicate.args[2];

        current_bindings.into_iter()
            .filter_map(|binding| {
                let pred_name = match pred_name_term {
                    Term::Constant(Value::String(s)) => s.as_str(),
                    Term::Variable(v) => {
                        match binding.get(v) {
                            Some(Value::String(s)) => s.as_str(),
                            _ => {
                                eprintln!("Type error: max first argument must be a string predicate name");
                                std::process::exit(1);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Type error: max first argument must be a string predicate name, got {:?}", pred_name_term);
                        std::process::exit(1);
                    }
                };

                let arg_index = match arg_index_term {
                    Term::Constant(Value::Integer(i)) => *i as usize,
                    Term::Variable(v) => {
                        match binding.get(v) {
                            Some(Value::Integer(i)) => *i as usize,
                            _ => {
                                eprintln!("Type error: max second argument must be an integer index");
                                std::process::exit(1);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Type error: max second argument must be an integer index, got {:?}", arg_index_term);
                        std::process::exit(1);
                    }
                };

                self.ensure_computed(pred_name);
                let facts = self.get_all_facts(pred_name);

                let max_value = facts.iter()
                    .filter_map(|fact| {
                        if arg_index < fact.args.len() {
                            match &fact.args[arg_index] {
                                Value::Integer(i) => Some(*i),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .max();

                let max = max_value?;

                match max_term {
                    Term::Variable(var) => {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var.clone(), Value::Integer(max));
                        Some(new_binding)
                    }
                    Term::Constant(Value::Integer(expected)) => {
                        if max == *expected {
                            Some(binding)
                        } else {
                            None
                        }
                    }
                    _ => {
                        eprintln!("Type error: max third argument must be an integer or variable, got {:?}", max_term);
                        std::process::exit(1);
                    }
                }
            })
            .collect()
    }

    fn eval_source_location(
        &mut self,
        predicate: &Predicate,
        current_bindings: Vec<Arc<HashMap<String, Value>>>,
    ) -> Vec<Arc<HashMap<String, Value>>> {
        if predicate.args.len() != 3 {
            eprintln!(
                "Syntax error: source_location requires exactly 3 arguments, got {}",
                predicate.args.len()
            );
            std::process::exit(1);
        }

        if !self.base_facts.contains_key("source_location") {
            self.materialize_source_locations();
        }

        let all_facts = self.get_all_facts("source_location");

        // Optimize by pre-filtering facts with constant args
        let facts: Vec<_> = all_facts
            .into_iter()
            .filter(|rc_fact| {
                for (i, term) in predicate.args.iter().enumerate() {
                    if let Term::Constant(const_val) = term {
                        if i >= rc_fact.args.len() || &rc_fact.args[i] != const_val {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        let mut result = Vec::new();

        for binding in current_bindings {
            for rc_fact in &facts {
                if let Some(extended) = try_extend_binding(&binding, &predicate.args, &rc_fact.args)
                {
                    result.push(extended);
                }
            }
        }

        result
    }

    fn query_base_facts(&self, predicate: &str, filters: &[Option<&str>]) -> Vec<Rc<Fact>> {
        let Some((fact_vec, idx_ptr)) = self.base_facts.get(predicate) else {
            return vec![];
        };

        let first_filter = filters.first().and_then(|f| f.as_ref());
        let idx = idx_ptr.and_then(|i| {
            if first_filter.is_some() {
                Some(i)
            } else {
                None
            }
        });

        let rc_facts: Vec<Rc<Fact>> = if let (Some(idx), Some(filter)) = (idx, first_filter) {
            let (index_cell, _) = &self.indices[idx];
            Self::build_index_if_needed(fact_vec, index_cell);
            let index_ref = index_cell.borrow();
            if let Some(ref index) = *index_ref {
                if let Some(indices) = index.get(*filter) {
                    indices.iter().map(|&i| Rc::clone(&fact_vec[i])).collect()
                } else {
                    return vec![];
                }
            } else {
                return vec![];
            }
        } else {
            fact_vec.iter().map(Rc::clone).collect()
        };

        apply_filters(rc_facts, filters)
    }

    fn detect_scc(&self, start: &str) -> Option<Vec<String>> {
        if !self.rules.contains_key(start) {
            return None;
        }

        if self.is_transitive_closure_predicate(start) {
            return None;
        }

        let mut visited = std::collections::HashSet::new();
        let mut rec_stack: Vec<String> = Vec::new();

        fn dfs(
            engine: &Engine,
            start: &str,
            pred: &str,
            visited: &mut std::collections::HashSet<String>,
            rec_stack: &mut Vec<String>,
        ) -> Option<Vec<String>> {
            if visited.contains(pred) {
                return None;
            }

            if let Some(pos) = rec_stack.iter().position(|p| p == pred) {
                let scc = rec_stack[pos..].to_vec();
                if scc.contains(&start.to_string()) {
                    return Some(scc);
                }
                return None;
            }

            rec_stack.push(pred.to_string());

            if let Some(rules) = engine.rules.get(pred) {
                for rule in rules {
                    for body_pred in &rule.body {
                        if Engine::is_special_predicate(&body_pred.name) {
                            continue;
                        }
                        if engine.is_transitive_closure_predicate(&body_pred.name) {
                            continue;
                        }
                        if engine.rules.contains_key(&body_pred.name) {
                            if let Some(scc) =
                                dfs(engine, start, &body_pred.name, visited, rec_stack)
                            {
                                return Some(scc);
                            }
                        }
                    }
                }
            }

            rec_stack.pop();
            visited.insert(pred.to_string());
            None
        }

        dfs(self, start, start, &mut visited, &mut rec_stack)
    }

    fn ensure_computed(&mut self, predicate: &str) {
        if self.computed.contains_key(predicate) {
            return;
        }

        if !self.rules.contains_key(predicate) {
            return;
        }

        if let Some(scc) = self.detect_scc(predicate) {
            let results = self.evaluate_fixpoint(&scc);
            for pred in &scc {
                if let Some(facts) = results.get(pred) {
                    self.computed.insert(pred.clone(), facts.clone());
                }
            }
        } else {
            let results = self.evaluate_fixpoint(&[predicate.to_string()]);
            if let Some(facts) = results.get(predicate) {
                self.computed.insert(predicate.to_string(), facts.clone());
            }
        }
    }

    fn get_all_facts(&self, predicate: &str) -> Vec<Rc<Fact>> {
        if let Some(computed) = self.computed.get(predicate) {
            return computed.iter().map(Rc::clone).collect();
        }
        if let Some((fact_vec, _)) = self.base_facts.get(predicate) {
            return fact_vec.iter().map(Rc::clone).collect();
        }
        Vec::new()
    }

    fn evaluate_fixpoint(&mut self, predicates: &[String]) -> HashMap<String, Vec<Rc<Fact>>> {
        let pred_set: std::collections::HashSet<String> = predicates.iter().cloned().collect();

        let mut base_rules: Vec<Rule> = Vec::new();
        let mut recursive_rules: Vec<Rule> = Vec::new();
        let mut referenced_predicates = std::collections::HashSet::new();

        for pred in predicates {
            referenced_predicates.insert(pred.clone());
            if let Some(rules) = self.rules.get(pred) {
                for rule in rules {
                    let depends_on_predicates = rule.body.iter().any(|body_pred| {
                        !Self::is_special_predicate(&body_pred.name)
                            && pred_set.contains(&body_pred.name)
                    });

                    for body_pred in &rule.body {
                        if !Self::is_special_predicate(&body_pred.name) {
                            referenced_predicates.insert(body_pred.name.clone());
                        }
                    }

                    if depends_on_predicates {
                        recursive_rules.push(rule.clone());
                    } else {
                        base_rules.push(rule.clone());
                    }
                }
            }
        }

        // Ensure all referenced predicates outside the SCC are computed first
        for pred in &referenced_predicates {
            if !pred_set.contains(pred) {
                self.ensure_computed(pred);
            }
        }

        let mut all_facts: HashMap<String, Vec<Rc<Fact>>> = HashMap::new();
        for pred in &referenced_predicates {
            // Check if already computed
            if let Some(computed_facts) = self.computed.get(pred) {
                all_facts.insert(pred.clone(), computed_facts.iter().map(Rc::clone).collect());
            } else if let Some((fact_vec, _)) = self.base_facts.get(pred) {
                all_facts.insert(pred.clone(), fact_vec.iter().map(Rc::clone).collect());
            } else {
                all_facts.insert(pred.clone(), Vec::new());
            }
        }

        let mut all_facts_set: HashMap<String, HashSet<Rc<Fact>>> = HashMap::new();
        for pred in &referenced_predicates {
            let facts = all_facts.get(pred).unwrap();
            all_facts_set.insert(pred.clone(), facts.iter().cloned().collect());
        }

        let mut delta: HashMap<String, Vec<Rc<Fact>>> = HashMap::new();
        for pred in predicates {
            if let Some(facts) = all_facts.get(pred) {
                if !facts.is_empty() {
                    delta.insert(pred.clone(), facts.clone());
                }
            }
        }

        for rule in &base_rules {
            let new_facts = self.evaluate_rule(rule);
            let pred = &rule.head.name;
            for fact in new_facts {
                if !all_facts_set.get(pred).unwrap().contains(&fact) {
                    delta.entry(pred.clone()).or_default().push(fact.clone());
                    all_facts_set.get_mut(pred).unwrap().insert(fact.clone());
                    all_facts.get_mut(pred).unwrap().push(fact);
                }
            }
        }

        let max_iterations = self
            .base_facts
            .get("max_iterations")
            .and_then(|(facts, _)| facts.first())
            .and_then(|fact| fact.args.first())
            .and_then(|arg| {
                if let Value::Integer(n) = arg {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(1000);

        let mut final_iteration = 0;
        for iteration in 0..max_iterations {
            if delta.values().all(|v| v.is_empty()) {
                final_iteration = iteration;
                break;
            }

            let mut new_delta: HashMap<String, Vec<Rc<Fact>>> = HashMap::new();
            let mut new_delta_set: HashMap<String, HashSet<Rc<Fact>>> = HashMap::new();

            for rule in &recursive_rules {
                let new_facts = self.evaluate_rule_with_delta(rule, &all_facts, &delta, &pred_set);
                let pred = &rule.head.name;

                let existing_set = all_facts_set.get(pred).unwrap();
                let delta_entry = new_delta.entry(pred.clone()).or_default();
                let delta_set_entry = new_delta_set.entry(pred.clone()).or_default();
                for rc_fact in new_facts {
                    if !existing_set.contains(&rc_fact) && !delta_set_entry.contains(&rc_fact) {
                        delta_entry.push(rc_fact.clone());
                        delta_set_entry.insert(rc_fact);
                    }
                }
            }

            if new_delta.is_empty() {
                final_iteration = iteration + 1;
                break;
            }

            for (pred, facts) in &new_delta {
                all_facts
                    .get_mut(pred)
                    .unwrap()
                    .extend(facts.iter().map(Rc::clone));
                all_facts_set
                    .get_mut(pred)
                    .unwrap()
                    .extend(facts.iter().cloned());
            }

            delta = new_delta;

            if iteration == max_iterations - 1 {
                eprintln!(
                    "Error: Fixpoint evaluation exceeded {} iterations for predicates: {:?}",
                    max_iterations, predicates
                );
                eprintln!("This indicates non-terminating rules or excessively deep recursion.");
                eprintln!(
                    "Increase the limit with: max_iterations({}).",
                    max_iterations * 10
                );
                std::process::exit(1);
            }
        }

        if final_iteration > max_iterations / 2 {
            eprintln!(
                "Warning: Fixpoint evaluation for {:?} took {} iterations (limit: {})",
                predicates, final_iteration, max_iterations
            );
            eprintln!("Consider increasing max_iterations or checking for inefficient rules.");
        }

        all_facts
    }

    fn evaluate_rule_with_delta(
        &mut self,
        rule: &Rule,
        all_facts: &HashMap<String, Vec<Rc<Fact>>>,
        delta: &HashMap<String, Vec<Rc<Fact>>>,
        pred_set: &std::collections::HashSet<String>,
    ) -> Vec<Rc<Fact>> {
        let scc_predicate_indices: Vec<usize> = rule
            .body
            .iter()
            .enumerate()
            .filter_map(|(i, pred)| {
                if !Self::is_special_predicate(&pred.name) && pred_set.contains(&pred.name) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        // If no SCC predicates in body, evaluate normally with all_facts
        if scc_predicate_indices.is_empty() {
            return self.evaluate_rule_with_facts(rule, all_facts);
        }

        // Generate a variant for each SCC predicate using delta
        let mut all_results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for &delta_idx in &scc_predicate_indices {
            let mut bindings = vec![Arc::new(HashMap::new())];

            for (i, predicate) in rule.body.iter().enumerate() {
                if Self::is_special_predicate(&predicate.name) {
                    bindings = self.eval_predicate_with_bindings(predicate, bindings);
                } else {
                    // Use delta only for the selected predicate, all_facts for others
                    let facts_to_use = if i == delta_idx {
                        delta.get(&predicate.name).map(Vec::as_slice).unwrap_or(&[])
                    } else {
                        all_facts.get(&predicate.name).map(Vec::as_slice).unwrap_or(&[])
                    };

                    let mut new_bindings = Vec::new();
                    for binding in bindings {
                        for rc_fact in facts_to_use {
                            if let Some(extended) =
                                try_extend_binding(&binding, &predicate.args, &rc_fact.args)
                            {
                                new_bindings.push(extended);
                            }
                        }
                    }
                    bindings = new_bindings;
                }

                if bindings.is_empty() {
                    break;
                }
            }

            for binding in bindings {
                let args = project_to_head(&rule.head.args, &binding);
                let fact = Fact::new(&rule.head.name, args);
                if seen.insert(fact.clone()) {
                    all_results.push(Rc::new(fact));
                }
            }
        }

        all_results
    }

    fn evaluate_rule_with_facts(
        &mut self,
        rule: &Rule,
        facts: &HashMap<String, Vec<Rc<Fact>>>,
    ) -> Vec<Rc<Fact>> {
        let mut bindings = vec![Arc::new(HashMap::new())];

        for predicate in &rule.body {
            if Self::is_special_predicate(&predicate.name) {
                bindings = self.eval_predicate_with_bindings(predicate, bindings);
            } else {
                let facts_to_use = facts.get(&predicate.name).map(Vec::as_slice).unwrap_or(&[]);
                let mut new_bindings = Vec::new();
                for binding in bindings {
                    for rc_fact in facts_to_use {
                        if let Some(extended) =
                            try_extend_binding(&binding, &predicate.args, &rc_fact.args)
                        {
                            new_bindings.push(extended);
                        }
                    }
                }
                bindings = new_bindings;
            }

            if bindings.is_empty() {
                return Vec::new();
            }
        }

        bindings
            .into_iter()
            .map(|binding| {
                let args = project_to_head(&rule.head.args, &binding);
                Rc::new(Fact::new(&rule.head.name, args))
            })
            .collect()
    }

    pub fn query_for_target(&mut self, predicate: &str, target: &str) -> Vec<Rc<Fact>> {
        self.query(predicate, &[Some(target)])
    }

    pub fn query_attr(&mut self, target: &str, key: &str) -> Option<String> {
        let facts = self.query("attr", &[Some(target), Some(key)]);
        facts.first().and_then(|f| {
            if let Some(Value::String(v)) = f.args.get(2) {
                Some(v.clone())
            } else {
                None
            }
        })
    }

    pub fn query_sources(&mut self, target: &str) -> Vec<String> {
        self.query("sources", &[Some(target)])
            .iter()
            .filter_map(|f| {
                if let Some(Value::String(path)) = f.args.get(1) {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

fn try_extend_binding(
    existing: &Arc<HashMap<String, Value>>,
    pattern: &[Term],
    values: &[Value],
) -> Option<Arc<HashMap<String, Value>>> {
    if pattern.len() < values.len() {
        return None;
    }

    let mut binding = Arc::clone(existing);
    let mut needs_modification = false;
    let mut temp_bindings = HashMap::new();

    for (i, term) in pattern.iter().enumerate() {
        if i >= values.len() {
            if let Term::Variable(var) = term {
                if var.starts_with("_anon_") {
                    continue;
                }
            }
            return None;
        }

        let value = &values[i];
        match term {
            Term::Variable(var) => {
                if let Some(existing_value) = binding.get(var) {
                    if existing_value != value {
                        return None;
                    }
                } else if let Some(temp_value) = temp_bindings.get(var) {
                    if temp_value != value {
                        return None;
                    }
                } else {
                    temp_bindings.insert(var.clone(), value.clone());
                    needs_modification = true;
                }
            }
            Term::Constant(const_val) => {
                if const_val != value {
                    return None;
                }
            }
        }
    }

    // This clones HashMap to create a mut pointer
    if needs_modification {
        let binding_mut = Arc::make_mut(&mut binding);
        for (var, value) in temp_bindings {
            binding_mut.insert(var, value);
        }
    }

    Some(binding)
}

fn project_to_head(head_args: &[Term], binding: &HashMap<String, Value>) -> Vec<Value> {
    head_args
        .iter()
        .map(|term| match term {
            Term::Variable(var) => binding
                .get(var)
                .cloned()
                .unwrap_or(Value::String("?".to_string())),
            Term::Constant(val) => val.clone(),
        })
        .collect()
}

fn apply_filters(facts: Vec<Rc<Fact>>, filters: &[Option<&str>]) -> Vec<Rc<Fact>> {
    if filters.is_empty() || filters.iter().all(|f| f.is_none()) {
        return facts;
    }

    facts
        .into_iter()
        .filter(|rc_fact| {
            for (i, filter_opt) in filters.iter().enumerate() {
                if let Some(filter_val) = filter_opt {
                    if i >= rc_fact.args.len() {
                        return false;
                    }
                    match &rc_fact.args[i] {
                        Value::String(s) if s != filter_val => return false,
                        Value::Integer(n) => {
                            if let Ok(filter_int) = filter_val.parse::<i64>() {
                                if *n != filter_int {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                        }
                        _ => {}
                    }
                }
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datalog::parser;

    #[test]
    fn basic_insert_and_query() {
        let mut db = Engine::new();

        db.insert_facts(vec![Fact {
            predicate: "test".to_string(),
            args: vec![Value::String("value".to_string())],
        }]);

        let results = db.query("test", &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].predicate, "test");
    }

    #[test]
    fn simple_rule() {
        let mut db = Engine::new();

        db.insert_facts(vec![Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("rust_binary".to_string()),
            ],
        }]);

        db.compile_rule(Rule {
            head: Predicate {
                name: "rust_target".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            body: vec![Predicate {
                name: "kind".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Constant(Value::String("rust_binary".to_string())),
                ],
            }],
        });

        let results = db.query("rust_target", &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
    }

    #[test]
    fn self_join_rule() {
        let mut db = Engine::new();

        db.insert_facts(vec![
            Fact {
                predicate: "edge".to_string(),
                args: vec![
                    Value::String("a".to_string()),
                    Value::String("b".to_string()),
                ],
            },
            Fact {
                predicate: "edge".to_string(),
                args: vec![
                    Value::String("c".to_string()),
                    Value::String("c".to_string()),
                ],
            },
        ]);

        db.compile_rule(Rule {
            head: Predicate {
                name: "self_loop".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            body: vec![Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("X".to_string()),
                ],
            }],
        });

        let results = db.query("self_loop", &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].args[0], Value::String("c".to_string()));
    }

    #[test]
    fn source_location_tracking() {
        let input = r#"
system_cc("//app:main").
sources("//app:main", "main.c").
deps(X, Y) :- depends_on(X, Y).
"#;

        let (facts, rules, locations) =
            parser::parse_program_with_file(input, "BUILD.datalog").unwrap();

        let mut db = Engine::new();
        for (key, loc) in locations {
            db.record_source_location(&key, loc);
        }
        db.insert_facts(facts);
        for rule in rules {
            db.compile_rule(rule);
        }

        let results = db.query("source_location", &[]);
        assert!(!results.is_empty(), "Should have source location entries");

        let app_main_locs: Vec<_> = results
            .iter()
            .filter(|f| {
                if let Some(Value::String(target)) = f.args.first() {
                    target == "//app:main"
                } else {
                    false
                }
            })
            .collect();

        assert!(
            !app_main_locs.is_empty(),
            "Should find source location for //app:main target"
        );

        if let Some(loc) = app_main_locs.first() {
            assert_eq!(loc.args[1], Value::String("BUILD.datalog".to_string()));
            if let Value::Integer(line) = loc.args[2] {
                assert!((1..=4).contains(&line), "Line number should be reasonable");
            } else {
                panic!("Expected line number to be an integer");
            }
        }
    }

    #[test]
    fn source_location_query_with_filters() {
        let mut db = Engine::new();
        db.record_source_location(
            "system_cc",
            SourceLocation {
                file: "BUILD.datalog".to_string(),
                line: 5,
            },
        );
        db.record_source_location(
            "sources",
            SourceLocation {
                file: "BUILD.datalog".to_string(),
                line: 10,
            },
        );
        db.record_source_location(
            "deps",
            SourceLocation {
                file: "other.datalog".to_string(),
                line: 3,
            },
        );

        let all = db.query("source_location", &[]);
        assert_eq!(all.len(), 3);

        let system_cc_only = db.query("source_location", &[Some("system_cc")]);
        assert_eq!(system_cc_only.len(), 1);
        assert_eq!(system_cc_only[0].args[2], Value::Integer(5));

        let build_file_only = db.query("source_location", &[None, Some("BUILD.datalog")]);
        assert_eq!(build_file_only.len(), 2);
    }

    #[test]
    fn detect_scc_returns_correct_component() {
        let mut db = Engine::new();

        db.insert_facts(vec![Fact::new(
            "target",
            vec![Value::String("//app:main".to_string())],
        )]);

        db.compile_rule(Rule::new(
            Predicate {
                name: "alias".to_string(),
                args: vec![
                    Term::Variable("A".to_string()),
                    Term::Variable("FinalTarget".to_string()),
                ],
            },
            vec![
                Predicate {
                    name: "alias".to_string(),
                    args: vec![
                        Term::Variable("A".to_string()),
                        Term::Variable("B".to_string()),
                    ],
                },
                Predicate {
                    name: "alias".to_string(),
                    args: vec![
                        Term::Variable("B".to_string()),
                        Term::Variable("FinalTarget".to_string()),
                    ],
                },
            ],
        ));

        // make target depend on alias
        db.compile_rule(Rule::new(
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("Alias".to_string())],
            },
            vec![
                Predicate {
                    name: "alias".to_string(),
                    args: vec![
                        Term::Variable("Alias".to_string()),
                        Term::Variable("Target".to_string()),
                    ],
                },
                Predicate {
                    name: "target".to_string(),
                    args: vec![Term::Variable("Target".to_string())],
                },
            ],
        ));

        let results = db.query("target", &[]);

        assert_eq!(results.len(), 1, "Should preserve base target fact");
        assert_eq!(results[0].args[0], Value::String("//app:main".to_string()));
    }

    #[test]
    fn test_semi_naive_multi_predicate_scc() {
        let mut engine = Engine::new();

        engine.insert_facts(vec![
            Fact::new(
                "edge",
                vec![Value::String("a".into()), Value::String("b".into())],
            ),
            Fact::new(
                "edge",
                vec![Value::String("b".into()), Value::String("c".into())],
            ),
        ]);

        // path(X,Y) :- edge(X,Y).
        engine.compile_rule(Rule::new(
            Predicate {
                name: "path".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
            vec![Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            }],
        ));

        // path(X,Z) :- path(X,Y), path(Y,Z).
        engine.compile_rule(Rule::new(
            Predicate {
                name: "path".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            vec![
                Predicate {
                    name: "path".to_string(),
                    args: vec![
                        Term::Variable("X".to_string()),
                        Term::Variable("Y".to_string()),
                    ],
                },
                Predicate {
                    name: "path".to_string(),
                    args: vec![
                        Term::Variable("Y".to_string()),
                        Term::Variable("Z".to_string()),
                    ],
                },
            ],
        ));

        let results = engine.query("path", &[Some("a")]);

        // Should find: path(a,b), path(a,c), path(b,c)
        assert!(
            results.len() >= 2,
            "Expected at least 2 path facts, got {}",
            results.len()
        );
        assert!(results
            .iter()
            .any(|f| f.args == vec![Value::String("a".into()), Value::String("b".into())]));
        assert!(results
            .iter()
            .any(|f| f.args == vec![Value::String("a".into()), Value::String("c".into())]));
    }

    #[test]
    fn test_index_consistency_after_insert_retract() {
        let mut db = Engine::new();

        let fact1 = Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ],
        };
        let fact2 = Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
        };
        db.insert_facts(vec![fact1.clone(), fact2.clone()]);

        db.compile_rule(Rule {
            head: Predicate {
                name: "path".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
            body: vec![Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            }],
        });

        let results_before = db.query("path", &[]);
        assert_eq!(
            results_before.len(),
            2,
            "Should have 2 path facts initially"
        );

        let fact3 = Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("c".to_string()),
                Value::String("d".to_string()),
            ],
        };
        db.insert_facts(vec![fact3.clone()]);

        let results_after_insert = db.query("path", &[]);
        assert_eq!(
            results_after_insert.len(),
            3,
            "Should have 3 path facts after insert"
        );

        db.retract_facts(vec![fact2]);

        let results_after_retract = db.query("path", &[]);
        assert_eq!(
            results_after_retract.len(),
            2,
            "Should have 2 path facts after retract"
        );

        let has_a_b = results_after_retract
            .iter()
            .any(|f| f.args == vec![Value::String("a".into()), Value::String("b".into())]);
        let has_c_d = results_after_retract
            .iter()
            .any(|f| f.args == vec![Value::String("c".into()), Value::String("d".into())]);
        let has_b_c = results_after_retract
            .iter()
            .any(|f| f.args == vec![Value::String("b".into()), Value::String("c".into())]);

        assert!(has_a_b, "Should still have edge(a,b)");
        assert!(has_c_d, "Should still have edge(c,d)");
        assert!(!has_b_c, "Should not have edge(b,c) after retraction");
    }

    #[test]
    fn test_lazy_tc_iterator() {
        let mut db = Engine::new();

        db.insert_facts(vec![
            Fact {
                predicate: "edge".to_string(),
                args: vec![Value::String("a".into()), Value::String("b".into())],
            },
            Fact {
                predicate: "edge".to_string(),
                args: vec![Value::String("b".into()), Value::String("c".into())],
            },
            Fact {
                predicate: "edge".to_string(),
                args: vec![Value::String("c".into()), Value::String("d".into())],
            },
        ]);

        db.compile_rule(Rule {
            head: Predicate {
                name: "path".to_string(),
                args: vec![Term::Variable("X".into()), Term::Variable("Y".into())],
            },
            body: vec![Predicate {
                name: "edge".to_string(),
                args: vec![Term::Variable("X".into()), Term::Variable("Y".into())],
            }],
        });

        db.compile_rule(Rule {
            head: Predicate {
                name: "path".to_string(),
                args: vec![Term::Variable("X".into()), Term::Variable("Z".into())],
            },
            body: vec![
                Predicate {
                    name: "edge".to_string(),
                    args: vec![Term::Variable("X".into()), Term::Variable("Y".into())],
                },
                Predicate {
                    name: "path".to_string(),
                    args: vec![Term::Variable("Y".into()), Term::Variable("Z".into())],
                },
            ],
        });

        let iter = db.query_tc_iter("path", "a");
        let results: Vec<_> = iter.collect();

        assert_eq!(results.len(), 3, "Should have 3 reachable nodes from 'a'");

        let result_set: std::collections::HashSet<_> = results
            .iter()
            .map(|f| {
                assert_eq!(f.predicate, "path");
                assert_eq!(f.args[0], Value::String("a".into()));
                f.args[1].clone()
            })
            .collect();

        assert!(result_set.contains(&Value::String("b".into())));
        assert!(result_set.contains(&Value::String("c".into())));
        assert!(result_set.contains(&Value::String("d".into())));

        let mut iter = db.query_tc_iter("path", "a");
        let first = iter.next();
        assert!(first.is_some(), "Should have at least one result");

        let remaining_count = iter.count();
        assert_eq!(remaining_count, 2, "Should have 2 remaining results");
    }
}
