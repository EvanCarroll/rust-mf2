//! Hoisting (`plans/05-tooling.md` §6.1, "Selection"): Fluent puts a select
//! expression anywhere in a pattern, MF2 has one `.match` per message.
//!
//! Every select becomes one selector column; the message's variants are the
//! product of the columns' keys, each variant's pattern the Fluent pattern
//! with every select replaced by the chosen arm. A select nested in an arm
//! contributes keys only under that arm (`*` under the others), and two
//! selects on the same selector with the same keys are one column.

use std::borrow::Cow;

use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, FunctionRef, InputDeclaration, Key, Literal,
    LocalDeclaration, Message, Pattern, PatternMessage, PatternPart, SelectMessage,
    VariableExpression, VariableRef, Variant,
};

/// The most variants a converted message may have; past it, converting by
/// hand is the better answer (`fluent-variant-limit`).
pub(super) const VARIANT_LIMIT: usize = 256;

/// A pattern with its references inlined and its constant selects folded.
pub(super) type Flat = Vec<Part>;

/// One piece of a [`Flat`] pattern.
#[derive(Clone, Debug)]
pub(super) enum Part {
    Text(String),
    /// A placeholder, and the text `fluent-bundle` writes for it when that
    /// is known at conversion (a number literal).
    Expr(Expression<'static>, Option<String>),
    Select(Box<Select>),
}

/// A select whose selector is known only at run time.
#[derive(Clone, Debug)]
pub(super) struct Select {
    /// The variable, and the function it selects with.
    pub(super) var: String,
    pub(super) function: FunctionRef<'static>,
    /// The arms in MF2 order: literal keys in Fluent's order, `*` last.
    pub(super) arms: Vec<Arm>,
}

#[derive(Clone, Debug)]
pub(super) struct Arm {
    /// `None` is `*`.
    pub(super) key: Option<String>,
    pub(super) pattern: Flat,
}

/// A column: a selector and its keys.
struct Column {
    var: String,
    function: FunctionRef<'static>,
    keys: Vec<Option<String>>,
}

impl Column {
    fn is(&self, s: &Select) -> bool {
        self.var == s.var
            && self.function == s.function
            && self.keys.len() == s.arms.len()
            && self.keys.iter().zip(&s.arms).all(|(k, a)| *k == a.key)
    }
}

/// One variant being built: the arm chosen in each column met so far, and
/// the pattern.
#[derive(Clone, Default)]
struct Row {
    chosen: Vec<Option<usize>>,
    pattern: Pattern<'static>,
}

/// The variant limit was passed.
pub(super) struct TooMany;

/// The MF2 message of a flat pattern.
pub(super) fn message(flat: &Flat) -> Result<Message<'static>, TooMany> {
    let mut columns: Vec<Column> = Vec::new();
    collect(flat, &mut columns);
    if columns.is_empty() {
        let mut pattern = Pattern::new();
        for part in flat {
            push(&mut pattern, part);
        }
        return Ok(Message::Pattern(PatternMessage {
            declarations: Vec::new(),
            pattern,
        }));
    }

    let rows = extend(
        vec![Row {
            chosen: vec![None; columns.len()],
            pattern: Pattern::new(),
        }],
        flat,
        &columns,
    )?;

    let (declarations, selectors) = declare(&columns, flat);
    let variants = rows
        .into_iter()
        .map(|row| Variant {
            keys: row
                .chosen
                .iter()
                .zip(&columns)
                .map(|(chosen, column)| {
                    match chosen.and_then(|i| column.keys.get(i).cloned().flatten()) {
                        Some(k) => Key::Literal(Literal {
                            value: Cow::Owned(k),
                        }),
                        None => Key::CatchAll(CatchAllKey::default()),
                    }
                })
                .collect(),
            value: row.pattern,
        })
        .collect();
    Ok(Message::Select(SelectMessage {
        declarations,
        selectors,
        variants,
    }))
}

/// Every column, in the order first met (so an outer select's column comes
/// before those of the selects nested in its arms, which MF2's column-wise
/// matching relies on).
fn collect(flat: &Flat, columns: &mut Vec<Column>) {
    for part in flat {
        if let Part::Select(s) = part {
            if !columns.iter().any(|c| c.is(s)) {
                columns.push(Column {
                    var: s.var.clone(),
                    function: s.function.clone(),
                    keys: s.arms.iter().map(|a| a.key.clone()).collect(),
                });
            }
            for arm in &s.arms {
                collect(&arm.pattern, columns);
            }
        }
    }
}

/// Appends `flat` to every row, branching at each select whose column the
/// row has not chosen yet.
fn extend(rows: Vec<Row>, flat: &Flat, columns: &[Column]) -> Result<Vec<Row>, TooMany> {
    let mut rows = rows;
    for part in flat {
        match part {
            Part::Select(s) => {
                let col = columns.iter().position(|c| c.is(s)).unwrap_or_default();
                let mut next = Vec::new();
                for row in rows {
                    match row.chosen.get(col).copied().flatten() {
                        Some(i) => {
                            let arm = &s.arms[i];
                            next.extend(extend(vec![row], &arm.pattern, columns)?);
                        }
                        None => {
                            for (i, arm) in s.arms.iter().enumerate() {
                                let mut branch = row.clone();
                                branch.chosen[col] = Some(i);
                                next.extend(extend(vec![branch], &arm.pattern, columns)?);
                                if next.len() > VARIANT_LIMIT {
                                    return Err(TooMany);
                                }
                            }
                        }
                    }
                }
                rows = next;
            }
            other => {
                for row in &mut rows {
                    push(&mut row.pattern, other);
                }
            }
        }
    }
    Ok(rows)
}

fn push(pattern: &mut Pattern<'static>, part: &Part) {
    match part {
        Part::Text(t) => pattern.push(PatternPart::Text(Cow::Owned(t.clone()))),
        Part::Expr(e, _) => pattern.push(PatternPart::Expression(e.clone())),
        Part::Select(_) => {}
    }
}

/// The declarations the columns need and the selector each column names:
/// `.input` for a variable's first annotation, `.local $v-2 = …` for another
/// annotation of the same variable, the suffix the smallest integer that
/// collides with no name in the message.
fn declare(
    columns: &[Column],
    flat: &Flat,
) -> (Vec<Declaration<'static>>, Vec<VariableRef<'static>>) {
    let mut names = std::collections::BTreeSet::new();
    names_in(flat, &mut names);
    let mut inputs: Vec<(String, FunctionRef<'static>)> = Vec::new();
    let mut locals: Vec<(String, String, FunctionRef<'static>)> = Vec::new();
    let mut selectors = Vec::new();
    for column in columns {
        let name = if let Some((_, f)) = inputs.iter().find(|(v, _)| *v == column.var) {
            if *f == column.function {
                column.var.clone()
            } else if let Some((n, _, _)) = locals
                .iter()
                .find(|(_, v, f)| *v == column.var && *f == column.function)
            {
                n.clone()
            } else {
                let mut k = 2;
                let local = loop {
                    let candidate = format!("{}-{k}", column.var);
                    if !names.contains(&candidate) {
                        break candidate;
                    }
                    k += 1;
                };
                names.insert(local.clone());
                locals.push((local.clone(), column.var.clone(), column.function.clone()));
                local
            }
        } else {
            inputs.push((column.var.clone(), column.function.clone()));
            column.var.clone()
        };
        selectors.push(VariableRef {
            name: Cow::Owned(name),
        });
    }
    let mut declarations: Vec<Declaration<'static>> = inputs
        .into_iter()
        .map(|(var, function)| {
            Declaration::Input(InputDeclaration {
                name: Cow::Owned(var.clone()),
                value: VariableExpression {
                    arg: VariableRef {
                        name: Cow::Owned(var),
                    },
                    function: Some(function),
                    attributes: Attributes::default(),
                },
            })
        })
        .collect();
    declarations.extend(locals.into_iter().map(|(name, var, function)| {
        Declaration::Local(LocalDeclaration {
            name: Cow::Owned(name),
            value: Expression::Variable(VariableExpression {
                arg: VariableRef {
                    name: Cow::Owned(var),
                },
                function: Some(function),
                attributes: Attributes::default(),
            }),
        })
    }));
    (declarations, selectors)
}

/// Every variable name the pattern uses.
fn names_in(flat: &Flat, names: &mut std::collections::BTreeSet<String>) {
    for part in flat {
        match part {
            Part::Expr(Expression::Variable(v), _) => {
                names.insert(v.arg.name.to_string());
            }
            Part::Text(_) | Part::Expr(..) => {}
            Part::Select(s) => {
                names.insert(s.var.clone());
                for arm in &s.arms {
                    names_in(&arm.pattern, names);
                }
            }
        }
    }
}
