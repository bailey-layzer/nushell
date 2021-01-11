use crate::hdf::{read_hdf, Hdf};
use nu_errors::ShellError;
use nu_plugin::Plugin;
use nu_protocol::{
    CallInfo, Primitive, ReturnSuccess, ReturnValue, Signature, SyntaxShape, TaggedDictBuilder,
    UntaggedValue, Value,
};
use nu_source::TaggedItem;

use futures::executor::block_on;

impl Plugin for Hdf {
    fn config(&mut self) -> Result<Signature, ShellError> {
        Ok(Signature::build("hdf")
            .desc("Open HDF5 file")
            .required("path", SyntaxShape::FilePath, "file path to open")
            .filter())
    }

    fn begin_filter(&mut self, callinfo: CallInfo) -> Result<Vec<ReturnValue>, ShellError> {
        if let Some(args) = callinfo.args.positional {
            if let Value {
                // TODO needed, yes?
                value: UntaggedValue::Primitive(Primitive::FilePath(pathbuf)),
                tag,
            } = &args[0]
            {
                // println!("pathbuf: {:#?}", pathbuf);
                // TODO async?
                return Ok(vec![block_on(read_hdf(pathbuf.to_owned().tagged(tag)))]);
            }

            return Err(ShellError::labeled_error(
                "Could not open HDF5 file",
                "TODO not a file",
                &args[0].tag.clone(),
            ));
        }

        Ok(vec![])
    }
}
