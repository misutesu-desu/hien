use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Note {
    pub id: u64,
    pub text: String,
    pub dirty: bool,
    pub version: u64,
    /// Set when the note is a file opened from outside the notes folder.
    pub file: Option<File>,
}

/// How an outside file was stored on disk, so saving writes it back the same way.
pub struct File {
    pub path: PathBuf,
    pub bom: bool,
    pub crlf: bool,
}

const BOM: &str = "\u{feff}";

fn decode(bytes: Vec<u8>) -> io::Result<(String, bool, bool)> {
    let text = String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not a UTF-8 text file"))?;
    let bom = text.starts_with(BOM);
    let text = if bom { &text[BOM.len()..] } else { &text[..] };
    // egui draws a stray glyph for \r, so lines are kept as \n while editing.
    let crlf = text.contains("\r\n");
    Ok((text.replace("\r\n", "\n"), bom, crlf))
}

fn encode(text: &str, bom: bool, crlf: bool) -> String {
    let body = if crlf { text.replace('\n', "\r\n") } else { text.to_owned() };
    if bom { format!("{BOM}{body}") } else { body }
}

/// Writes to a temp file first, then renames it. A write cut short leaves the old file intact.
fn write_atomic(path: &Path, tmp: &Path, contents: &str) -> io::Result<()> {
    fs::write(tmp, contents)?;
    fs::rename(tmp, path)
}

pub fn open_file(path: &Path, id: u64) -> io::Result<Note> {
    let (text, bom, crlf) = decode(fs::read(path)?)?;
    let file = File { path: path.to_owned(), bom, crlf };
    Ok(Note { text, file: Some(file), ..Note::new(id) })
}

pub fn save_file(note: &Note, file: &File) -> io::Result<()> {
    let tmp = file.path.with_extension("hien-tmp");
    write_atomic(&file.path, &tmp, &encode(&note.text, file.bom, file.crlf))
}

impl Note {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            text: String::new(),
            dirty: false,
            version: 0,
            file: None,
        }
    }

    /// File name for an outside file, otherwise the first non-empty line cut to 48 chars.
    /// `None` for an empty note.
    pub fn title(&self) -> Option<String> {
        if let Some(f) = &self.file {
            return f.path.file_name().map(|n| n.to_string_lossy().into_owned());
        }
        let line = self.text.lines().map(str::trim).find(|s| !s.is_empty())?;
        Some(if line.chars().count() > 48 {
            line.chars().take(48).chain(['…']).collect()
        } else {
            line.to_owned()
        })
    }
}

/// Every note is its own .txt file in the folder. Deleted notes move to `trash`.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn open() -> io::Result<Self> {
        let root = dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no data directory"))?
            .join("Hien");
        let dir = root.join("notes");
        // Earlier builds used Turkish folder names. Move them once so existing notes stay.
        let old = root.join("notlar");
        if old.exists() && !dir.exists() {
            fs::rename(&old, &dir)?;
            if dir.join("cop").exists() {
                fs::rename(dir.join("cop"), dir.join("trash"))?;
            }
        }
        fs::create_dir_all(dir.join("trash"))?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.txt"))
    }

    fn trash_path(&self, id: u64) -> PathBuf {
        self.dir.join("trash").join(format!("{id}.txt"))
    }

    /// Reads all notes, most recently modified first.
    pub fn load(&self) -> io::Result<Vec<Note>> {
        let mut notes = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().is_none_or(|e| e != "txt") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse().ok())
            else {
                continue;
            };
            let modified = fs::metadata(&path)?.modified()?;
            let text = fs::read_to_string(&path)?;
            notes.push((
                modified,
                Note {
                    text,
                    ..Note::new(id)
                },
            ));
        }
        notes.sort_by_key(|n| std::cmp::Reverse(n.0));
        Ok(notes.into_iter().map(|(_, n)| n).collect())
    }

    pub fn new_id(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        (now..)
            .find(|id| !self.path(*id).exists() && !self.trash_path(*id).exists())
            .unwrap_or(now)
    }

    pub fn save(&self, note: &Note) -> io::Result<()> {
        let tmp = self.dir.join(format!("{}.tmp", note.id));
        write_atomic(&self.path(note.id), &tmp, &note.text)
    }

    pub fn trash(&self, id: u64) -> io::Result<()> {
        let path = self.path(id);
        if path.exists() {
            fs::rename(path, self.trash_path(id))
        } else {
            Ok(())
        }
    }

    pub fn restore(&self, id: u64) -> io::Result<()> {
        let path = self.trash_path(id);
        if path.exists() {
            fs::rename(path, self.path(id))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_and_bom_survive_a_round_trip() {
        let raw = "\u{feff}bir\r\niki\r\n";
        let (text, bom, crlf) = decode(raw.as_bytes().to_vec()).unwrap();
        assert_eq!(text, "bir\niki\n");
        assert!(bom && crlf);
        assert_eq!(encode(&text, bom, crlf), raw);
    }

    #[test]
    fn plain_lf_file_stays_lf() {
        let (text, bom, crlf) = decode(b"a\nb".to_vec()).unwrap();
        assert!(!bom && !crlf);
        assert_eq!(encode(&text, bom, crlf), "a\nb");
    }

    #[test]
    fn rejects_non_utf8() {
        assert!(decode(vec![0x66, 0xFE, 0x67]).is_err());
    }
}
