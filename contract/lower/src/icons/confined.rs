//! A package's file read only where it is its own (LLP 1121 stage 13,
//! review rounds 1 to 3): under the package's own `assets/`, links
//! followed, and read from the very file that was checked.

use std::io::Read;
use std::path::Path;

/// Why a package's file was not read.
#[derive(Debug)]
pub enum Unread {
    /// Nothing is at the path (no file, no link): missing, said so.
    Missing(std::io::Error),
    /// It leaves the package: a link out of its `assets/`, one that can't
    /// be resolved (dangling), or a file swapped while it was checked.
    Leaves,
    /// It is the package's, and couldn't be read.
    Failed(std::io::Error),
}

/// The bytes of `path`, a file under `root`'s `assets/`, read only when it
/// is the package's own:
/// - nothing at the path is [`Unread::Missing`], with no second open;
/// - the file is opened (links followed) and its descriptor's identity
///   taken; then the path is resolved again and must be under the canonical
///   `<root>/assets/` and be that same file (device and inode), else
///   [`Unread::Leaves`]; and the bytes come from the descriptor opened.
///
/// What this closes: a link planted, or swapped in after the check, that
/// points out of the package (the open follows it, and the resolution after
/// it names the outside file); a parent directory swapped between the open
/// and the re-check (the resolved file is another inode). What it doesn't:
/// a file inside the package's `assets/` replaced by another file there
/// (still the package's own bytes); on a host without inodes the identity
/// check, where only the resolution after the open is compared; and a
/// three-swap interleaving (narrow re-check, RE-3; QUEUE): the path a link
/// out when it is opened, a plain file when it is resolved (so it resolves
/// to itself, inside), and a link out again before that resolved path's
/// metadata is read, which then follows the link to the opened outside
/// file and passes, so the outside bytes are read. It takes three swaps by
/// a writer of the package's `assets/` within a few system calls.
pub fn read(path: &Path, root: &Path) -> Result<Vec<u8>, Unread> {
    if let Err(e) = path.symlink_metadata() {
        return Err(match e.kind() {
            std::io::ErrorKind::NotFound => Unread::Missing(e),
            _ => Unread::Failed(e),
        });
    }
    // A link whose target isn't there can't be told inside.
    let mut file = std::fs::File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Unread::Leaves,
        _ => Unread::Failed(e),
    })?;
    let opened = file.metadata().map_err(Unread::Failed)?;
    let real = path.canonicalize().map_err(|_| Unread::Leaves)?;
    let assets = root
        .canonicalize()
        .map_err(|_| Unread::Leaves)?
        .join("assets");
    if !real.starts_with(&assets) {
        return Err(Unread::Leaves);
    }
    let checked = std::fs::metadata(&real).map_err(|_| Unread::Leaves)?;
    if !same(&opened, &checked) {
        return Err(Unread::Leaves);
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(Unread::Failed)?;
    Ok(bytes)
}

#[cfg(unix)]
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    (a.dev(), a.ino()) == (b.dev(), b.ino())
}

#[cfg(not(unix))]
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.len() == b.len() && a.is_file() == b.is_file()
}

#[cfg(all(test, unix))]
mod tests {
    use super::{read, Unread};

    #[test]
    fn only_the_packages_own_file_is_read() {
        let dir = std::env::temp_dir().join(format!("exact-confined-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (package, outside) = (dir.join("pkg"), dir.join("app"));
        std::fs::create_dir_all(package.join("assets")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(package.join("assets/own.svg"), "own").unwrap();
        std::fs::write(outside.join("star.svg"), "app").unwrap();
        let assets = package.join("assets");
        assert_eq!(read(&assets.join("own.svg"), &package).unwrap(), b"own");
        // A link planted at the path, out of the package.
        std::os::unix::fs::symlink(outside.join("star.svg"), assets.join("star.svg")).unwrap();
        assert!(matches!(
            read(&assets.join("star.svg"), &package),
            Err(Unread::Leaves)
        ));
        // A link inside its own `assets/` is its own.
        std::os::unix::fs::symlink(assets.join("own.svg"), assets.join("alias.svg")).unwrap();
        assert_eq!(read(&assets.join("alias.svg"), &package).unwrap(), b"own");
        // Dangling: leaves; nothing there: missing.
        std::os::unix::fs::symlink(dir.join("nowhere"), assets.join("dangling.svg")).unwrap();
        assert!(matches!(
            read(&assets.join("dangling.svg"), &package),
            Err(Unread::Leaves)
        ));
        assert!(matches!(
            read(&assets.join("absent.svg"), &package),
            Err(Unread::Missing(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
