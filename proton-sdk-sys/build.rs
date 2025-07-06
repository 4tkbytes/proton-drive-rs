use std::io::Result;

fn main() -> Result<()> {
    prost_build::Config::new()
        .compile_protos(
            &[
                "protos/account.proto",
                "protos/drive.proto"
            ],
            &["protos/"]
        )?;
    
    println!("cargo:rerun-if-changed=protos/account.proto");
    println!("cargo:rerun-if-changed=protos/drive.proto");
    Ok(())
}