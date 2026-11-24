//! Offline research runner for one ordered batch of complete preset files.
use std::{env, fs, path::Path};

use variax_workbench::{
    document::save_new,
    host::{HostEngine, HostOptions, HostParameters, ModelLibrary},
    preset::PresetFile,
};

fn main() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let [list, models, output] = args.as_slice() else {
        return Err("usage: render_preset_corpus PRESETS.txt MODELS.tsv NEW.packed".into());
    };
    let list = Path::new(list);
    let paths = fs::read_to_string(list).map_err(|error| error.to_string())?;
    let library = ModelLibrary::load_manifest(Path::new(models))?;
    let mut engine = HostEngine::new(&library);
    let directory = list.parent().ok_or("preset list has no parent")?;
    let mut batch = Vec::new();
    for (index, path) in paths.lines().enumerate() {
        let mut render = || {
            let bytes = fs::read(directory.join(path)).map_err(|error| error.to_string())?;
            let preset = PresetFile::parse(bytes)?;
            let parameters = HostParameters::from_preset(&preset)?;
            let settings = parameters.map(HostOptions::default())?;
            let packed = engine.generate(&settings)?.packed();
            Ok::<_, String>(parameters.finish(preset.name(), &settings, packed))
        };
        match render() {
            Ok(packed) => batch.extend_from_slice(&packed),
            Err(error) => {
                println!("{index}\t{error}");
                batch.extend_from_slice(&[0; variax_workbench::dsp::PACKED_BYTES]);
            }
        }
    }
    save_new(Path::new(output), &batch)
}
