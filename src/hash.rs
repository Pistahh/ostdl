use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::num::Wrapping;
use std::{io, mem};

const CHUNKSIZE: usize = 65536;
const CHUNKSIZE_U64: u64 = CHUNKSIZE as u64;

/// Calculates the hashes for a block
fn hash_block(mut file: &File) -> Result<Wrapping<u64>, io::Error> {
    let mut buf = [0u8; CHUNKSIZE];

    file.read_exact(&mut buf)?;

    let buf_u64: [u64; CHUNKSIZE / 8] = unsafe { mem::transmute(buf) };

    let hash = buf_u64
        .iter()
        .fold(Wrapping(0), |sum, &i| sum + Wrapping(i));

    Ok(hash)
}

// Calculates the file hash using the algo described at
// http://trac.opensubtitles.org/projects/opensubtitles/wiki/HashSourceCodes
pub fn size_and_hash(path: &OsStr) -> Result<(u64, u64), io::Error> {
    let mut file = File::open(path)?;
    let c1 = hash_block(&file)?;
    let fsize = file.seek(SeekFrom::End(0))?;
    let seekto = if fsize > CHUNKSIZE_U64 {
        fsize - CHUNKSIZE_U64
    } else {
        0
    };
    file.seek(SeekFrom::Start(seekto))?;
    let c2 = hash_block(&file)?;

    Ok((fsize, (Wrapping(fsize) + c1 + c2).0))
}
