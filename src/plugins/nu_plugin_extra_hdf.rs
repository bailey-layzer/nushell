use nu_plugin::serve_plugin;
use nu_plugin_hdf::Hdf;

fn main() {
    serve_plugin(&mut Hdf::new())
}
