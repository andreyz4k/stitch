use crate::*;
use compression::*;
use lambdas::*;
use rustc_hash::{FxHashMap, FxHashSet};

/// a rule for determining when to shift and by how much.
/// if anything points above the `depth_cutoff` (absolute depth
/// of lambdas from the top of the program) it gets shifted by `shift`. If
/// more than one ShiftRule applies then more then one shift will happen.
#[derive(Clone, Debug)]
struct ShiftRule {
    depth_cutoff: i32,
    shift: i32,
}

pub fn rewrite_fast(
    pattern: &FinishedPattern,
    shared: &SharedData,
    inv_name: &Node,
    cost_fn: &ExprCost,
) -> Vec<ExprOwned> {
    //  if !shared.cfg.quiet { println!("rewriting with {}", pattern.info(&shared)) }
    #[allow(clippy::too_many_arguments)]
    fn helper(
        owned_set: &mut ExprSet,
        new_vars_mapping: &mut FxHashMap<Symbol, Idx>,
        used_vars: &mut Vec<Symbol>,
        unused_vars: &mut FxHashSet<Symbol>,
        pattern: &FinishedPattern,
        shared: &SharedData,
        unshifted_id: Idx,
        total_depth: i32, // depth from the very root of the program down
        shift_rules: &mut Vec<ShiftRule>,
        inv_name: &Node,
        should_skip: bool,
    ) -> Idx {
        // println!("next @ {}: {}", total_depth, shared.set.get(unshifted_id));

        // we search using the the *unshifted* one since its an original program tree node
        if !should_skip && pattern.pattern.match_locations.binary_search_by(|(n, _)| n.cmp(&unshifted_id)).is_ok() // if the pattern matches here
           && (!pattern.util_calc.corrected_utils.contains_key(&unshifted_id) // and either we have no conflict (ie corrected_utils doesnt have an entry)
             || pattern.util_calc.corrected_utils[&unshifted_id])
        // or we have a conflict but we choose to accept it (which is contextless in this top down approach so its the right move)
        //    && !pattern.pattern.first_zid_of_ivar.iter().any(|zid| // and there are no negative vars anywhere in the arguments
        //         shared.egraph[shared.arg_of_zid_node[*zid][&unshifted_id].Idx].data.free_vars.iter().any(|var| *var < 0))
        {
            //  if !shared.cfg.quiet { println!("inv applies at unshifted={} with shift={}", extract(unshifted_id,&shared.egraph), shift) }
            let mut expr = (*owned_set).add(inv_name.clone());
            // wrap the prim in all the Apps to args
            for zid in pattern.pattern.first_zid_of_ivar.iter() {
                let arg: &Arg = &shared.arg_of_zid_node[*zid][&unshifted_id];

                if arg.shift != 0 {
                    shift_rules.push(ShiftRule {
                        depth_cutoff: total_depth,
                        shift: arg.shift,
                    });
                    // println!("pushing shift rule: {:?}", shift_rules.last().unwrap());
                }
                // println!("rewriting arg: {}", shared.set.get(arg.unshifted_id));
                // recurse with the shift added (subtracted since it's negative) to the depth
                let mut rewritten_arg = helper(
                    owned_set,
                    new_vars_mapping,
                    used_vars,
                    unused_vars,
                    pattern,
                    shared,
                    arg.unshifted_id,
                    total_depth - arg.shift,
                    shift_rules,
                    inv_name,
                    should_skip,
                );
                if arg.shift != 0 {
                    // println!("popping shift rule");
                    shift_rules.pop(); // pop the rule back off after
                }

                if shared.cfg.eta_long {
                    // assuming eta long form in the original corpus, the number of times we need to insert new apps() via eta expanding
                    // is the number of apps directly up and to the right of this argument - ie how many Funcs are at the end of the zipper to
                    // this argument?
                    // Also note that in the single_hole code --eta-long enforces that match locations never contains anything that starts to the left of a func so
                    // we dont need to worry about the case where the zipper would extend even past the root of the match location
                    // Also note that due to beta normal form, this will be zero and will be a no-op if the arg is a lambda
                    let arity_of_arg = shared.zip_of_zid[*zid]
                        .iter()
                        .rev()
                        .take_while(|znode| **znode == ZNode::Func)
                        .count();
                    if arity_of_arg > 0 {
                        let analyzed_free_vars = &mut AnalyzedExpr::new(FreeVarAnalysis);

                        // shift body by arity_of_arg
                        let mut shifted_rewritten_arg = owned_set.get_mut(rewritten_arg).shift(
                            arity_of_arg as i32,
                            0,
                            analyzed_free_vars,
                        );

                        // wrap like: f => (f $1 $0)
                        for i in 0..arity_of_arg {
                            // Just hardcode -1 here, there's no way to infer the tag of the new var
                            // In practice, eta long form should not be used with tagged inputs
                            let var = owned_set.add(Node::Var((arity_of_arg - i - 1) as i32, -1));
                            shifted_rewritten_arg =
                                owned_set.add(Node::App(shifted_rewritten_arg, var));
                        }
                        // wrap in lambdas: (f $1 $0) => (lam (lam (f $1 $0)))
                        for _ in 0..arity_of_arg {
                            // Just hardcode -1 here, there's no way to infer the tag of the new var
                            // In practice, eta long form should not be used with tagged inputs
                            shifted_rewritten_arg =
                                owned_set.add(Node::Lam(shifted_rewritten_arg, -1));
                        }

                        rewritten_arg = shifted_rewritten_arg;
                    }
                }

                expr = owned_set.add(Node::App(expr, rewritten_arg));
            }

            // println!("rewrote: {} -> {}", shared.set.get(unshifted_id), owned_set.get(expr));

            return expr;
        }

        match &shared.set[unshifted_id] {
            Node::Prim(p) => owned_set.add(Node::Prim(p.clone())),
            Node::Var(i, tag) => {
                let mut j = *i;
                for rule in shift_rules.iter() {
                    // take "i" steps upward from current depth and see if you meet or pass the cutoff.
                    // exactly equaling the cutoff counts as needing shifting.

                    // so for example ShiftRule { depth_cutoff: 2, shift: -1 }
                    if total_depth - i <= rule.depth_cutoff {
                        j += rule.shift;
                    }
                }

                assert!(j >= 0);
                owned_set.add(Node::Var(j, *tag))
            } // we extract from the *shifted* one since thats the real one
            Node::App(unshifted_f, unshifted_x) => {
                let f = helper(
                    owned_set,
                    new_vars_mapping,
                    used_vars,
                    unused_vars,
                    pattern,
                    shared,
                    *unshifted_f,
                    total_depth,
                    shift_rules,
                    inv_name,
                    should_skip,
                );
                let x = helper(
                    owned_set,
                    new_vars_mapping,
                    used_vars,
                    unused_vars,
                    pattern,
                    shared,
                    *unshifted_x,
                    total_depth,
                    shift_rules,
                    inv_name,
                    should_skip,
                );
                owned_set.add(Node::App(f, x))
            }
            Node::Lam(unshifted_b, tag) => {
                let b = helper(
                    owned_set,
                    new_vars_mapping,
                    used_vars,
                    unused_vars,
                    pattern,
                    shared,
                    *unshifted_b,
                    total_depth + 1,
                    shift_rules,
                    inv_name,
                    should_skip,
                );
                owned_set.add(Node::Lam(b, *tag))
            }
            Node::IVar(_) => {
                unreachable!()
            }
            Node::NVar(name) => {
                if !used_vars.contains(name) {
                    used_vars.push(name.clone());
                }
                owned_set.add(Node::NVar(name.clone()))
            }
            Node::NLinkVar(name, _link) => {
                if !used_vars.contains(name) {
                    used_vars.push(name.clone());
                }
                let new_link = new_vars_mapping.get(name).unwrap();
                owned_set.add(Node::NLinkVar(name.clone(), *new_link))
            }
            Node::Let {
                var,
                def: unshifted_def,
                body: unshifted_body,
            } => {
                let should_skip = should_skip
                    || pattern
                        .util_calc
                        .skip_rewrite_nested
                        .contains(&unshifted_id);
                if unused_vars.contains(var) {
                    helper(
                        owned_set,
                        new_vars_mapping,
                        used_vars,
                        unused_vars,
                        pattern,
                        shared,
                        *unshifted_body,
                        total_depth,
                        shift_rules,
                        inv_name,
                        should_skip,
                    )
                } else {
                    let def = helper(
                        owned_set,
                        new_vars_mapping,
                        used_vars,
                        unused_vars,
                        pattern,
                        shared,
                        *unshifted_def,
                        total_depth,
                        shift_rules,
                        inv_name,
                        should_skip,
                    );
                    new_vars_mapping.insert(var.clone(), def);
                    let body = helper(
                        owned_set,
                        new_vars_mapping,
                        used_vars,
                        unused_vars,
                        pattern,
                        shared,
                        *unshifted_body,
                        total_depth,
                        shift_rules,
                        inv_name,
                        should_skip,
                    );
                    if !used_vars.contains(var) {
                        unused_vars.insert(var.clone());
                    }
                    owned_set.add(Node::Let {
                        var: var.clone(),
                        def,
                        body,
                    })
                }
            }
            Node::RevLet {
                inp_var,
                def_vars,
                def: unshifted_def,
                body: unshifted_body,
            } => {
                if unused_vars.contains(inp_var) {
                    let body = helper(
                        owned_set,
                        new_vars_mapping,
                        used_vars,
                        unused_vars,
                        pattern,
                        shared,
                        *unshifted_body,
                        total_depth,
                        shift_rules,
                        inv_name,
                        should_skip,
                    );
                    for var in def_vars.iter() {
                        unused_vars.insert(var.clone());
                    }
                    body
                } else {
                    let body = helper(
                        owned_set,
                        new_vars_mapping,
                        used_vars,
                        unused_vars,
                        pattern,
                        shared,
                        *unshifted_body,
                        total_depth,
                        shift_rules,
                        inv_name,
                        should_skip,
                    );
                    let mut def_used_vars: Vec<Symbol> = vec![];
                    let def = helper(
                        owned_set,
                        new_vars_mapping,
                        &mut def_used_vars,
                        unused_vars,
                        pattern,
                        shared,
                        *unshifted_def,
                        total_depth,
                        shift_rules,
                        inv_name,
                        should_skip,
                    );
                    for var in def_vars.iter() {
                        if !def_used_vars.contains(var) {
                            unused_vars.insert(var.clone());
                        }
                    }
                    for var in def_used_vars.iter() {
                        if unused_vars.contains(var) {
                            // We need this in case when two revlets are rewritten by overlapping patterns
                            unused_vars.remove(var);
                        }
                    }
                    if !used_vars.contains(inp_var) {
                        used_vars.push(inp_var.clone());
                    }
                    new_vars_mapping.insert(inp_var.clone(), def);
                    owned_set.add(Node::RevLet {
                        inp_var: inp_var.clone(),
                        def_vars: def_used_vars,
                        def,
                        body,
                    })
                }
            }
        }
    }

    // println!(
    //     "skip rewrite nested: {:?}",
    //     pattern.util_calc.skip_rewrite_nested
    // );

    let shift_rules = &mut vec![];
    let rewritten_exprs: Vec<ExprOwned> = shared
        .roots
        .iter()
        .map(|root| {
            // println!("ROOT");
            let mut owned_set = ExprSet::empty(Order::ChildFirst, false, false); // need struct hash off for cost_span later
            let mut new_vars_mapping: FxHashMap<Symbol, Idx> = FxHashMap::default();
            let mut used_vars: Vec<Symbol> = vec![];
            let mut unused_vars: FxHashSet<Symbol> = FxHashSet::default();
            let mut unused_count = 0;
            let mut idx = helper(
                &mut owned_set,
                &mut new_vars_mapping,
                &mut used_vars,
                &mut unused_vars,
                pattern,
                shared,
                *root,
                0,
                shift_rules,
                inv_name,
                false,
            );
            while unused_vars.len() != unused_count {
                unused_count = unused_vars.len();
                owned_set = ExprSet::empty(Order::ChildFirst, false, false);
                used_vars = vec![];
                new_vars_mapping = FxHashMap::default();
                idx = helper(
                    &mut owned_set,
                    &mut new_vars_mapping,
                    &mut used_vars,
                    &mut unused_vars,
                    pattern,
                    shared,
                    *root,
                    0,
                    shift_rules,
                    inv_name,
                    false,
                );
            }
            ExprOwned {
                set: owned_set,
                idx,
            }
        })
        .collect();

    if !shared.cfg.no_mismatch_check && !shared.cfg.utility_by_rewrite {
        // for (i, root_idxs) in shared.root_idxs_of_task.iter().enumerate() {
        //     for idx in root_idxs {
        //         let init_cost = shared.init_cost_by_root_idx_weighted[*idx] as i32;
        //         let new_cost = (rewritten_exprs[*idx].cost(cost_fn) as f32
        //             * shared.weight_by_root_idx[*idx])
        //             .round() as i32;
        //         let diff = init_cost - new_cost;

        //         if diff > 0 {
        //             println!(
        //                 "task {} root {} was rewritten with cost diff {}",
        //                 i, shared.roots[*idx], diff
        //             );
        //         }
        //     }
        // }
        assert_eq!(
            shared
                .root_idxs_of_task
                .iter()
                .map(|root_idxs| root_idxs
                    .iter()
                    .map(|idx| (rewritten_exprs[*idx].cost(cost_fn) as f32
                        * shared.weight_by_root_idx[*idx])
                        .round() as i32)
                    .min()
                    .unwrap())
                .sum::<i32>(),
            shared.init_cost_weighted - pattern.util_calc.util,
            "\n{}\n",
            pattern.info(shared)
        );
    }

    rewritten_exprs
}

/// Rewrite with the given abstractions by performing a ultra heavily pruned version of the compression search
/// using follow/track.
pub fn rewrite_with_inventions(
    programs: &[String],
    invs: &[Invention],
    cfg: &MultistepCompressionConfig,
) -> (Vec<String>, Vec<CompressionStepResult>, serde_json::Value) {
    // if invs.is_empty() {
    //     return programs.to_vec()
    // }

    let mut cfg = cfg.clone();

    // programs.to_vec()
    let follow = Some(invs.to_vec());
    cfg.iterations = invs.len();
    cfg.step.max_arity = invs.iter().map(|inv| inv.arity).max().unwrap();
    cfg.silent = true;
    cfg.no_opt = true;
    cfg.step.allow_single_task = true;

    if cfg.verbose_rewrite {
        cfg.silent = false;
        cfg.step.quiet = false;
    }

    // ugh somewhat gross to just set this to true
    // cfg.step.rewritten_dreamcoder = true;
    // cfg.step.rewritten_intermediates = true;

    let (step_results, json_res) = multistep_compression(programs, None, None, None, follow, &cfg);

    // return the last one - note that if an abstraction wasn't used anywhere it will not be included in the step_results so this
    // may be shorter than invs.len(), however we do ensure that we continue searching for the rest of the abstractions if this happens
    // anyways.
    let rewritten = step_results
        .last()
        .map(|res| res.rewritten.iter().map(|s| s.to_string()).collect())
        .unwrap_or_else(|| programs.to_vec());
    (rewritten, step_results, json_res)
}
