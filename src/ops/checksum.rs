use std::{
    borrow::Cow,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use rayon::{iter::ParallelIterator, prelude::ParallelBridge};
use sha2::{Digest, Sha256};

use crate::error::Error;

pub trait Checksum {
    /// compute the hash of the file pointed by the filepath by using [io::copy] between a file handler and the hasher.
    /// As such, it shouldn't make the program go OOM with big files, but it has not been tested.
    /// Can return an error if there has been problems regarding IO.
    #[inline]
    fn get_hash<R>(reader: &mut R, hasher: &mut Sha256) -> Result<String, Error>
    where
        R: std::io::Read,
    {
        io::copy(reader, hasher)?;
        let result = format!("{:x}", hasher.finalize_reset());
        Ok(result)
    }

    /// corpus/lang/lang_part_x.jsonl
    #[inline]
    fn get_hash_path(src: &Path, hasher: &mut Sha256) -> Result<String, Error> {
        let mut f = File::open(src)?;
        Self::get_hash(&mut f, hasher)
    }

    /// this should operate on the wide-level.
    fn checksum_folder(src: &Path, num_threads: usize) -> Result<(), Error> {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()?;
        if src.is_file() {
            // TODO #86442 merged
            // return Err(io::Error::new(
            //     io::ErrorKind::IsADirectory,
            //     format!("{}", src),
            // ));
            error!("Checksum only works on folders!");
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{:?}", src)).into());
        }

        let language_dirs = std::fs::read_dir(src)?
            .filter_map(|entry| {
                // check entry validity
                let entry = match entry {
                    Ok(e) => e.path(),
                    Err(e) => {
                        error!("error with directory entry {:?}", e);
                        return None;
                    }
                };

                // filter out files
                if !entry.is_dir() {
                    warn!("{:?} is not a directory: ignoring checksum op", entry);
                    None
                } else {
                    Some(entry)
                }
            })
            .into_iter();

        let language_dirs_par = language_dirs.par_bridge();
        language_dirs_par.for_each(|language_dir| match Self::get_write_hashes(&language_dir) {
            Ok(_) => (),
            Err(e) => error!("Error with directory {:?}: {:?}", language_dir, e),
        });
        Ok(())
    }

    #[inline]
    /// convinience function for checksum_folder
    /// TODO: move out of trait
    fn get_write_hashes(src: &Path) -> Result<(), Error> {
        debug!("getting hashes");
        let hashes = Self::checksum_lang(src)?;
        let checksum_filepath = src.clone().join("checksum.sha256");
        debug!("writing checksum file");
        let mut checksum_file = File::create(&checksum_filepath)?;
        Self::write_checksum(&mut checksum_file, hashes)?;
        Ok(())
    }
    fn write_checksum<W: Write>(
        writer: &mut W,
        hashes: Vec<(PathBuf, String)>,
    ) -> Result<(), Error> {
        for (path, hash) in hashes {
            if let Some(filename) = path.file_name() {
                let filename = if let Some(filename_string) = filename.to_str() {
                    Cow::from(filename_string)
                } else {
                    let filename_string = filename.to_string_lossy();
                    warn!(
                        "could not convert path to string: {:?}, using {} in replacement.",
                        filename, filename_string
                    );
                    filename_string
                };
                writeln!(writer, "{} {}", hash, filename)?;
            } else {
                warn!("Could not get filename for {:?}: ignoring in checksum. Add manually if necessary.", path);
            }
        }
        Ok(())
    }
    /// this should operate on lang-level
    fn checksum_lang(src: &Path) -> Result<Vec<(PathBuf, String)>, Error> {
        let mut hasher = Sha256::new();
        let mut hashes = Vec::new();
        for filepath in std::fs::read_dir(src)? {
            let filepath = filepath?.path();
            let hash = Self::get_hash_path(&filepath, &mut hasher)?;
            hashes.push((filepath, hash));
        }
        Ok(hashes)
    }
}

#[cfg(test)]
mod tests {
    use sha2::Digest;
    use std::fs::File;
    use std::io::Write;

    use sha2::Sha256;

    use crate::error::Error;
    use crate::ops::Checksum;

    #[test]
    fn test_checksum_folder() -> Result<(), Error> {
        struct DummyChecksum;
        impl Checksum for DummyChecksum {}

        let corpus_dir = tempfile::tempdir().unwrap();

        let (langs, contents): (Vec<&str>, Vec<&str>) = [
            ("fr", r#"{{"content":"foo_french"}}"#),
            ("en", r#"{{"content":"foo_english"}}"#),
            ("de", r#"{{"content":"foo_german"}}"#),
            ("es", r#"{{"content":"foo_spanish"}}"#),
        ]
        .iter()
        .cloned()
        .unzip();
        for (lang, content) in langs.iter().zip(contents.iter()) {
            let path = corpus_dir.path();
            let lang_dir = path.join(lang);
            std::fs::create_dir(&lang_dir)?;
            let lang_text_file = lang_dir.clone().join(format!("{lang}.jsonl"));
            let mut f = File::create(&lang_text_file)?;
            write!(&mut f, "{content}")?;
        }

        for (lang, content) in langs.iter().zip(contents) {
            // corpora are not split, so there's only one file (hence [0]). We then take the hash (hence .1)
            let hash = &DummyChecksum::checksum_lang(&corpus_dir.path().join(lang))?[0].1;
            let expected = {
                let mut hasher = Sha256::new();
                let mut reader = content.as_bytes();
                DummyChecksum::get_hash(&mut reader, &mut hasher)?
            };

            assert_eq!(hash, &expected);
        }

        Ok(())
    }
}
