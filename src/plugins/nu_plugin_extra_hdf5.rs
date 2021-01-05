use nu_plugin::serve_plugin;
use nu_plugin_hdf5::Hdf5;

fn main() {
    serve_plugin(&mut Hdf5::new())
}
