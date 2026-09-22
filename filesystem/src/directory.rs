// @ref LLP 1030.002 D1 — every name is opened relative to an owned directory.
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::path::{Component, Path};

pub fn refuse(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
fn name(value: &str) -> io::Result<CString> {
    CString::new(value).map_err(|_| refuse("NUL in path"))
}
fn checked(value: i32) -> io::Result<i32> {
    if value < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(value)
    }
}
fn open(parent: &File, value: &str, flags: i32) -> io::Result<File> {
    let value = name(value)?;
    // SAFETY: value is NUL terminated; parent remains owned for this call.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            value.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o644,
        )
    };
    let fd = checked(fd).map_err(|error| {
        if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
            refuse("static app files cannot be symlinks; path must be a real directory or regular file")
        } else { error }
    })?;
    // SAFETY: openat returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub struct Directory(pub File);
impl Directory {
    /// Visit each regular file through its owned parent, refusing links and special files.
    pub fn visit_files(
        &self,
        prefix: &str,
        visit: &mut impl FnMut(&str, &Directory, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        for leaf in self.names()? {
            let path = format!("{prefix}{leaf}");
            match self.kind(&leaf)?.st_mode & libc::S_IFMT {
                libc::S_IFDIR => self
                    .child(&leaf, false)?
                    .visit_files(&format!("{path}/"), visit)?,
                libc::S_IFREG => visit(&path, self, &leaf)?,
                _ => {
                    return Err(refuse(
                        "static app files must be regular files or directories",
                    ))
                }
            }
        }
        Ok(())
    }

    pub fn root(path: &str, create: bool) -> io::Result<Self> {
        let mut path = path.to_owned();
        // macOS's fixed system aliases are normalized without consulting a
        // mutable symlink. No user-controlled path component is resolved.
        if cfg!(target_os = "macos") {
            for alias in ["var", "tmp", "etc"] {
                if path == format!("/{alias}") || path.starts_with(&format!("/{alias}/")) {
                    path = format!("/private{path}");
                    break;
                }
            }
        }
        if !Path::new(&path).is_absolute() {
            return Err(refuse("root must be absolute"));
        }
        let mut dir = Self(File::open("/")?);
        for part in Path::new(&path).components() {
            match part {
                Component::RootDir => {}
                Component::Normal(part) => {
                    dir = dir.child(
                        part.to_str().ok_or_else(|| refuse("non-UTF8 path"))?,
                        create,
                    )?
                }
                _ => return Err(refuse("invalid root component")),
            }
        }
        Ok(dir)
    }
    pub fn child(&self, leaf: &str, create: bool) -> io::Result<Self> {
        let result = open(&self.0, leaf, libc::O_RDONLY | libc::O_DIRECTORY);
        if !create || result.as_ref().err().and_then(io::Error::raw_os_error) != Some(libc::ENOENT)
        {
            return result.map(Self);
        }
        let cname = name(leaf)?;
        // SAFETY: owned parent and NUL-terminated single child name.
        let made = unsafe { libc::mkdirat(self.0.as_raw_fd(), cname.as_ptr(), 0o755) };
        if made < 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            checked(made)?;
        }
        open(&self.0, leaf, libc::O_RDONLY | libc::O_DIRECTORY).map(Self)
    }
    pub fn parent(&self, relative: &str, create: bool) -> io::Result<(Self, String)> {
        let parts: Vec<_> = relative.split('/').collect();
        if parts.iter().any(|part| {
            part.is_empty() || *part == "." || *part == ".." || part.contains(['\\', '\0'])
        }) {
            return Err(refuse("not a relative filesystem path"));
        }
        let mut dir = Self(self.0.try_clone()?);
        for part in &parts[..parts.len() - 1] {
            dir = dir.child(part, create)?;
        }
        Ok((dir, parts.last().unwrap().to_string()))
    }
    pub fn file(&self, leaf: &str, flags: i32) -> io::Result<File> {
        let file = open(&self.0, leaf, flags)?;
        if !file.metadata()?.is_file() {
            return Err(refuse("static app files must be regular files"));
        }
        Ok(file)
    }
    pub fn read(&self, leaf: &str) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.file(leaf, libc::O_RDONLY)?.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
    pub fn names(&self) -> io::Result<Vec<String>> {
        let fd = open(&self.0, ".", libc::O_RDONLY | libc::O_DIRECTORY)?.into_raw_fd();
        // SAFETY: dup creates a new fd; fdopendir assumes its ownership.
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            unsafe { libc::close(fd) };
            return Err(io::Error::last_os_error());
        }
        struct Entries(*mut libc::DIR);
        impl Drop for Entries {
            fn drop(&mut self) {
                unsafe { libc::closedir(self.0) };
            }
        }
        let entries = Entries(stream);
        let mut names = Vec::new();
        loop {
            // SAFETY: stream is owned and each name is copied before readdir advances.
            #[cfg(target_os = "macos")]
            let errno = unsafe { libc::__error() };
            #[cfg(target_os = "linux")]
            let errno = unsafe { libc::__errno_location() };
            unsafe { *errno = 0 };
            let entry = unsafe { libc::readdir(entries.0) };
            if entry.is_null() {
                let code = unsafe { *errno };
                if code != 0 {
                    return Err(io::Error::from_raw_os_error(code));
                }
                break;
            }
            let leaf = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_str()
                .map_err(|_| refuse("non-UTF8 filename"))?;
            if leaf != "." && leaf != ".." {
                names.push(leaf.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }
    pub fn kind(&self, leaf: &str) -> io::Result<libc::stat> {
        let leaf = name(leaf)?;
        let mut info = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fstatat initializes info on success, without following links.
        checked(unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                leaf.as_ptr(),
                info.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        })?;
        let info = unsafe { info.assume_init() };
        if info.st_mode & libc::S_IFMT == libc::S_IFLNK {
            return Err(refuse(
                "static app files cannot be symlinks; root must be a real directory",
            ));
        }
        Ok(info)
    }
    pub fn unlink(&self, leaf: &str) -> io::Result<()> {
        checked(unsafe { libc::unlinkat(self.0.as_raw_fd(), name(leaf)?.as_ptr(), 0) })?;
        Ok(())
    }
    pub fn write(
        &self,
        leaf: &str,
        bytes: &[u8],
        immutable: bool,
        token: &str,
    ) -> io::Result<&'static str> {
        self.write_checked(leaf, bytes, immutable, token, || Ok(()))
    }
    pub fn write_checked(
        &self,
        leaf: &str,
        bytes: &[u8],
        immutable: bool,
        token: &str,
        before_commit: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<&'static str> {
        self.write_with_sync(leaf, bytes, immutable, token, before_commit, true)
    }
    /// Immutable development-cache writes retain complete-file visibility,
    /// but do not promise recovery after a machine crash or power loss.
    pub fn write_cached(
        &self,
        leaf: &str,
        bytes: &[u8],
        token: &str,
        before_commit: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<&'static str> {
        self.write_with_sync(leaf, bytes, true, token, before_commit, false)
    }
    fn write_with_sync(
        &self,
        leaf: &str,
        bytes: &[u8],
        immutable: bool,
        token: &str,
        before_commit: impl FnOnce() -> io::Result<()>,
        synchronize: bool,
    ) -> io::Result<&'static str> {
        match self.kind(leaf) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        if token.len() != 48 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(refuse("invalid temporary-file token"));
        }
        let temp = format!(".tmp-{}-{token}", std::process::id());
        let mut file = self.file(&temp, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
        let result = (|| {
            file.write_all(bytes)?;
            if synchronize {
                file.sync_all()?;
            }
            before_commit()?;
            match self.kind(leaf) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            let from = name(&temp)?;
            let to = name(leaf)?;
            if immutable {
                let linked = unsafe {
                    libc::linkat(
                        self.0.as_raw_fd(),
                        from.as_ptr(),
                        self.0.as_raw_fd(),
                        to.as_ptr(),
                        0,
                    )
                };
                if linked < 0 {
                    if io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST)
                        && self.read(leaf)? == bytes
                    {
                        return Ok("present");
                    }
                    return Err(refuse("immutable file exists with other bytes"));
                }
            } else {
                checked(unsafe {
                    libc::renameat(
                        self.0.as_raw_fd(),
                        from.as_ptr(),
                        self.0.as_raw_fd(),
                        to.as_ptr(),
                    )
                })?;
            }
            if synchronize {
                self.0.sync_all()?;
            }
            Ok("written")
        })();
        let _ = self.unlink(&temp);
        result
    }
}
