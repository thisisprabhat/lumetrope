//! Plain zip archives for workspace backups (the web host builds the entry
//! list; this only packs and unpacks bytes).

use std::io::{Cursor, Read, Write};

/// Pack `(name, bytes)` entries into a deflate-compressed zip.
pub fn pack(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(false);
    for (name, data) in entries {
        w.start_file(name.as_str(), o).map_err(|e| e.to_string())?;
        w.write_all(data).map_err(|e| e.to_string())?;
    }
    w.finish().map(Cursor::into_inner).map_err(|e| e.to_string())
}

/// Every file entry of a zip (directories skipped), in archive order.
pub fn unpack(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a zip file: {e}"))?;
    let mut out = Vec::with_capacity(z.len());
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let mut data = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut data).map_err(|e| format!("{}: {e}", f.name()))?;
        out.push((f.name().to_string(), data));
    }
    Ok(out)
}
