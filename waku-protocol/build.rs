extern crate protoc_rust;
use std::env;

fn main() {
    let mut protos_path = env::current_dir().unwrap();
    protos_path.push("src/pb");

    // absolute paths for .protos
    let waku_message_proto_path = [
        protos_path.display().to_string(),
        "waku_message.pb.proto".to_string(),
    ]
    .join("/");
    let waku_store_proto_path = [
        protos_path.display().to_string(),
        "waku_store.pb.proto".to_string(),
    ]
    .join("/");
    let waku_lightpush_proto_path = [
        protos_path.display().to_string(),
        "waku_lightpush.pb.proto".to_string(),
    ]
    .join("/");

    protoc_rust::Codegen::new()
        .out_dir(protos_path.display().to_string())
        .inputs(&[
            waku_message_proto_path,
            waku_store_proto_path,
            waku_lightpush_proto_path,
        ])
        .include(protos_path.display().to_string())
        .run()
        .expect("Running protoc failed.");
}
