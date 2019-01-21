extern crate capnpc;

fn main() {
    ::capnpc::CompilerCommand::new()
        .file("schema/pubsub.capnp")
        .run()
        .unwrap();
}
