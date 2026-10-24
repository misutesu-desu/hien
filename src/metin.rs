use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Secenek {
    pub buyuk_kucuk: bool,
    pub tam_kelime: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Konum {
    On,
    Arka,
}

// Türkçede I/ı ve İ/i çiftleri standart küçültmeyle eşleşmez. Aramada
// dördünü aynı harf saymak, yanlış eşleşmeden daha az şaşırtıyor.
fn katla(c: char) -> char {
    match c {
        'I' | 'İ' | 'ı' => 'i',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

fn ayni(a: char, b: char, s: Secenek) -> bool {
    if s.buyuk_kucuk {
        a == b
    } else {
        katla(a) == katla(b)
    }
}

// Kesme işareti kelimeye dahil değil, böylece "Ali" araması "Ali'nin" içinde de bulunur.
fn kelime_harfi(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn bul(metin: &str, aranan: &str, s: Secenek) -> Vec<Range<usize>> {
    let aranan: Vec<char> = aranan.chars().collect();
    if aranan.is_empty() {
        return Vec::new();
    }
    let harfler: Vec<(usize, char)> = metin.char_indices().collect();
    let n = aranan.len();
    let mut sonuc = Vec::new();
    let mut i = 0;
    while i + n <= harfler.len() {
        let uyuyor = (0..n).all(|j| ayni(harfler[i + j].1, aranan[j], s));
        let sinirda = !s.tam_kelime
            || ((i == 0 || !kelime_harfi(harfler[i - 1].1))
                && harfler.get(i + n).is_none_or(|&(_, c)| !kelime_harfi(c)));
        if uyuyor && sinirda {
            let bas = harfler[i].0;
            let son = harfler.get(i + n).map_or(metin.len(), |&(b, _)| b);
            sonuc.push(bas..son);
            i += n;
        } else {
            i += 1;
        }
    }
    sonuc
}

pub fn degistir(metin: &str, eslesmeler: &[Range<usize>], yeni: &str) -> String {
    let mut cikti = String::with_capacity(metin.len());
    let mut son = 0;
    for m in eslesmeler {
        cikti.push_str(&metin[son..m.start]);
        cikti.push_str(yeni);
        son = m.end;
    }
    cikti.push_str(&metin[son..]);
    cikti
}

fn bitiyor(metin: &str, ek: &str, s: Secenek) -> bool {
    let mut a = metin.chars().rev();
    ek.chars()
        .rev()
        .all(|e| a.next().is_some_and(|c| ayni(c, e, s)))
}

fn basliyor(metin: &str, ek: &str, s: Secenek) -> bool {
    let mut a = metin.chars();
    ek.chars().all(|e| a.next().is_some_and(|c| ayni(c, e, s)))
}

/// Eşleşmelerin önüne ya da arkasına `kelime` ekler. Kelime zaten oradaysa
/// o eşleşmeyi atlar, böylece aynı işlem iki kez uygulanınca metin bozulmaz.
/// Dönen sayı eklenen yer sayısıdır.
pub fn ekle(
    metin: &str,
    eslesmeler: &[Range<usize>],
    kelime: &str,
    konum: Konum,
    bosluk: bool,
    s: Secenek,
) -> (String, usize) {
    let ara = if bosluk { " " } else { "" };
    let parca = match konum {
        Konum::On => format!("{kelime}{ara}"),
        Konum::Arka => format!("{ara}{kelime}"),
    };
    let mut cikti = String::with_capacity(metin.len() + eslesmeler.len() * parca.len());
    let mut son = 0;
    let mut adet = 0;
    for m in eslesmeler {
        match konum {
            Konum::On => {
                cikti.push_str(&metin[son..m.start]);
                if !bitiyor(&metin[..m.start], &parca, s) {
                    cikti.push_str(&parca);
                    adet += 1;
                }
                cikti.push_str(&metin[m.clone()]);
            }
            Konum::Arka => {
                cikti.push_str(&metin[son..m.end]);
                if !basliyor(&metin[m.end..], &parca, s) {
                    cikti.push_str(&parca);
                    adet += 1;
                }
            }
        }
        son = m.end;
    }
    cikti.push_str(&metin[son..]);
    (cikti, adet)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sayim {
    pub kelime: usize,
    pub harf: usize,
    pub satir: usize,
}

pub fn say(metin: &str) -> Sayim {
    Sayim {
        kelime: metin.split_whitespace().count(),
        harf: metin.chars().count(),
        satir: metin.lines().count().max(1),
    }
}

/// Karakter sırasına göre satır ve sütun, ikisi de 1'den başlar.
pub fn satir_sutun(metin: &str, harf_sirasi: usize) -> (usize, usize) {
    let mut satir = 1;
    let mut sutun = 1;
    for c in metin.chars().take(harf_sirasi) {
        if c == '\n' {
            satir += 1;
            sutun = 1;
        } else {
            sutun += 1;
        }
    }
    (satir, sutun)
}

pub fn harf_sirasi(metin: &str, bayt: usize) -> usize {
    metin[..bayt].chars().count()
}

#[cfg(test)]
mod testler {
    use super::*;

    const DUZ: Secenek = Secenek {
        buyuk_kucuk: false,
        tam_kelime: false,
    };
    const TAM: Secenek = Secenek {
        buyuk_kucuk: false,
        tam_kelime: true,
    };

    fn parcalar<'a>(metin: &'a str, m: &[Range<usize>]) -> Vec<&'a str> {
        m.iter().map(|r| &metin[r.clone()]).collect()
    }

    #[test]
    fn turkce_buyuk_kucuk_esitlenir() {
        let t = "İstanbul ve istanbul, ISPARTA ıspanak";
        assert_eq!(
            parcalar(t, &bul(t, "istanbul", DUZ)),
            ["İstanbul", "istanbul"]
        );
        assert_eq!(bul(t, "ıs", DUZ).len(), 4);
    }

    #[test]
    fn buyuk_kucuk_duyarli() {
        let s = Secenek {
            buyuk_kucuk: true,
            tam_kelime: false,
        };
        assert_eq!(bul("Kedi kedi KEDİ", "kedi", s), [5..9]);
    }

    #[test]
    fn tam_kelime_kesme_isaretinde_biter() {
        let t = "Ali'nin Alişan ali";
        assert_eq!(parcalar(t, &bul(t, "ali", TAM)), ["Ali", "ali"]);
    }

    #[test]
    fn cakisan_eslesme_yok() {
        assert_eq!(bul("aaaa", "aa", DUZ), [0..2, 2..4]);
    }

    #[test]
    fn bos_arama_bos_doner() {
        assert!(bul("metin", "", DUZ).is_empty());
    }

    #[test]
    fn tumunu_degistir() {
        let t = "çay ve çay";
        assert_eq!(degistir(t, &bul(t, "çay", DUZ), "kahve"), "kahve ve kahve");
    }

    #[test]
    fn one_ekle() {
        let t = "kedi uyudu, kedi uyandı";
        let (yeni, adet) = ekle(t, &bul(t, "kedi", TAM), "tekir", Konum::On, true, TAM);
        assert_eq!(yeni, "tekir kedi uyudu, tekir kedi uyandı");
        assert_eq!(adet, 2);
    }

    #[test]
    fn ikinci_kez_eklemez() {
        let t = "tekir kedi ve kedi";
        let (yeni, adet) = ekle(t, &bul(t, "kedi", TAM), "Tekir", Konum::On, true, TAM);
        assert_eq!(yeni, "tekir kedi ve Tekir kedi");
        assert_eq!(adet, 1);
    }

    #[test]
    fn arkaya_bosluksuz_ekle() {
        let t = "not1 not2";
        let (yeni, _) = ekle(t, &bul(t, "not", DUZ), "#", Konum::Arka, false, DUZ);
        assert_eq!(yeni, "not#1 not#2");
    }

    #[test]
    fn sayim_ve_konum() {
        let t = "bir iki\nüç";
        assert_eq!(
            say(t),
            Sayim {
                kelime: 3,
                harf: 10,
                satir: 2
            }
        );
        assert_eq!(satir_sutun(t, 9), (2, 2));
        assert_eq!(harf_sirasi(t, t.len()), 10);
    }
}
