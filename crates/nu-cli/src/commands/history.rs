use crate::cli::History as HistoryFile;
use crate::commands::WholeStreamCommand;
use crate::prelude::*;
use nu_errors::ShellError;
use nu_protocol::{ReturnSuccess, Signature, UntaggedValue, Value};
use nu_source::Tagged;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct History;

#[derive(Deserialize)]
pub struct HistoryArgs {
    #[serde(rename(deserialize = "lite-parse"))]
    lite_parsed: Tagged<bool>,
}

#[async_trait]
impl WholeStreamCommand for History {
    fn name(&self) -> &str {
        "history"
    }

    fn signature(&self) -> Signature {
        Signature::build("history").switch(
            "lite-parse",
            "List of commands and arguments as split by nu lite-parse",
            Some('l'),
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
}

async fn history(
    args: CommandArgs,
    registry: &CommandRegistry,
) -> Result<OutputStream, ShellError> {
    let tag = args.call_info.name_tag.clone(); // TODO?
    let registry = registry.clone();
    let (
        HistoryArgs {
            lite_parsed: lite_parse,
        },
        _,
    ) = args.process(&registry).await?;

    let history_path = HistoryFile::path();
    let file = File::open(history_path);
    if let Ok(file) = file {
        let reader = BufReader::new(file);
        let output = reader.lines().filter_map(move |line| match line {
            Ok(line) => Some(ReturnSuccess::value({
                if lite_parse.item {
                    let result = match nu_parser::lite_parse(&line, 0) {
                        Err(err) => {
                            return None; // TODO
                        }
                        Ok(val) => val,
                    };

                    let mut words = vec![];
                    for lp in result.block.clone() {
                        for light_cmd in lp.commands {
                            words.push(light_cmd.name.item.clone());
                            for arg in light_cmd.args {
                                words.push(arg.item.clone());
                            }
                        }
                    }

                    let table: Vec<Value> = words
                        .iter()
                        .map(|s| UntaggedValue::string(s).into())
                        .collect();
                    UntaggedValue::table(table.as_slice()).into_value(&tag)
                } else {
                    UntaggedValue::string(line).into_value(&tag)
                }
            })),
            Err(_) => None,
        });

        Ok(futures::stream::iter(output).to_output_stream())
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
