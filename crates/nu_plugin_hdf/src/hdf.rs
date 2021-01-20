use nu_errors::ShellError;
use nu_plugin::Plugin;
use nu_protocol::{
    CallInfo, Primitive, ReturnSuccess, ReturnValue, Signature, SyntaxShape, TaggedDictBuilder,
    UntaggedValue, Value,
};
use nu_source::{Span, Tag, Tagged};

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
    unsafe {
        // turn off unwanted output to stderr by hdf5 C library should file not exist
        H5Eset_auto2(H5E_DEFAULT, None, std::ptr::null_mut());
    }

    return match hdf5::File::open(path.as_path()) {
        Ok(file) => {
            // TODO anything with plist? how to get encoding (not here)?
            // eprintln!("{:#?}", file.access_plist().unwrap().properties());

            // TODO what happens to error?
            // dereferencing a File makes a Group
            ReturnSuccess::value(read_group(&*file)?.into_value(path.tag))
            // TODO correct tag usage?

            // return Ok(OutputStream::empty());
        }
        Err(e) => Err(ShellError::labeled_error(
            // TODO {:?} shows flags etc
            format!("Cannot open file as HDF5: {:?}", e),
            "error opening file",
            path.tag.clone(),
        )),
    };
}

fn read_group(group: &hdf5::Group) -> Result<UntaggedValue, ShellError> {
    let members = group.member_names().map_err(|e| {
        // TODO error phrasing
        ShellError::untagged_runtime_error(format!("Problem reading HDF members: {:?}", e))
    })?;

    // TODO handle singleton dataset differently?

    // TODO should the tag be the file path??
    let mut dict = TaggedDictBuilder::new(Tag::unknown());
    let mut datasets: Vec<String> = Vec::new();
    let re_axis = Regex::new(r"axis(\d)(?:_(label|level)(\d))?").expect("literal regex");

    unsafe {
        // turn off unwanted output to stderr by hdf5 C library when calling .group below
        //  TODO is there a way to not call this repeatedly?
        H5Eset_auto2(H5E_DEFAULT, None, std::ptr::null_mut());
    }

    for name in members {
        match group.group(&name) {
            // TODO need to tag recursive tables??
            // TODO different handling of recursive error?
            Ok(group) => dict.insert_value(name, read_group(&group)?),
            Err(_) => datasets.push(name),
        }
    }

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
                            // eprintln!("{:#?}", axes.get(&n));
                        }
                    }
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
                            .map(|v| v.convert_to_string())
                            .collect();
                        let values = vals
                            .into_iter()
                            .filter_map(|v| {
                                if let Value {
                                    // TODO this is now standardized to table
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
    Ok(dict.into_untagged_value())
}

// TODO make it all async?
fn consolidate_block(headers: Vec<String>, values: Vec<Vec<Value>>) -> UntaggedValue {
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
    // .into_untagged_value() // TODO tag?
} //

fn read_dataset(dataset: hdf5::Dataset) -> Result<Value, ShellError> {
    // println!("SHAPE {:?}", dataset.shape());
    let dtype = dataset.dtype().unwrap();
    // println!("TYPE {:?}", dtype.to_descriptor());

    macro_rules! read_type {
        ($read_type:ty, $untagged:path) => {
            dataset
                // TODO does >2d exist?
                .read_dyn::<$read_type>()
                .unwrap()
                .outer_iter()
                .map(|row| {
                    UntaggedValue::Table(
                        row.iter()
                            .map(|val| $untagged(*val).into_untagged_value())
                            .collect::<Vec<_>>(),
                    )
                    // TODO into_untagged_values a problem?
                    //  (will tagging happen later regardless)
                    .into_untagged_value()
                })
                .collect::<Vec<_>>()
        };
    }

    Ok(UntaggedValue::Table(match dtype.to_descriptor().unwrap() {
        // TODO see issue with h5ex_t_vlstringatt.h5
        TypeDescriptor::Integer(_) => {
            read_type!(i64, UntaggedValue::int)
        }
        TypeDescriptor::Unsigned(_) => {
            // TODO care about smaller ints?
            read_type!(u64, UntaggedValue::int)
        }
        TypeDescriptor::Float(_) => {
            let to_untagged = |f: f64| UntaggedValue::decimal_from_float(f, Span::unknown());
            read_type!(f64, to_untagged)
        }
        TypeDescriptor::Boolean => {
            read_type!(bool, UntaggedValue::boolean)
        }
        // TypeDescriptor::Enum(_) => {}
        // TypeDescriptor::Compound(_) => {}
        // TypeDescriptor::FixedArray(_, _) => {}
        TypeDescriptor::FixedAscii(size) => {
            // read_fixed_ascii(dataset, size).map(UntaggedValue::string)
            if size <= 16 {
                read_type!(FixedAscii<[u8; 16]>, UntaggedValue::string)
            } else if size <= 64 {
                read_type!(FixedAscii<[u8; 64]>, UntaggedValue::string)
            } else {
                return Err(ShellError::unimplemented("TODO big fuckin strings"));
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
        } // TypeDescriptor::FixedUnicode(_) => {}
          // TypeDescriptor::VarLenAscii => {}
          // TypeDescriptor::VarLenUnicode => {}
    })
    .into_untagged_value())
}
