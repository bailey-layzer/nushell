use crate::cli::History as HistoryFile;
use crate::commands::WholeStreamCommand;
use crate::prelude::*;
use nu_errors::ShellError;
use nu_protocol::hir::{ClassifiedCommand, Commands};
use nu_protocol::{ReturnSuccess, Signature, UntaggedValue};
use nu_source::Tagged;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct History;

#[derive(Deserialize)]
pub struct HistoryArgs {
    structured: Tagged<bool>,
}

#[async_trait]
impl WholeStreamCommand for History {
    fn name(&self) -> &str {
        "history"
    }

    fn signature(&self) -> Signature {
        Signature::build("history").switch(
            "structured",
            "output retrieved history as row",
            Some('s'), // TODO example for this
        )
    }

    fn usage(&self) -> &str {
        "Display command history."
    }

    async fn run(
        &self,
        args: CommandArgs,
        registry: &CommandRegistry,
    ) -> Result<OutputStream, ShellError> {
        history(args, registry).await
    }

    fn examples(&self) -> Vec<Example> {
        vec![Example {
            description: "TODO",
            example: r#"history -s | last 1 | TODO"#,
            result: None, // TODO probably can't test here
        }]
    }
}

async fn history(
    args: CommandArgs,
    registry: &CommandRegistry,
) -> Result<OutputStream, ShellError> {
    let tag = args.call_info.name_tag.clone();
    let (HistoryArgs { structured }, _) = args.process(&registry).await?;
    let history_path = HistoryFile::path();
    let file = File::open(history_path);
    if let Ok(file) = file {
        let reader = BufReader::new(file);

        if structured.item {
            let registry = registry.clone();
            let lines = reader.lines().filter_map(move |line| match line {
                Ok(line) => {
                    let lite = match nu_parser::lite_parse(&line, 0) {
                        Ok(val) => val,
                        Err(_) => {
                            return Some(Err(ShellError::unexpected("TODO or is it?")));
                            // return Some(Ok(format!("ERR: {}", line)));
                        }
                    };

                    let mut classified_block = nu_parser::classify_block(&lite, &registry);
                    if let Some(failure) = classified_block.failed {
                        return None; // TODO?
                    }

                    let block = classified_block.block.clone(); // TODO don't clone
                    if let Some(last) = classified_block.block.block.pop() {
                        let rows = last.list.iter().filter_map(|row| match row {
                            ClassifiedCommand::Expr(expr) => None,
                            ClassifiedCommand::Internal(internal) => Some("foo"),

                            // FIXME implement when these are implemented elsewhere
                            ClassifiedCommand::Dynamic(_) => None,
                            ClassifiedCommand::Error(_) => None,
                        });
                    }

                    Some(Ok(block))

                    // Some(Err(ShellError::unimplemented("foo")))
                }
                Err(_) => None,
            });

            // Err(ShellError::unimplemented("structured history"))
            Ok(futures::stream::iter(lines)
                .map(move |block| match block {
                    Err(err) => Err(err),
                    Ok(bl) => {
                        ReturnSuccess::value(UntaggedValue::Block(bl).into_value(tag.clone()))
                    }
                })
                .to_output_stream())
        } else {
            let output = reader.lines().filter_map(move |line| match line {
                Ok(line) => Some(ReturnSuccess::value(
                    UntaggedValue::string(line).into_value(tag.clone()),
                )),
                Err(_) => None,
            });
            Ok(futures::stream::iter(output).to_output_stream())
        }
    } else {
        Err(ShellError::labeled_error(
            "Could not open history",
            "history file could not be opened",
            tag,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    #[test]
    fn examples_work_as_expected() {
        use crate::examples::test as test_examples;

        test_examples(History {})
    }
}
