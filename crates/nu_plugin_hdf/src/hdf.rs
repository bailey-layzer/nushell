use nu_errors::ShellError;
use nu_plugin::Plugin;
use nu_protocol::{
    CallInfo, Primitive, ReturnSuccess, ReturnValue, Signature, SyntaxShape, TaggedDictBuilder,
    UntaggedValue, Value,
};
use nu_source::{Tag, Tagged};

use hdf5::types::{Array, FixedAscii, TypeDescriptor, VarLenArray, VarLenAscii};
use hdf5_sys::h5e::{H5Eset_auto2, H5E_DEFAULT};
use indexmap::IndexMap;
use regex::Regex;
use std::path::PathBuf;

pub struct Hdf {
    // TODO what is actually done with this?
    path: PathBuf,
}

// TODO Handler? Write capabilities?
impl Hdf {
    pub fn new() -> Hdf {
        Hdf {
            path: PathBuf::new(),
        }
    }
}

pub(crate) async fn read_hdf(path: Tagged<PathBuf>) -> ReturnValue {
    return match hdf5::File::open(path.as_path()) {
        Ok(file) => {
            // TODO anything with plist? how to get encoding (not here)?
            // println!("{:#?}", file.access_plist().unwrap().properties());

            // TODO what happens to error?
            // dereferencing a File makes a Group
            ReturnSuccess::value(read_group(&*file)?)

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
}

fn read_group(group: &hdf5::Group) -> Result<Value, ShellError> {
    let members = group.member_names().map_err(|e| {
        ShellError::untagged_runtime_error(format!("problem reading HDF file: {:?}", e))
    })?;

    // println!("{:#?}", members);
    // TODO handle only one dataset differently?

    // TODO with_capacity?
    let mut dict = TaggedDictBuilder::new(Tag::unknown()); // TODO is it known?
    let mut datasets: Vec<String> = Vec::new();
    let re_axis = Regex::new(r"axis(\d)(?:_(label|level)(\d))?").expect("literal regex");

    unsafe {
        // turns off unwanted output to stderr by hdf5 C library
        H5Eset_auto2(H5E_DEFAULT, None, std::ptr::null_mut());
    }

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
                    // println!("ignoring label");
                    continue;
                } // TODO are these needed?
                Some("level") => {
                    // println!("ignoring level");
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

    // println!("{:#?}", dict);

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
                    eprintln!("varlen unsigned: {:#?}", val);
                }
                _ => (),
            }

            return Err(ShellError::unimplemented("TODO"));
        }
        other => {
            eprintln!("ignoring {:#?}", other);
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
