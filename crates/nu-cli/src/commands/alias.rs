use crate::commands::WholeStreamCommand;
use crate::context::CommandRegistry;
use crate::prelude::*;
use nu_data::config;
use nu_errors::ShellError;
use nu_parser::SignatureRegistry;
use nu_protocol::hir::{
    AliasBlock, Block, ClassifiedCommand, Expression, NamedValue, SpannedExpression, Variable,
};
use nu_protocol::{
    CommandAction, NamedType, PositionalType, ReturnSuccess, Signature, SyntaxShape, UntaggedValue,
    Value,
};
use nu_source::Tagged;
use std::collections::HashMap;

pub struct Alias;

#[derive(Deserialize)]
pub struct AliasArgs {
    pub name: Tagged<String>,
    pub args: Vec<Value>,
    pub block: Block,
    pub save: Option<bool>,
}

#[async_trait]
impl WholeStreamCommand for Alias {
    fn name(&self) -> &str {
        "alias"
    }

    fn signature(&self) -> Signature {
        Signature::build("alias")
            .required("name", SyntaxShape::String, "the name of the alias")
            .required("args", SyntaxShape::Table, "the arguments to the alias")
            .required(
                "block",
                SyntaxShape::Block,
                "the block to run as the body of the alias",
            )
            .switch("save", "save the alias to your config", Some('s'))
    }

    fn usage(&self) -> &str {
        "Define a shortcut for another command."
    }

    async fn run(
        &self,
        args: CommandArgs,
        registry: &CommandRegistry,
    ) -> Result<OutputStream, ShellError> {
        alias(args, registry).await
    }

    fn examples(&self) -> Vec<Example> {
        vec![
            Example {
                description: "An alias without parameters",
                example: "alias say-hi [] { echo 'Hello!' }",
                result: None,
            },
            Example {
                description: "An alias with a single parameter",
                example: "alias l [x] { ls $x }",
                result: None,
            },
        ]
    }
}

pub async fn alias(
    args: CommandArgs,
    registry: &CommandRegistry,
) -> Result<OutputStream, ShellError> {
    let registry = registry.clone();
    let mut raw_input = args.raw_input.clone();
    let (
        AliasArgs {
            name,
            args: list,
            block,
            save,
        },
        _ctx,
    ) = args.process(&registry).await?;
    let mut processed_args: Vec<String> = vec![];

    if let Some(true) = save {
        let mut result = nu_data::config::read(name.clone().tag, &None)?;

        // process the alias to remove the --save flag
        let left_brace = raw_input.find('{').unwrap_or(0);
        let right_brace = raw_input.rfind('}').unwrap_or_else(|| raw_input.len());
        let left = raw_input[..left_brace]
            .replace("--save", "")
            .replace("-s", "");
        let right = raw_input[right_brace..]
            .replace("--save", "")
            .replace("-s", "");
        raw_input = format!("{}{}{}", left, &raw_input[left_brace..right_brace], right);

        // create a value from raw_input alias
        let alias: Value = raw_input.trim().to_string().into();
        let alias_start = raw_input.find('[').unwrap_or(0); // used to check if the same alias already exists

        // add to startup if alias doesn't exist and replace if it does
        match result.get_mut("startup") {
            Some(startup) => {
                if let UntaggedValue::Table(ref mut commands) = startup.value {
                    if let Some(command) = commands.iter_mut().find(|command| {
                        let cmd_str = command.as_string().unwrap_or_default();
                        cmd_str.starts_with(&raw_input[..alias_start])
                    }) {
                        *command = alias;
                    } else {
                        commands.push(alias);
                    }
                }
            }
            None => {
                let table = UntaggedValue::table(&[alias]);
                result.insert("startup".to_string(), table.into_value(Tag::default()));
            }
        }
        config::write(&result, &None)?;
    }

    for item in list.iter() {
        if let Ok(string) = item.as_string() {
            processed_args.push(format!("${}", string));
        } else {
            return Err(ShellError::labeled_error(
                "Expected a string",
                "expected a string",
                item.tag(),
            ));
        }
    }

    Ok(OutputStream::one(ReturnSuccess::action(
        CommandAction::AddAlias(
            name.to_string(),
            process_block(processed_args, block, &registry)?,
        ),
    )))
}

fn process_block(
    args: Vec<String>,
    block: Block,
    registry: &CommandRegistry,
) -> Result<AliasBlock, ShellError> {
    let (found_args, found_cmds) = inspect_block(&block, registry)?;
    let arg_shapes = args
        .iter()
        .map(|arg| {
            (
                arg.clone(),
                match found_args.get(arg) {
                    None | Some((_, None)) => SyntaxShape::Any,
                    Some((_, Some(shape))) => *shape,
                },
            )
        })
        .collect();

    Ok(AliasBlock {
        block,
        arg_shapes,
        cmd_scopes: found_cmds.into_iter().collect(),
    })
}

type ShapeMap = HashMap<String, (Span, Option<SyntaxShape>)>;
type ScopeMap = HashMap<String, usize>;
type BlockInfo = (ShapeMap, ScopeMap); // TODO name? restructure?

fn check_merge(existing: &mut BlockInfo, new: BlockInfo) -> Result<(), ShellError> {
    let (new_shapes, new_scopes) = new;
    existing.1.extend(new_scopes);
    for (name, v) in new_shapes.into_iter() {
        match v.1 {
            None => match existing.0.get(&name) {
                None => {
                    existing.0.insert(name, v);
                    Ok(())
                }
                Some(_) => Ok(()),
            },
            Some(new) => match existing.0.insert(name, v) {
                None => Ok(()),
                Some((_, shp)) => match shp {
                    None => Ok(()),
                    Some(shape) => match shape {
                        SyntaxShape::Any => Ok(()),
                        shape if shape == new => Ok(()),
                        _ => Err(ShellError::labeled_error(
                            "Type conflict in alias variable use",
                            "creates type conflict",
                            v.0,
                        )),
                    },
                },
            },
        }?
    }

    Ok(())
}

fn inspect_expr(
    spanned_expr: &SpannedExpression,
    registry: &CommandRegistry,
) -> Result<(ShapeMap, ScopeMap), ShellError> {
    match &spanned_expr.expr {
        // TODO range will need similar if/when invocations can be parsed within range expression
        Expression::Binary(bin) => inspect_expr(&bin.left, registry).and_then(|mut left| {
            inspect_expr(&bin.right, registry)
                .and_then(|right| check_merge(&mut left, right))
                .map(|()| left)
        }),
        Expression::Block(b) => inspect_block(&b, registry),
        Expression::Path(path) => match &path.head.expr {
            Expression::Invocation(b) => inspect_block(&b, registry),
            Expression::Variable(Variable::Other(var, _)) => {
                let mut result = HashMap::new();
                result.insert(var.to_string(), (spanned_expr.span, None));
                Ok((result, ScopeMap::new()))
            }
            _ => Ok((ShapeMap::new(), ScopeMap::new())),
        },
        _ => Ok((ShapeMap::new(), ScopeMap::new())),
    }
}

fn inspect_block(block: &Block, registry: &CommandRegistry) -> Result<BlockInfo, ShellError> {
    let apply_shape = |found: ShapeMap, sig_shape: SyntaxShape| -> ShapeMap {
        found
            .iter()
            .map(|(v, sh)| match sh.1 {
                None => (v.clone(), (sh.0, Some(sig_shape))),
                Some(shape) => (v.clone(), (sh.0, Some(shape))),
            })
            .collect()
    };

    let mut block_info = (ShapeMap::new(), ScopeMap::new());

    for pipeline in &block.block {
        for classified in &pipeline.list {
            match classified {
                ClassifiedCommand::Expr(spanned_expr) => {
                    let found = inspect_expr(&spanned_expr, registry)?;
                    check_merge(&mut block_info, found)?
                }
                ClassifiedCommand::Internal(internal) => {
                    let name = &internal.name;
                    if let Some(signature) = registry.get(name) {
                        if !block_info.1.contains_key(name) {
                            block_info.1.insert(
                                name.to_string(),
                                registry
                                    .get_scope(name)
                                    .expect("name should be in regsitry"),
                            );
                        }

                        if let Some(positional) = &internal.args.positional {
                            for (i, spanned_expr) in positional.iter().enumerate() {
                                let mut found = inspect_expr(&spanned_expr, registry)?;
                                if i >= signature.positional.len() {
                                    if let Some((sig_shape, _)) = &signature.rest_positional {
                                        found.0 = apply_shape(found.0, *sig_shape);
                                        check_merge(&mut block_info, found)?;
                                    } else {
                                        unreachable!("should have error'd in parsing");
                                    }
                                } else {
                                    let (pos_type, _) = &signature.positional[i];
                                    match pos_type {
                                        // TODO pass on mandatory/optional?
                                        PositionalType::Mandatory(_, sig_shape)
                                        | PositionalType::Optional(_, sig_shape) => {
                                            found.0 = apply_shape(found.0, *sig_shape);
                                            check_merge(&mut block_info, found)?;
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(named) = &internal.args.named {
                            for (name, val) in named.iter() {
                                if let NamedValue::Value(_, spanned_expr) = val {
                                    let mut found = inspect_expr(&spanned_expr, registry)?;
                                    match signature.named.get(name) {
                                        None => {
                                            unreachable!("should have error'd in parsing");
                                        }
                                        Some((named_type, _)) => {
                                            if let NamedType::Mandatory(_, sig_shape)
                                            | NamedType::Optional(_, sig_shape) = named_type
                                            {
                                                found.0 = apply_shape(found.0, *sig_shape);
                                                check_merge(&mut block_info, found)?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        unreachable!("registry has lost name it provided");
                    }
                }
                ClassifiedCommand::Dynamic(_) | ClassifiedCommand::Error(_) => (),
            }
        }
    }

    Ok(block_info)
}

#[cfg(test)]
mod tests {
    use super::Alias;

    #[test]
    fn examples_work_as_expected() {
        use crate::examples::test as test_examples;

        test_examples(Alias {})
    }
}
