use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
};

const MAX_JSON_BYTES: u64 = 128 * 1024 * 1024;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("opening {}", path.display())),
    };
    if file.metadata()?.len() > MAX_JSON_BYTES {
        bail!("{} exceeds the maximum snapshot size", path.display());
    }
    let mut reader = BufReader::new((&mut file).take(MAX_JSON_BYTES + 1));
    let data = serde_json::from_reader(&mut reader)
        .with_context(|| format!("reading snapshot {}", path.display()))?;
    Ok(Some(data))
}

pub fn write_json(path: &Path, data: &impl Serialize) -> Result<()> {
    replace(path, |writer| {
        serde_json::to_writer(&mut *writer, data)?;
        writer.write_all(b"\n")?;
        Ok(())
    })
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    replace(path, |writer| Ok(writer.write_all(bytes)?))
}

fn replace(path: &Path, write: impl FnOnce(&mut BufWriter<&mut File>) -> Result<()>) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staging = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temporary file for {}", path.display()))?;
    let mut writer = BufWriter::new(staging.as_file_mut());
    write(&mut writer).with_context(|| format!("writing {}", path.display()))?;
    writer.flush()?;
    drop(writer);
    if staging.as_file().metadata()?.len() > MAX_JSON_BYTES {
        bail!("{} exceeds the maximum snapshot size", path.display());
    }
    staging.as_file().sync_all()?;
    staging
        .persist(path)
        .with_context(|| format!("replacing {}", path.display()))?;
    #[cfg(unix)]
    if let Err(error) = File::open(parent).and_then(|directory| directory.sync_all()) {
        tracing::warn!(path = %parent.display(), %error, "could not flush snapshot directory");
    }
    Ok(())
}

pub fn append_json_line(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&line)?;
    file.sync_data()?;
    Ok(())
}

pub fn lock_run(path: &Path) -> Result<File> {
    fs::create_dir_all(path).with_context(|| format!("creating save {}", path.display()))?;
    let lock_path = path.join(".lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock).with_context(|| {
        format!(
            "save {} is already open or could not be locked",
            path.display()
        )
    })?;
    Ok(lock)
}
