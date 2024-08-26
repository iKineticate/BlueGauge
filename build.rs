extern crate embed_resource;

fn main() {
    embed_resource::compile("resources/logo.rc", embed_resource::NONE);
}