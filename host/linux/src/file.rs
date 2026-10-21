//! Open regular asset/font files without blocking on a substituted Unix FIFO.
use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

pub(crate) fn open_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(file)
}
