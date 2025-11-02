// The purpose of this file is to compile the protobuf files into Rust code.
// run: `cargo build` at the root of the project

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir("src/rpc")
        .compile_protos(&["proto/lightning.proto"], &["proto"])
        .unwrap();
    Ok(())
}
