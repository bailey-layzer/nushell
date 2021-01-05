use nu_errors::ShellError;
use nu_plugin::Plugin;
use nu_protocol::{CallInfo, ReturnValue, Signature, UntaggedValue};

use crate::Hdf5;

impl Plugin for Hdf5 {
    fn config(&mut self) -> Result<Signature, ShellError> {
        Ok(Signature::build("hdf5").desc("Open HDF5 file").filter())
    }

    fn begin_filter(&mut self, callinfo: CallInfo) -> Result<Vec<ReturnValue>, ShellError> {
        Ok(vec![Ok(ReturnSuccess::value(
            UntaggedValue::string("hello world").into_untagged_value(),
        ))])
    }
}
