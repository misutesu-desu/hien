use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Note {
    pub id: u64,
    pub text: String,
    pub dirty: bool,
    pub version: u64,
}

impl Note {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            text: String::new(),
            dirty: false,
            version: 0,
        }
    }

    /// First non-empty line, cut to 48 chars. `None` for an empty note.
    pub fn title(&self) -> Option<String> {
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

    /// Writes to a temp file first, then renames it. A write cut short leaves the old note intact.
    pub fn save(&self, note: &Note) -> io::Result<()> {
        let tmp = self.dir.join(format!("{}.tmp", note.id));
        fs::write(&tmp, &note.text)?;
        fs::rename(&tmp, self.path(note.id))
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
