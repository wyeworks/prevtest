//! Computes file content hashes and combines test inputs into cache keys.

use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsString,
    fs::File,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub struct FileIdentity {
    pub path: PathBuf,
    pub hash: blake3::Hash,
}

impl FileIdentity {
    pub fn read(path: PathBuf) -> Result<Self> {
        let metadata = path
            .metadata()
            .with_context(|| format!("reading {}", path.display()))?;
        ensure!(
            metadata.is_file(),
            "{} is not a regular file",
            path.display()
        );
        let mut file = File::open(&path)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update_reader(&mut file)?;
        Ok(Self {
            path,
            hash: hasher.finalize(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CacheKey(pub [u8; 32]);

impl CacheKey {
    pub fn new(
        binary: &FileIdentity,
        binary_id: &str,
        test_name: &str,
        working_dir: &Path,
        workspace: &Path,
        args: &[OsString],
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        let mut field = |bytes: &[u8]| {
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        };
        field(&1_u16.to_le_bytes());
        field(binary.hash.as_bytes());
        field(binary_id.as_bytes());
        field(test_name.as_bytes());
        let cwd = working_dir.strip_prefix(workspace).unwrap_or(working_dir);
        field(cwd.as_os_str().as_bytes());
        for arg in args {
            field(arg.as_bytes());
        }
        Self(*hasher.finalize().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(args: &[&str]) -> CacheKey {
        let binary = FileIdentity {
            path: "/workspace/test-binary".into(),
            hash: blake3::hash(b"binary contents"),
        };
        let args: Vec<_> = args.iter().map(OsString::from).collect();
        CacheKey::new(
            &binary,
            "probe::integration",
            "tests::example",
            Path::new("/workspace"),
            Path::new("/workspace"),
            &args,
        )
    }

    #[test]
    fn argument_boundaries_and_order_affect_the_key() {
        assert_ne!(key(&["ab", "c"]), key(&["a", "bc"]));
        assert_ne!(key(&["a", "b"]), key(&["b", "a"]));
        assert_ne!(key(&[]), key(&[""]));
    }
}
