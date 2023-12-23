use std::cell::RefCell;

use crate::{fixed_point::FixedPointStack, Set};

mod proven_set;
pub use proven_set::{FailedJudgment, FailedRule, ProvenSet, RuleFailureCause, TryIntoIter};

mod test_filtered;
mod test_reachable;

pub type JudgmentStack<J, O> = RefCell<FixedPointStack<J, Set<O>>>;

/// `judgment_fn!` allows construction of inference rules using a more logic-like notation.
///
/// The macro input looks like so:
///
/// ```ignore
/// (
///     ( /* condition 1 */)  // each condition is a parenthesized group
///     ( /* condition 2 */)
///     ( /* condition 3 */)! // `!` is optional and indicates match commit point, see below
///     -------------------- // 3 or more `-` separate the condition from the conclusion
///     ( /* conclusion */)  // as is the conclusion
/// )
/// ```
///
/// The conditions can be the following
///
/// * `(<expr> => <binding>)` -- used to apply judgments, but really `<expr>` can be anything with an `into_iter` method.
/// * `(if <expr>)`
/// * `(if let <pat> = <expr>)`
/// * `(let <binding> = <expr>)`
///
/// The conclusions can be the following
///
/// * `(<pat> => <binding>)
///
/// ## Failure reporting and match commit points
///
/// When we fail to prove a judgment, we'll produce a failure that includes
/// all rules that partially matched. By default this lits includes every
/// rule that matches the patterns in its conclusion.
/// Sometimes this is annoyingly verbose.
/// You can place a `!` after a condition to mark it as a "match commit point".
/// Rules that fail before reaching the match commit point will not be included
/// in the failure result.
#[macro_export]
macro_rules! judgment_fn {
    (
        $(#[$attr:meta])*
        $v:vis fn $name:ident($($input_name:ident : $input_ty:ty),* $(,)?) => $output:ty {
            debug($($debug_input_name:ident),*)
            $(assert($assert_expr:expr))*
            $(trivial($trivial_expr:expr => $trivial_result:expr))*
            $(($($rule:tt)*))*
        }
    ) => {
        $(#[$attr])*
        $v fn $name($($input_name : impl $crate::Upcast<$input_ty>),*) -> $crate::ProvenSet<$output> {
            #[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone)]
            struct __JudgmentStruct($($input_ty),*);

            $crate::cast_impl!(__JudgmentStruct);

            impl std::fmt::Debug for __JudgmentStruct {
                fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut f = fmt.debug_struct(stringify!($name));
                    let __JudgmentStruct($($input_name),*) = self;
                    $(
                        f.field(stringify!($debug_input_name), $debug_input_name);
                    )*
                    f.finish()
                }
            }

            $(let $input_name: $input_ty = $crate::Upcast::upcast($input_name);)*

            $(
                // Assertions are preconditions
                assert!($assert_expr);
            )*

            $(
                // Trivial cases are an (important) optimization that lets
                // you cut out all the normal rules.
                if $trivial_expr {
                    return $crate::ProvenSet::proven(std::iter::once($trivial_result).collect());
                }
            )*

            let mut failed_rules = $crate::set![];
            let input = __JudgmentStruct($($input_name),*);
            let output = $crate::fixed_point::fixed_point::<
                __JudgmentStruct,
                $crate::Set<$output>,
            >(
                // Tracing span:
                |input| {
                    let __JudgmentStruct($($input_name),*) = input;
                    tracing::debug_span!(
                        stringify!($name),
                        $(?$debug_input_name),*
                    )
                },

                // Stack:
                {
                    thread_local! {
                        static R: $crate::judgment::JudgmentStack<__JudgmentStruct, $output> = Default::default()
                    }
                    &R
                },

                // Input:
                input.clone(),

                // Default value:
                |_| Default::default(),

                // Next value:
                |input: __JudgmentStruct| {
                    let mut output = $crate::Set::new();

                    failed_rules.clear();

                    $crate::push_rules!(
                        $name,
                        &input,
                        output,
                        failed_rules,
                        ($($input_name),*) => $output,
                        $(($($rule)*))*
                    );

                    output
                },
            );

            if !output.is_empty() {
                $crate::ProvenSet::proven(output)
            } else {
                $crate::ProvenSet::failed_rules(&input, failed_rules)
            }
        }
    }
}

#[macro_export]
macro_rules! push_rules {
    ($judgment_name:ident, $input_value:expr, $output:expr, $failed_rules:expr, $input_names:tt => $output_ty:ty, $($rule:tt)*) => {
        $($crate::push_rules!(@rule ($judgment_name, $input_value, $output, $failed_rules, $input_names => $output_ty) $rule);)*
    };

    // `@rule (builder) rule` phase: invoked for each rule, emits `push_rule` call

    (@rule ($judgment_name:ident, $input_value:expr, $output:expr, $failed_rules:expr, $input_names:tt => $output_ty:ty) ($($m:tt)*)) => {
        // Start accumulating.
        $crate::push_rules!(@accum
            args($judgment_name, $input_value, $output, $failed_rules, $input_names => $output_ty)
            accum((1-1); 0;)
            input($($m)*)
        );
    };

    // `@accum ($match_index; $current_index; conditions)` phase: accumulates the contents of a given rule,
    // pushing tokens into `conditions` until the `-----` and conclusion are found.
    //
    // The `$match_index` stores the location where `!` was found. It is expected to start
    // at 0. The `current_index` is also expected to start as the expression `0`.

    (@accum
        args($judgment_name:ident, $input_value:expr, $output:expr, $failed_rules:expr, ($($input_names:ident),*) => $output_ty:ty)
        accum($match_index:expr; $current_index:expr; $($m:tt)*)
        input(
            ---$(-)* ($n:literal)
            ($conclusion_name:ident($($patterns:tt)*) => $v:expr)
        )
    ) => {
        // Found the conclusion.
        {
            // give the user a type error if the name they gave
            // in the conclusion is not the same as the name of the
            // function
            #[allow(dead_code)]
            struct WrongJudgmentNameInConclusion;
            const _: WrongJudgmentNameInConclusion = {
                let $judgment_name = WrongJudgmentNameInConclusion;
                $conclusion_name
            };

            if let Some(__JudgmentStruct($($input_names),*)) = Some($input_value) {
                $crate::push_rules!(@match
                    inputs($($input_names)*)
                    patterns($($patterns)*,)
                    args(@body
                        ($judgment_name; $n; $v; $output);
                        ($failed_rules, $match_index, ($($input_names),*), $n);
                        $($m)*
                    )
                );
            }
        }
    };

    (@accum
        args $args:tt
        accum($match_index:expr; $current_index:expr; $($m:tt)*)
        input(! $($o:tt)*)
    ) => {
        // If we see a `!` in the list, that represents a "match commit point".
        // Overwrite `$match_index` with `$current_index` and proceed.
        $crate::push_rules!(@accum
            args $args
            accum($current_index; $current_index; $($m)*)
            input($($o)*)
        )
    };

    (@accum
        args $args:tt
        accum($match_index:expr; $current_index:expr; $($m:tt)*)
        input(($($n:tt)*) $($o:tt)*)
    ) => {
        // Move one parenthesized condition `($n*)` onto the list after `$m`.
        $crate::push_rules!(@accum
            args $args
            accum($match_index; $current_index+1; $($m)* ($($n)*))
            input($($o)*)
        )
    };

    // Matching phase: peel off the patterns one by one and match them against the values
    // extracted from the input. For anything that is not an identity pattern, invoke `downcast`.

    (@match inputs() patterns() args(@body ($judgment_name:ident; $n:literal; $v:expr; $output:expr); $inputs:tt; $($m:tt)*)) => {
        tracing::trace_span!("matched rule", rule = $n, judgment = stringify!($judgment_name)).in_scope(|| {
            let mut step_index = 0;
            $crate::push_rules!(@body ($judgment_name, $n, $v, $output); $inputs; step_index; $($m)*);
        });
    };

    (@match inputs() patterns(,) args $args:tt) => {
        $crate::push_rules!(@match inputs() patterns() args $args);
    };

    (@match inputs() patterns $patterns:tt args $args:tt) => {
        compile_error!("more patterns in rule than arguments on fn")
    };

    (@match inputs $inputs:tt patterns() args $args:tt) => {
        compile_error!("fewer patterns in rule than arguments on fn")
    };

    (@match inputs($in0:ident $($inputs:tt)*) patterns($pat0:ident : $ty0:ty, $($pats:tt)*) args $args:tt) => {
        {
            if let Some($pat0) = $crate::Downcast::downcast::<$ty0>($in0) {
                $crate::push_rules!(@match inputs($($inputs)*) patterns($($pats)*) args $args);
            }
        }
    };

    (@match inputs($in0:ident $($inputs:tt)*) patterns($pat0:ident, $($pats:tt)*) args $args:tt) => {
        {
            let $pat0 = Clone::clone($in0);
            $crate::push_rules!(@match inputs($($inputs)*) patterns($($pats)*) args $args);
        }
    };

    (@match inputs($in0:ident $($inputs:tt)*) patterns($pat0:pat, $($pats:tt)*) args $args:tt) => {
        if let Some($pat0) = $crate::Downcast::downcast(&$in0) {
            $crate::push_rules!(@match inputs($($inputs)*) patterns($($pats)*) args $args);
        }
    };

    // (@match (($arg0:ident @ $pat0:pat) $($args:tt)*) ($n:literal; $v:expr; $output:ident) $($m:tt)*) => {
    //     if let Some($pat0) = $crate::cast::Downcast::downcast(&$arg0) {
    //         $crate::push_rules!(@match ($($args)*) ($n; $v; $output) $($m)*);
    //     }
    // };

    // `@body (v)` phase: processes the conditions, generating the code
    // to evaluate the rule. This is effectively an iterator chain. The
    // expression `v` is carried in from the conclusion and forms the final
    // output of this rule, once all the conditions are evaluated.

    (@body $args:tt; $inputs:tt; $step_index:ident; (if $c:expr) $($m:tt)*) => {
        if $c {
            $step_index += 1;
            $crate::push_rules!(@body $args; $inputs; $step_index; $($m)*);
        } else {
            $crate::push_rules!(@record_failure $inputs; $step_index; $crate::judgment::RuleFailureCause::IfFalse {
                expr: stringify!($c).to_string(),
            });
        }
    };

    (@body $args:tt; $inputs:tt; $step_index:ident; (assert $c:expr) $($m:tt)*) => {
        assert!($c);
        $step_index += 1;
        $crate::push_rules!(@body $args; $inputs; $step_index; $($m)*);
    };

    (@body $args:tt; $inputs:tt; $step_index:ident; (if let $p:pat = $e:expr) $($m:tt)*) => {
        let value = &$e;
        if let $p = Clone::clone(value) {
            $step_index += 1;
            $crate::push_rules!(@body $args; $inputs; $step_index; $($m)*);
        } else {
            $crate::push_rules!(@record_failure $inputs; $step_index; $crate::judgment::RuleFailureCause::IfLetDidNotMatch {
                pattern: stringify!($p).to_string(),
                value: format!("{:?}", value),
            });
        }
    };

    (@body $args:tt; $inputs:tt; $step_index:ident; ($i:expr => $p:pat) $($m:tt)*) => {
        // Explicitly calling `into_iter` silences some annoying lints
        // in the case where `$i` is an `Option` or a `Result`
        match $crate::judgment::TryIntoIter::try_into_iter($i, || stringify!($i).to_string()) {
            Ok(i) => {
                $step_index += 1;
                for $p in std::iter::IntoIterator::into_iter(i) {
                    $crate::push_rules!(@body $args; $inputs; $step_index; $($m)*);
                }
            }
            Err(e) => {
                $crate::push_rules!(@record_failure $inputs; $step_index; e);
            }
        }
    };

    (@body $args:tt; $inputs:tt; $step_index:ident; (let $p:pat = $i:expr) $($m:tt)*) => {
        {
            let $p = $i;
            $step_index += 1;
            $crate::push_rules!(@body $args; $inputs; $step_index; $($m)*);
        }
    };

    (@body ($judgment_name:ident, $rule_name:literal, $v:expr, $output:expr); $inputs:tt; $step_index:ident;) => {
        {
            let _ = $step_index; // suppress warnings about value not being read
            let result = $crate::Upcast::upcast($v);
            tracing::debug!("produced {:?} from rule {:?} in judgment {:?}", result, $rule_name, stringify!($judgment_name));
            $output.insert(result)
        }
    };

    //

    (@record_failure ($failed_rules:expr, $match_index:expr, $inputs:tt, $rule_name:literal); $step_index:ident; $cause:expr) => {
        if $step_index >= $match_index {
            tracing::debug!(
                "rule {rn} failed at step {s} because {cause} ({file}:{line}:{column})",
                rn = $rule_name,
                s = $step_index,
                cause = $cause,
                file = file!(),
                line = line!(),
                column = column!(),
            );
            $failed_rules.insert(
                $crate::judgment::FailedRule {
                    rule_name_index: Some(($rule_name.to_string(), $step_index)),
                    file: file!().to_string(),
                    line: line!(),
                    column: column!(),
                    cause: $cause,
                }
            );
        } else {
            tracing::trace!(
                "rule {rn} failed at step {s} because {cause} ({file}:{line}:{column})",
                rn = $rule_name,
                s = $step_index,
                cause = $cause,
                file = file!(),
                line = line!(),
                column = column!(),
            );
        }
    }
}
