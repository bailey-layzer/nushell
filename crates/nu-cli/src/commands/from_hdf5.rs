use crate::commands::from_delimited_data::from_delimited_data;
use crate::commands::WholeStreamCommand;
use crate::prelude::*;
use nu_errors::ShellError;
use nu_protocol::{Primitive, Signature, SyntaxShape, UntaggedValue, Value};

pub struct FromHDF5;

#[derive(Deserialize)]
pub struct FromHDF5Args {}

#[async_trait]
impl WholeStreamCommand for FromHDF5 {
    fn name(&self) -> &str {
        "from hdf5"
    }

    fn signature(&self) -> Signature {
        Signature::build("from hdf5")
        // .named(
        //     "separator",
        //     SyntaxShape::String,
        //     "a character to separate columns, defaults to ','",
        //     Some('s'),
        // )
        // .switch(
        //     "headerless",
        //     "don't treat the first row as column names",
        //     None,
        // )
    }

    fn usage(&self) -> &str {
        "Parse hdf5 file TODO."
    }

    async fn run(&self, args: CommandArgs) -> Result<OutputStream, ShellError> {
        from_hdf5(args).await
    }

    fn examples(&self) -> Vec<Example> {
        vec![
            Example {
                description: "Convert comma-separated data to a table",
                example: "open data.txt | from csv",
                result: None,
            },
            Example {
                description: "Convert comma-separated data to a table, ignoring headers",
                example: "open data.txt | from csv --headerless",
                result: None,
            },
            Example {
                description: "Convert semicolon-separated data to a table",
                example: "open data.txt | from csv --separator ';'",
                result: None,
            },
        ]
    }
}

async fn from_hdf5(args: CommandArgs) -> Result<OutputStream, ShellError> {
    let name = args.call_info.name_tag.clone();

    // let (FromHDF5Args {}, input) = args.process().await?;

    let input = args.input;

    // match hdf5::File::open(input) {
    //     Ok(file) => {
    //         println!("opened");
    //     },
    //     Err(e) => {
    //         ShellError::
    //     }
    // }

    Ok(OutputStream::empty())

    // from_delimited_data(headerless, sep, "CSV", input, name).await
}

#[cfg(test)]
mod tests {
    use super::FromCSV;
    use super::ShellError;

    #[test]
    fn examples_work_as_expected() -> Result<(), ShellError> {
        use crate::examples::test as test_examples;

        Ok(test_examples(FromCSV {})?)
    }
}
