use crate::commands::constants::BAT_LANGUAGES;
use crate::prelude::*;
use encoding_rs::{Encoding, UTF_8};
use futures_util::StreamExt;
use hdf5::types::{Array, FixedAscii, TypeDescriptor, VarLenArray, VarLenAscii};
use log::debug;
use nu_engine::StringOrBinary;
use nu_engine::WholeStreamCommand;
use nu_errors::ShellError;
use nu_protocol::{
    CommandAction, Primitive, ReturnSuccess, Signature, SyntaxShape, TaggedDictBuilder,
    UntaggedValue, Value,
};
use nu_source::{AnchorLocation, Span, Tagged};
use regex::Regex;
use std::path::PathBuf;

pub struct Open;

#[derive(Deserialize)]
pub struct OpenArgs {
    path: Tagged<PathBuf>,
    raw: Tagged<bool>,
    encoding: Option<Tagged<String>>,
}

#[async_trait]
impl WholeStreamCommand for Open {
    fn name(&self) -> &str {
        "open"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required(
                "path",
                SyntaxShape::FilePath,
                "the file path to load values from",
            )
            .switch(
                "raw",
                "load content as a string instead of a table",
                Some('r'),
            )
            .named(
                "encoding",
                SyntaxShape::String,
                "encoding to use to open file",
                Some('e'),
            )
    }

    fn usage(&self) -> &str {
        r#"Load a file into a cell, convert to table if possible (avoid by appending '--raw').
        
Multiple encodings are supported for reading text files by using
the '--encoding <encoding>' parameter. Here is an example of a few:
big5, euc-jp, euc-kr, gbk, iso-8859-1, utf-16, cp1252, latin5

For a more complete list of encodings please refer to the encoding_rs
documentation link at https://docs.rs/encoding_rs/0.8.23/encoding_rs/#statics"#
    }

    async fn run(&self, args: CommandArgs) -> Result<OutputStream, ShellError> {
        open(args).await
    }

    fn examples(&self) -> Vec<Example> {
        vec![
            Example {
                description: "Opens \"users.csv\" and creates a table from the data",
                example: "open users.csv",
                result: None,
            },
            Example {
                description: "Opens file with iso-8859-1 encoding",
                example: "open file.csv --encoding iso-8859-1 | from csv",
                result: None,
            },
        ]
    }
}

pub fn get_encoding(opt: Option<Tagged<String>>) -> Result<&'static Encoding, ShellError> {
    match opt {
        None => Ok(UTF_8),
        Some(label) => match Encoding::for_label((&label.item).as_bytes()) {
            None => Err(ShellError::labeled_error(
                format!(
                    r#"{} is not a valid encoding, refer to https://docs.rs/encoding_rs/0.8.23/encoding_rs/#statics for a valid list of encodings"#,
                    label.item
                ),
                "invalid encoding",
                label.span(),
            )),
            Some(encoding) => Ok(encoding),
        },
    }
}

fn read_hdf5(path: Tagged<PathBuf>) -> Result<OutputStream, ShellError> {
    return match hdf5::File::open(path.as_path()) {
        Ok(file) => {
            // TODO anything with plist? how to get encoding (not here)?
            // println!("{:#?}", file.access_plist().unwrap().properties());

            // TODO what happens to error?
            // dereferencing a File makes a Group
            Ok(OutputStream::one(ReturnSuccess::value(read_group(&*file)?)))

            // for name in file
            //     .clone() // TODO
            //     .member_names()
            //     .map_err(|_| ShellError::unimplemented(""))?
            // {
            //     // for name in vec!["axis0"] {
            //     println!("==== {:?} =====", name);
            //     println!("{:#?}", file.dataset(&name));
            //     let group = file
            //         .group(&name)
            //         .map_err(|_| ShellError::unimplemented(""))?;
            //     // TODO top level can be a dataset?
            //     read_group(group);
            // }

            // return Ok(OutputStream::empty());
        }
        Err(e) => Err(ShellError::labeled_error(
            "Cannot open file as HDF5",
            "TODO",
            path.tag.clone(),
        )),
    };
    // println!("{:?}", file.member_names());
}

fn read_group(group: &hdf5::Group) -> Result<Value, ShellError> {
    let members = group.member_names().map_err(|e| {
        ShellError::untagged_runtime_error(format!("problem reading HDF file: {:?}", e))
    })?;

    println!("{:#?}", members);
    // TODO handle only one dataset differently?

    // TODO with_capacity?
    let mut dict = TaggedDictBuilder::new(Tag::unknown()); // TODO is it known?

    let re_axis = Regex::new(r"axis(\d)(?:_(label|level)(\d))?").expect("literal regex");

    let mut datasets: Vec<String> = Vec::new();
    for name in members {
        match group.group(&name) {
            Ok(group) => dict.insert_value(name, read_group(&group)?), // TODO ? appropriate?
            Err(_) => datasets.push(name),
        }
    }

    // TODO size?
    let mut axes: IndexMap<usize, Vec<Value>> = IndexMap::new();
    let mut blocks: Vec<String> = Vec::new();

    for name in datasets {
        if let Some(captures) = re_axis.captures(&name) {
            match captures.get(2).map(|m| m.as_str()) {
                None => {
                    if let Some(capture) = captures.get(1) {
                        let n = str::parse::<usize>(capture.as_str())
                            .map_err(|_| ShellError::unimplemented("TODO parsing string axis"))?;
                        if let Ok(Value {
                            value: UntaggedValue::Table(values),
                            ..
                        }) = read_dataset(
                            // TODO silly to bundle it up only to pull it back out?
                            // TODO deal with errors
                            group.dataset(&name).expect("TODO not a group or dataset?"),
                        ) {
                            // TODO don't expect
                            axes.insert(n, values);
                        }
                    }
                    // axes.insert()
                    // match .map(|m| ) {
                    //
                    //
                    //         Some("0") => {
                    //     // read_fixed_ascii(
                    //     //     group.dataset(&name).expect("TODO should be dataset?"),
                    //     //     16, // TODO need to actually extract
                    //     // )
                    //     if let Ok(val) = read_dataset(
                    //     // TODO deal with errors
                    //     group.dataset(&name).expect("TODO not a group or dataset?"),
                    //     ) {
                    //     dict.insert_value(name, val);
                    //     }
                    //     }
                    //     Some("1") => {
                    //         println!("ignoring axis1");
                    //         continue;
                    //     }
                    //     _ => panic!("TODO"),
                    // }
                }
                Some("label") => {
                    println!("ignoring label");
                    continue;
                } // TODO are these needed?
                Some("level") => {
                    println!("ignoring level");
                    continue;
                } // TODO read_fixed_ascii(dataset),
                _ => {
                    panic!("TODO")
                } // return Err(ShellError::unimplemented(format ! ("axis label {}", name)));
            }

            continue;
        }

        blocks.push(name);
    }

    let re_block = Regex::new(r"block(\d)_(items|values)").expect("literal regex");

    for name in blocks {
        match re_block.captures(&name) {
            None => (), // TODO
            Some(captures) => match captures.get(2).map(|m| m.as_str()) {
                None => (),
                Some("items") => {
                    if let Ok(Value {
                        // TODO more assert-like, repetition
                        value: UntaggedValue::Table(vals),
                        ..
                    }) =
                        read_dataset(group.dataset(&name).expect("TODO not a group or dataset?"))
                    {
                    }
                }
                Some("values") => {
                    if let Ok(Value {
                        // TODO more assert-like
                        value: UntaggedValue::Table(vals),
                        ..
                    }) =
                        read_dataset(group.dataset(&name).expect("TODO not a group or dataset?"))
                    {
                        // TODO if int headers, may have to convert them for Nu
                        let headers = axes
                            .get(&0)
                            .unwrap()
                            .iter()
                            .map(|v| v.value.expect_string().to_owned())
                            .collect();
                        let values = vals
                            .into_iter()
                            .filter_map(|v| {
                                if let Value {
                                    value: UntaggedValue::Row(dict),
                                    ..
                                } = v
                                {
                                    // TODO cloned here? later?
                                    Some(dict.values().cloned().collect::<Vec<_>>())
                                } else {
                                    None // TODO?
                                }
                            })
                            .collect();

                        // dict.insert_value(name, consolidate_block(headers, values))
                        return Ok(consolidate_block(headers, values));
                    }
                }
                Some(other) => panic!("TODO {}", other),
            },
        }

        // // TODO just blocks?
        // // TODO deal with errors
        // if let Ok(val) = read_dataset(group.dataset(&name).expect("TODO not a group or dataset?")) {
        //     dict.insert_value(name, val);
        // }
    }

    // for (i, val) in axes.into_iter() {
    //     dict.insert_value(i.to_string(), val);
    // }

    // TODO return/add to builder as untagged value instead??
    Ok(dict.into_value())

    // .iter()
    // .filter_map(|name| {
    //     (
    //         name.clone(),
    //
    //     )
    // })
    // .collect::<IndexMap<String, Value>>(),
    // )
    // .into_untagged_value())
}

// TODO make it all async?
fn consolidate_block(headers: Vec<String>, values: Vec<Vec<Value>>) -> Value {
    UntaggedValue::Table(
        values
            .into_iter()
            .map(|row| {
                UntaggedValue::row(
                    headers.iter().cloned().zip(row.into_iter()).collect(), //::<IndexMap<_, _>>(),
                )
                .into_untagged_value()
            })
            .collect(),
    )
    .into_untagged_value() // TODO tag?
} //

// fn read_fixed_ascii(dataset: hdf5::Dataset, size: usize) -> Result<Vec<String>, ShellError> {
//     // TODO break points
//     if size <= 16 {
//         Ok(dataset
//             .as_reader()
//             // TODO problem of fixed length
//             .read_raw::<FixedAscii<[u8; 16]>>()
//             .unwrap()
//             .iter()
//             .map(|fa| fa.to_string())
//             // .map(UntaggedValue::string)
//             .collect())
//     } else if size <= 64 {
//         Ok(dataset
//             .as_reader()
//             // TODO problem of fixed length
//             .read_raw::<FixedAscii<[u8; 64]>>()
//             .unwrap()
//             .iter()
//             .map(|fa| fa.to_string())
//             // .map()
//             .collect())
//     } else {
//         Err(ShellError::unimplemented("big fuckin strings"))
//     }
// }

fn read_dataset(dataset: hdf5::Dataset) -> Result<Value, ShellError> {
    // println!("SHAPE {:?}", dataset.shape());
    let dtype = dataset.dtype().unwrap();
    // println!("TYPE {:?}", dtype.to_descriptor());

    Ok(UntaggedValue::Table(match dtype.to_descriptor().unwrap() {
        // TODO see issue with h5ex_t_vlstringatt.h5
        TypeDescriptor::Integer(_) => {
            // println!("READ {:?}", dataset.read_2d::<i64>());
            // TODO assumes 2d?
            dataset
                .read_dyn::<i64>()
                .unwrap()
                // .clone()
                .outer_iter()
                // .genrows()
                // .into_iter()
                .map(|row| {
                    UntaggedValue::row(
                        row.iter()
                            .enumerate()
                            .map(|(i, val)| {
                                (
                                    format!("Column{}", i),
                                    UntaggedValue::int(*val).into_untagged_value(),
                                )
                            })
                            .collect::<IndexMap<String, Value>>(),
                    )
                    .into_untagged_value()
                })
                .collect::<Vec<_>>()

            // TODO into_untagged_values a problem?

            // Ok(futures::stream::iter(
            //     .to_output_stream());
        }
        TypeDescriptor::FixedAscii(size) => {
            // read_fixed_ascii(dataset, size).map(UntaggedValue::string)
            if size <= 16 {
                dataset
                    .as_reader()
                    // TODO problem of fixed length
                    .read_raw::<FixedAscii<[u8; 16]>>()
                    .unwrap()
                    .iter()
                    .map(|fa| UntaggedValue::string(fa.to_string()).into_untagged_value())
                    // .map(UntaggedValue::string)
                    .collect()
            // ))
            } else if size <= 64 {
                dataset
                    .as_reader()
                    // TODO problem of fixed length
                    .read_raw::<FixedAscii<[u8; 64]>>()
                    .unwrap()
                    .iter()
                    .map(|fa| UntaggedValue::string(fa.to_string()).into_untagged_value())
                    // .map()
                    .collect()
            // ))
            } else {
                return Err(ShellError::unimplemented("big fuckin strings"));
            }

            // return Err(ShellError::unimplemented(""));
        }
        TypeDescriptor::VarLenArray(td) => {
            match *td {
                TypeDescriptor::Unsigned(size) => {
                    // TODO could use size?
                    let val: Vec<_> = dataset
                        .as_reader()
                        .read_raw::<VarLenArray<u8>>()
                        .unwrap()
                        .iter()
                        // .map(|vla| )
                        // .flatten() // TODO what happens with > 1 col
                        .map(|arr| serde_pickle::de::value_from_slice(&arr.to_vec()))
                        // TODO gives shape and values
                        // .map_err(|_| ShellError::unimplemented("TODO some serde malarkey")?
                        .collect();
                    println!("varlen unsigned: {:#?}", val);
                }
                _ => (),
            }

            return Err(ShellError::unimplemented("TODO"));
        }
        other => {
            println!("ignoring {:#?}", other);
            return Err(ShellError::unimplemented("TODO"));
        } // TypeDescriptor::Unsigned(_) => {
          //     // TODO care about smaller ints?
          //     let data = dataset.read_2d::<u64>();
          //     println!("READ {:?}", data);
          // }
          // TypeDescriptor::Float(_) => {
          //     let data = dataset.read_raw::<f64>();
          // }
          // TypeDescriptor::Boolean => {}
          // TypeDescriptor::Enum(_) => {}
          // TypeDescriptor::Compound(_) => {}
          // TypeDescriptor::FixedArray(_, _) => {}

          // TypeDescriptor::FixedUnicode(_) => {}
          // TypeDescriptor::VarLenAscii => {}
          // TypeDescriptor::VarLenUnicode => {}
    })
    .into_untagged_value())
}

// println!("{:?}", dataset.read_raw::<String>());

// println!();}

async fn open(args: CommandArgs) -> Result<OutputStream, ShellError> {
    let scope = args.scope.clone();
    let cwd = PathBuf::from(args.shell_manager.path());
    let shell_manager = args.shell_manager.clone();

    let (
        OpenArgs {
            path,
            raw,
            encoding,
        },
        _,
    ) = args.process().await?;

    // TODO: Remove once Streams are supported everywhere!
    // As a short term workaround for getting AutoConvert and Bat functionality (Those don't currently support Streams)

    // Check if the extension has a "from *" command OR "bat" supports syntax highlighting
    // AND the user doesn't want the raw output
    // In these cases, we will collect the Stream
    let ext = if raw.item {
        None
    } else {
        path.extension()
            .map(|name| name.to_string_lossy().to_string())
    };

    if let Some(ext) = ext {
        // TODO use fetch somehow?
        if ext == "h5" {
            return read_hdf5(path);
        }

        // Check if we have a conversion command
        if let Some(_command) = scope.get_command(&format!("from {}", ext)) {
            let (_, tagged_contents) = crate::commands::open::fetch(
                &cwd,
                &PathBuf::from(&path.item),
                path.tag.span,
                encoding,
            )
            .await?;
            return Ok(OutputStream::one(ReturnSuccess::action(
                CommandAction::AutoConvert(tagged_contents, ext),
            )));
        }
        // Check if bat does syntax highlighting
        if BAT_LANGUAGES.contains(&ext.as_ref()) {
            let (_, tagged_contents) = crate::commands::open::fetch(
                &cwd,
                &PathBuf::from(&path.item),
                path.tag.span,
                encoding,
            )
            .await?;
            return Ok(OutputStream::one(ReturnSuccess::value(tagged_contents)));
        }
    }

    // Normal Streaming operation
    let with_encoding = if encoding.is_none() {
        None
    } else {
        Some(get_encoding(encoding)?)
    };

    let sob_stream = shell_manager.open(&path.item, path.tag.span, with_encoding)?;

    let final_stream = sob_stream.map(move |x| {
        // The tag that will used when returning a Value
        let file_tag = Tag {
            span: path.tag.span,
            anchor: Some(AnchorLocation::File(path.to_string_lossy().to_string())),
        };

        match x {
            Ok(StringOrBinary::String(s)) => {
                ReturnSuccess::value(UntaggedValue::string(s).into_value(file_tag))
            }
            Ok(StringOrBinary::Binary(b)) => ReturnSuccess::value(
                UntaggedValue::binary(b.into_iter().collect()).into_value(file_tag),
            ),
            Err(se) => Err(se),
        }
    });

    Ok(OutputStream::new(final_stream))
}

// Note that we do not output a Stream in "fetch" since it is only used by "enter" command
// Which we expect to use a concrete Value a not a Stream
pub async fn fetch(
    cwd: &PathBuf,
    location: &PathBuf,
    span: Span,
    encoding_choice: Option<Tagged<String>>,
) -> Result<(Option<String>, Value), ShellError> {
    // TODO: I don't understand the point of this? Maybe for better error reporting
    let mut cwd = cwd.clone();
    cwd.push(location);
    let nice_location = dunce::canonicalize(&cwd).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ShellError::labeled_error(
            format!("Cannot find file {:?}", cwd),
            "cannot find file",
            span,
        ),
        std::io::ErrorKind::PermissionDenied => {
            ShellError::labeled_error("Permission denied", "permission denied", span)
        }
        _ => ShellError::labeled_error(
            format!("Cannot open file {:?} because {:?}", &cwd, e),
            "Cannot open",
            span,
        ),
    })?;

    // The extension may be used in AutoConvert later on
    let ext = location
        .extension()
        .map(|name| name.to_string_lossy().to_string());

    // The tag that will used when returning a Value
    let file_tag = Tag {
        span,
        anchor: Some(AnchorLocation::File(
            nice_location.to_string_lossy().to_string(),
        )),
    };

    let res = std::fs::read(location)
        .map_err(|_| ShellError::labeled_error("Can't open filename given", "can't open", span))?;

    // If no encoding is provided we try to guess the encoding to read the file with
    let encoding = if encoding_choice.is_none() {
        UTF_8
    } else {
        get_encoding(encoding_choice.clone())?
    };

    // If the user specified an encoding, then do not do BOM sniffing
    let decoded_res = if encoding_choice.is_some() {
        let (cow_res, _replacements) = encoding.decode_with_bom_removal(&res);
        cow_res
    } else {
        // Otherwise, use the default UTF-8 encoder with BOM sniffing
        let (cow_res, actual_encoding, replacements) = encoding.decode(&res);
        // If we had to use replacement characters then fallback to binary
        if replacements {
            return Ok((ext, UntaggedValue::binary(res).into_value(file_tag)));
        }
        debug!("Decoded using {:?}", actual_encoding);
        cow_res
    };
    let v = UntaggedValue::string(decoded_res.to_string()).into_value(file_tag);
    Ok((ext, v))
}

#[cfg(test)]
mod tests {
    use super::Open;
    use super::ShellError;

    #[test]
    fn examples_work_as_expected() -> Result<(), ShellError> {
        use crate::examples::test as test_examples;

        Ok(test_examples(Open {})?)
    }
}
