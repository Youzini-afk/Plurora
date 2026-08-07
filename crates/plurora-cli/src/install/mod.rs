pub mod consent;
pub mod url_parser;

pub fn default_data_dir() -> std::path::PathBuf {
    plurora_core::paths::data_dir().expect("could not resolve Plurora data dir")
}
