//! Repackage a private Workbench 1.75 manifest into app model records.
use sha2::{Digest, Sha256};
use std::{env, path::Path};
use variax_workbench::{document::save_new, host::ModelLibrary};

fn main() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let [manifest, output] = args.as_slice() else {
        return Err("usage: convert_variax_models MODELS.tsv NEW.vxm.zlib".into());
    };
    let models = ModelLibrary::load_manifest(Path::new(manifest))?;
    let bytes = models.encode_model_bundle()?;
    save_new(Path::new(output), &bytes)?;
    println!(
        "{} bodies, {} pickups, {} special responses. {} bytes. SHA-256 {:x}",
        models.bodies.len(),
        models.pickups.len(),
        models.special_responses.len(),
        bytes.len(),
        Sha256::digest(&bytes),
    );
    Ok(())
}
