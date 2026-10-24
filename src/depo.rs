use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Not {
    pub id: u64,
    pub metin: String,
    pub kirli: bool,
    pub surum: u64,
}

impl Not {
    pub fn baslik(&self) -> String {
        let satir = self.metin.lines().map(str::trim).find(|s| !s.is_empty());
        match satir {
            Some(s) if s.chars().count() > 48 => s.chars().take(48).chain(['…']).collect(),
            Some(s) => s.to_owned(),
            None => "Adsız not".to_owned(),
        }
    }
}

/// Her not klasörde ayrı bir .txt dosyası. Silinen not `cop` klasörüne taşınır.
pub struct Depo {
    klasor: PathBuf,
}

impl Depo {
    pub fn ac() -> io::Result<Self> {
        let kok = dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "veri klasörü bulunamadı"))?;
        let klasor = kok.join("Hien").join("notlar");
        fs::create_dir_all(klasor.join("cop"))?;
        Ok(Self { klasor })
    }

    pub fn klasor(&self) -> &Path {
        &self.klasor
    }

    fn yol(&self, id: u64) -> PathBuf {
        self.klasor.join(format!("{id}.txt"))
    }

    fn cop_yolu(&self, id: u64) -> PathBuf {
        self.klasor.join("cop").join(format!("{id}.txt"))
    }

    /// Notları son değiştirilen en üstte olacak şekilde okur.
    pub fn yukle(&self) -> io::Result<Vec<Not>> {
        let mut notlar = Vec::new();
        for giris in fs::read_dir(&self.klasor)? {
            let yol = giris?.path();
            if yol.extension().is_none_or(|e| e != "txt") {
                continue;
            }
            let Some(id) = yol
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse().ok())
            else {
                continue;
            };
            let degisti = fs::metadata(&yol)?.modified()?;
            let metin = fs::read_to_string(&yol)?;
            notlar.push((
                degisti,
                Not {
                    id,
                    metin,
                    kirli: false,
                    surum: 0,
                },
            ));
        }
        notlar.sort_by_key(|n| std::cmp::Reverse(n.0));
        Ok(notlar.into_iter().map(|(_, n)| n).collect())
    }

    pub fn yeni_id(&self) -> u64 {
        let simdi = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        (simdi..)
            .find(|id| !self.yol(*id).exists() && !self.cop_yolu(*id).exists())
            .unwrap_or(simdi)
    }

    /// Önce geçici dosyaya yazar, sonra adını değiştirir. Yazma yarıda kesilirse eski not sağlam kalır.
    pub fn kaydet(&self, not: &Not) -> io::Result<()> {
        let gecici = self.klasor.join(format!("{}.tmp", not.id));
        fs::write(&gecici, &not.metin)?;
        fs::rename(&gecici, self.yol(not.id))
    }

    pub fn cope_at(&self, id: u64) -> io::Result<()> {
        let yol = self.yol(id);
        if yol.exists() {
            fs::rename(yol, self.cop_yolu(id))
        } else {
            Ok(())
        }
    }

    pub fn copten_al(&self, id: u64) -> io::Result<()> {
        let cop = self.cop_yolu(id);
        if cop.exists() {
            fs::rename(cop, self.yol(id))
        } else {
            Ok(())
        }
    }
}
