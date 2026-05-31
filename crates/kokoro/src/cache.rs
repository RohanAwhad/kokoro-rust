use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::Result;

pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(crate::types::CACHE_DIR_NAME)
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

pub fn download_file(url: &str, dest: &Path) -> Result<()> {
    log::info!("Downloading: {url}");

    if let Some(parent) = dest.parent() {
        ensure_dir(parent)?;
    }

    let response = ureq::get(url).call().map_err(Box::new)?;
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body)?;

    std::fs::write(dest, &body)?;
    log::info!("Downloaded to: {}", dest.display());
    Ok(())
}
