#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod depo;
mod metin;
mod tema;

use std::ops::Range;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Key, KeyboardShortcut, Layout, Margin,
    Modifiers, Sense, Stroke, TextEdit, ThemePreference, pos2, text::CCursor, text::CCursorRange,
    text::LayoutJob, text::TextFormat, vec2,
};

use depo::{Depo, Not};
use metin::{Konum, Secenek};
use tema::Pal;

const KAYIT_GECIKMESI: Duration = Duration::from_millis(400);
const GERI_SINIRI: usize = 50;

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum TemaSecimi {
    Sistem,
    Koyu,
    Acik,
}

impl TemaSecimi {
    fn sonraki(self) -> Self {
        match self {
            Self::Sistem => Self::Koyu,
            Self::Koyu => Self::Acik,
            Self::Acik => Self::Sistem,
        }
    }

    fn ad(self) -> &'static str {
        match self {
            Self::Sistem => "Tema: sistem",
            Self::Koyu => "Tema: koyu",
            Self::Acik => "Tema: açık",
        }
    }

    fn tercih(self) -> ThemePreference {
        match self {
            Self::Sistem => ThemePreference::System,
            Self::Koyu => ThemePreference::Dark,
            Self::Acik => ThemePreference::Light,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Ayarlar {
    tema: TemaSecimi,
    son_not: Option<u64>,
    secenek: Secenek,
    konum: Konum,
    bosluk: bool,
}

impl Default for Ayarlar {
    fn default() -> Self {
        Self {
            tema: TemaSecimi::Sistem,
            son_not: None,
            secenek: Secenek::default(),
            konum: Konum::On,
            bosluk: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kip {
    Bul,
    Degistir,
    Ekle,
}

struct AramaPaneli {
    acik: bool,
    kip: Kip,
    aranan: String,
    yeni: String,
    kelime: String,
    sira: usize,
    odaklan: bool,
}

#[derive(Default)]
struct Vurgu {
    metin: String,
    aranan: String,
    secenek: Secenek,
    eslesmeler: Vec<Range<usize>>,
}

impl Vurgu {
    fn guncelle(&mut self, metin: &str, aranan: &str, s: Secenek) -> &[Range<usize>] {
        if self.metin != metin || self.aranan != aranan || self.secenek != s {
            self.eslesmeler = metin::bul(metin, aranan, s);
            self.metin.clear();
            self.metin.push_str(metin);
            self.aranan.clear();
            self.aranan.push_str(aranan);
            self.secenek = s;
        }
        &self.eslesmeler
    }
}

struct Mesaj {
    metin: String,
    hata: bool,
}

struct Uygulama {
    depo: Option<Depo>,
    notlar: Vec<Not>,
    secili: usize,
    liste_ara: String,
    ayar: Ayarlar,
    panel: AramaPaneli,
    vurgu: Vurgu,
    geri: Vec<(u64, String)>,
    silinen: Option<Not>,
    mesaj: Option<Mesaj>,
    son_degisim: Instant,
    sayim: (u64, u64, metin::Sayim),
    imlec: (usize, usize),
    kaydir: bool,
    editor_odak: bool,
    liste_odak: bool,
}

impl Uygulama {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        tema::kur(&cc.egui_ctx);
        let ayar: Ayarlar = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        cc.egui_ctx.set_theme(ayar.tema.tercih());

        let mut mesaj = None;
        let (depo, mut notlar) = match Depo::ac() {
            Ok(d) => match d.yukle() {
                Ok(n) => (Some(d), n),
                Err(e) => {
                    mesaj = Some(Mesaj {
                        metin: format!("Notlar okunamadı: {e}"),
                        hata: true,
                    });
                    (None, Vec::new())
                }
            },
            Err(e) => {
                mesaj = Some(Mesaj {
                    metin: format!("Not klasörü açılamadı: {e}"),
                    hata: true,
                });
                (None, Vec::new())
            }
        };
        if notlar.is_empty() {
            let id = depo.as_ref().map_or(0, Depo::yeni_id);
            notlar.push(Not {
                id,
                metin: String::new(),
                kirli: false,
                surum: 0,
            });
        }
        let secili = ayar
            .son_not
            .and_then(|id| notlar.iter().position(|n| n.id == id))
            .unwrap_or(0);

        Self {
            depo,
            notlar,
            secili,
            liste_ara: String::new(),
            panel: AramaPaneli {
                acik: false,
                kip: Kip::Bul,
                aranan: String::new(),
                yeni: String::new(),
                kelime: String::new(),
                sira: 0,
                odaklan: false,
            },
            ayar,
            vurgu: Vurgu::default(),
            geri: Vec::new(),
            silinen: None,
            mesaj,
            son_degisim: Instant::now(),
            sayim: (u64::MAX, 0, metin::Sayim::default()),
            imlec: (1, 1),
            kaydir: false,
            editor_odak: true,
            liste_odak: false,
        }
    }

    fn not(&self) -> &Not {
        &self.notlar[self.secili]
    }

    fn editor_id(&self) -> Id {
        Id::unique(("editor", self.not().id))
    }

    fn degisti(&mut self) {
        let not = &mut self.notlar[self.secili];
        not.kirli = true;
        not.surum += 1;
        self.son_degisim = Instant::now();
    }

    fn kaydet(&mut self) {
        let Some(depo) = &self.depo else { return };
        for not in self.notlar.iter_mut().filter(|n| n.kirli) {
            match depo.kaydet(not) {
                Ok(()) => not.kirli = false,
                Err(e) => {
                    self.mesaj = Some(Mesaj {
                        metin: format!("Kaydedilemedi: {e}"),
                        hata: true,
                    });
                    return;
                }
            }
        }
    }

    // Boş bırakılan not listede yer kaplamasın diye geçişte atılır.
    fn sec(&mut self, sira: usize) {
        if sira == self.secili {
            return;
        }
        self.kaydet();
        let mut hedef = sira;
        if self.notlar.len() > 1 && self.not().metin.trim().is_empty() {
            let bos = self.notlar.remove(self.secili);
            if let Some(d) = &self.depo {
                let _ = d.cope_at(bos.id);
            }
            if hedef > self.secili {
                hedef -= 1;
            }
        }
        self.secili = hedef;
        self.panel.sira = 0;
        self.mesaj = None;
        self.editor_odak = true;
    }

    fn yeni_not(&mut self) {
        if self.not().metin.trim().is_empty() {
            self.editor_odak = true;
            return;
        }
        self.kaydet();
        let id = self.depo.as_ref().map_or(0, Depo::yeni_id);
        self.notlar.insert(
            0,
            Not {
                id,
                metin: String::new(),
                kirli: false,
                surum: 0,
            },
        );
        self.secili = 0;
        self.liste_ara.clear();
        self.mesaj = None;
        self.editor_odak = true;
    }

    fn sil(&mut self) {
        let not = self.notlar.remove(self.secili);
        if let Some(d) = &self.depo
            && let Err(e) = d.cope_at(not.id)
        {
            self.mesaj = Some(Mesaj {
                metin: format!("Silinemedi: {e}"),
                hata: true,
            });
            self.notlar.insert(self.secili, not);
            return;
        }
        if self.notlar.is_empty() {
            let id = self.depo.as_ref().map_or(0, Depo::yeni_id);
            self.notlar.push(Not {
                id,
                metin: String::new(),
                kirli: false,
                surum: 0,
            });
        }
        self.secili = self.secili.min(self.notlar.len() - 1);
        self.mesaj = Some(Mesaj {
            metin: "Not çöp klasörüne taşındı.".into(),
            hata: false,
        });
        self.silinen = Some(not);
    }

    fn geri_getir(&mut self) {
        let Some(not) = self.silinen.take() else {
            return;
        };
        if let Some(d) = &self.depo
            && let Err(e) = d.copten_al(not.id)
        {
            self.mesaj = Some(Mesaj {
                metin: format!("Geri getirilemedi: {e}"),
                hata: true,
            });
            return;
        }
        self.notlar.insert(0, not);
        self.secili = 0;
        self.mesaj = None;
    }

    fn eslesmeler(&self) -> Vec<Range<usize>> {
        metin::bul(&self.not().metin, &self.panel.aranan, self.ayar.secenek)
    }

    fn metni_degistir(&mut self, yeni: String, mesaj: String) {
        let id = self.not().id;
        let eski = std::mem::replace(&mut self.notlar[self.secili].metin, yeni);
        self.geri.push((id, eski));
        if self.geri.len() > GERI_SINIRI {
            self.geri.remove(0);
        }
        self.degisti();
        self.mesaj = Some(Mesaj {
            metin: mesaj,
            hata: false,
        });
    }

    fn geri_al(&mut self) {
        let id = self.not().id;
        if self.geri.last().is_some_and(|(g, _)| *g == id) {
            let (_, eski) = self.geri.pop().unwrap();
            self.notlar[self.secili].metin = eski;
            self.degisti();
            self.mesaj = Some(Mesaj {
                metin: "Son işlem geri alındı.".into(),
                hata: false,
            });
        }
    }

    fn git(&mut self, ctx: &egui::Context, adim: isize) {
        let m = self.eslesmeler();
        if m.is_empty() {
            return;
        }
        let n = m.len() as isize;
        self.panel.sira = ((self.panel.sira as isize + adim).rem_euclid(n)) as usize;
        self.eslesmeyi_sec(ctx, &m);
    }

    fn eslesmeyi_sec(&mut self, ctx: &egui::Context, m: &[Range<usize>]) {
        let Some(r) = m.get(self.panel.sira) else {
            return;
        };
        let metin = &self.not().metin;
        let bas = metin::harf_sirasi(metin, r.start);
        let son = bas + metin[r.clone()].chars().count();
        let id = self.editor_id();
        let mut durum = TextEdit::load_state(ctx, id).unwrap_or_default();
        durum.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(bas),
            CCursor::new(son),
        )));
        durum.store(ctx, id);
        self.kaydir = true;
    }

    fn degistir_tek(&mut self) {
        let m = self.eslesmeler();
        let Some(r) = m.get(self.panel.sira.min(m.len().saturating_sub(1))) else {
            return;
        };
        let yeni = metin::degistir(&self.not().metin, std::slice::from_ref(r), &self.panel.yeni);
        self.metni_degistir(yeni, format!("1 yer değiştirildi, {} kaldı.", m.len() - 1));
    }

    fn degistir_tumu(&mut self) {
        let m = self.eslesmeler();
        if m.is_empty() {
            return;
        }
        let yeni = metin::degistir(&self.not().metin, &m, &self.panel.yeni);
        self.metni_degistir(yeni, format!("{} yer değiştirildi.", m.len()));
    }

    fn ekle(&mut self, tumu: bool) {
        let kelime = self.panel.kelime.trim();
        if kelime.is_empty() {
            return;
        }
        let m = self.eslesmeler();
        let hedef: &[Range<usize>] = if tumu {
            &m
        } else {
            match m.get(self.panel.sira) {
                Some(r) => std::slice::from_ref(r),
                None => return,
            }
        };
        if hedef.is_empty() {
            return;
        }
        let (yeni, adet) = metin::ekle(
            &self.not().metin,
            hedef,
            kelime,
            self.ayar.konum,
            self.ayar.bosluk,
            self.ayar.secenek,
        );
        let atlanan = hedef.len() - adet;
        if adet == 0 {
            self.mesaj = Some(Mesaj {
                metin: "Kelime zaten orada, değişiklik yok.".into(),
                hata: false,
            });
            return;
        }
        let mut mesaj = format!("{adet} yere eklendi.");
        if atlanan > 0 {
            mesaj = format!("{adet} yere eklendi, {atlanan} yerde zaten vardı.");
        }
        self.metni_degistir(yeni, mesaj);
    }

    fn secili_metin(&self, ctx: &egui::Context) -> Option<String> {
        let r = TextEdit::load_state(ctx, self.editor_id())?
            .cursor
            .char_range()?;
        let (a, b): (usize, usize) = (
            r.primary.index.min(r.secondary.index).into(),
            r.primary.index.max(r.secondary.index).into(),
        );
        let s: String = self.not().metin.chars().skip(a).take(b - a).collect();
        (!s.is_empty() && !s.contains('\n')).then_some(s)
    }

    fn paneli_ac(&mut self, ctx: &egui::Context, kip: Kip) {
        if let Some(s) = self.secili_metin(ctx) {
            self.panel.aranan = s;
            self.panel.sira = 0;
        }
        self.panel.acik = true;
        self.panel.kip = kip;
        self.panel.odaklan = true;
    }

    fn kisayollar(&mut self, ctx: &egui::Context) {
        let k = |m, t| KeyboardShortcut::new(m, t);
        let ctrl = Modifiers::COMMAND;
        let bas = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));

        if bas(k(ctrl, Key::N)) {
            self.yeni_not();
        }
        if bas(k(ctrl, Key::S)) {
            self.kaydet();
        }
        if bas(k(ctrl, Key::F)) {
            self.paneli_ac(ctx, Kip::Bul);
        }
        if bas(k(ctrl, Key::H)) {
            self.paneli_ac(ctx, Kip::Degistir);
        }
        if bas(k(ctrl, Key::E)) {
            self.paneli_ac(ctx, Kip::Ekle);
        }
        if bas(k(ctrl, Key::K)) {
            self.liste_odak = true;
        }
        if bas(k(Modifiers::SHIFT, Key::F3)) {
            self.git(ctx, -1);
        }
        if bas(k(Modifiers::NONE, Key::F3)) {
            self.git(ctx, 1);
        }
        if bas(k(Modifiers::ALT, Key::ArrowDown)) && self.secili + 1 < self.notlar.len() {
            self.sec(self.secili + 1);
        }
        if bas(k(Modifiers::ALT, Key::ArrowUp)) && self.secili > 0 {
            self.sec(self.secili - 1);
        }
        if self.panel.acik && bas(k(Modifiers::NONE, Key::Escape)) {
            self.panel.acik = false;
            self.editor_odak = true;
        }
    }

    fn kenar(&mut self, ui: &mut egui::Ui, p: &Pal) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let yeni = ui.button("Yeni not").on_hover_text("Ctrl+N");
            if yeni.clicked() {
                self.yeni_not();
            }
        });
        ui.add_space(4.0);
        let ara = alan(
            ui,
            p,
            &mut self.liste_ara,
            "Notlarda ara  Ctrl+K",
            ui.available_width(),
        );
        if std::mem::take(&mut self.liste_odak) {
            ara.request_focus();
        }
        ui.add_space(6.0);

        let alt_yukseklik = 40.0;
        let mut tikla = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - alt_yukseklik)
            .show(ui, |ui| {
                let q = self.liste_ara.trim();
                let mut gosterilen = 0;
                for (i, not) in self.notlar.iter().enumerate() {
                    if !q.is_empty() && metin::bul(&not.metin, q, Secenek::default()).is_empty() {
                        continue;
                    }
                    gosterilen += 1;
                    if satir(ui, p, &not.baslik(), i == self.secili).clicked() {
                        tikla = Some(i);
                    }
                }
                if gosterilen == 0 {
                    ui.add_space(8.0);
                    ui.colored_label(p.weak, "Eşleşen not yok.");
                }
            });
        if let Some(i) = tikla {
            self.sec(i);
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(self.ayar.tema.ad()).clicked() {
                    self.ayar.tema = self.ayar.tema.sonraki();
                    ui.ctx().set_theme(self.ayar.tema.tercih());
                }
                if let Some(d) = &self.depo
                    && ui
                        .button("Klasörü aç")
                        .on_hover_text(d.klasor().display().to_string())
                        .clicked()
                    && let Err(e) = std::process::Command::new("explorer")
                        .arg(d.klasor())
                        .spawn()
                {
                    self.mesaj = Some(Mesaj {
                        metin: format!("Klasör açılamadı: {e}"),
                        hata: true,
                    });
                }
            });
        });
    }

    fn baslik_cubugu(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button("Sil")
                    .on_hover_text("Not çöp klasörüne taşınır")
                    .clicked()
                {
                    self.sil();
                }
                let ctx = ui.ctx().clone();
                for (ad, kisa, kip) in [
                    ("Kelime ekle", "Ctrl+E", Kip::Ekle),
                    ("Değiştir", "Ctrl+H", Kip::Degistir),
                    ("Bul", "Ctrl+F", Kip::Bul),
                ] {
                    let secili = self.panel.acik && self.panel.kip == kip;
                    if ui
                        .selectable_label(secili, ad)
                        .on_hover_text(kisa)
                        .clicked()
                    {
                        if secili {
                            self.panel.acik = false;
                        } else {
                            self.paneli_ac(&ctx, kip);
                        }
                    }
                }
            });
        });
    }

    fn arama_paneli(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let ctx = ui.ctx().clone();
        let adet = self
            .vurgu
            .guncelle(
                &self.notlar[self.secili].metin,
                &self.panel.aranan,
                self.ayar.secenek,
            )
            .len();
        if self.panel.sira >= adet {
            self.panel.sira = 0;
        }

        Frame::new()
            .fill(p.field.gamma_multiply(if ctx.theme() == egui::Theme::Dark {
                0.75
            } else {
                1.0
            }))
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ara");
                    let genislik = (ui.available_width() - 330.0).max(140.0);
                    let r = alan(ui, p, &mut self.panel.aranan, "Aranacak kelime", genislik);
                    if std::mem::take(&mut self.panel.odaklan) {
                        r.request_focus();
                        select_all(&ctx, r.id, &self.panel.aranan);
                    }
                    if r.changed() {
                        self.panel.sira = 0;
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let geri = ui.input(|i| i.modifiers.shift);
                        self.git(&ctx, if geri { -1 } else { 1 });
                        r.request_focus();
                    }

                    let sayac = match (self.panel.aranan.is_empty(), adet) {
                        (true, _) => String::new(),
                        (false, 0) => "Sonuç yok".into(),
                        (false, n) => format!("{}/{}", self.panel.sira + 1, n),
                    };
                    let renk = if adet == 0 && !self.panel.aranan.is_empty() {
                        p.error_text
                    } else {
                        p.weak
                    };
                    ui.add_sized(
                        [64.0, 24.0],
                        egui::Label::new(egui::RichText::new(sayac).color(renk)),
                    );

                    if ui
                        .add_enabled(adet > 0, egui::Button::new("↑"))
                        .on_hover_text("Önceki  Shift+F3")
                        .clicked()
                    {
                        self.git(&ctx, -1);
                    }
                    if ui
                        .add_enabled(adet > 0, egui::Button::new("↓"))
                        .on_hover_text("Sonraki  F3")
                        .clicked()
                    {
                        self.git(&ctx, 1);
                    }
                    ui.toggle_value(&mut self.ayar.secenek.buyuk_kucuk, "Aa")
                        .on_hover_text("Büyük küçük harfe duyarlı");
                    ui.toggle_value(&mut self.ayar.secenek.tam_kelime, "Tam kelime")
                        .on_hover_text("Yalnız kelimenin tamamı eşleşir");
                    if ui.button("✕").on_hover_text("Kapat  Esc").clicked() {
                        self.panel.acik = false;
                    }
                });

                match self.panel.kip {
                    Kip::Bul => {}
                    Kip::Degistir => {
                        ui.horizontal(|ui| {
                            ui.label("Yeni");
                            let genislik = (ui.available_width() - 330.0).max(140.0);
                            let r = alan(ui, p, &mut self.panel.yeni, "Yerine yazılacak", genislik);
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.degistir_tek();
                                r.request_focus();
                            }
                            if ui
                                .add_enabled(adet > 0, egui::Button::new("Değiştir"))
                                .clicked()
                            {
                                self.degistir_tek();
                            }
                            if ui
                                .add_enabled(adet > 0, birincil(p, "Tümünü değiştir"))
                                .clicked()
                            {
                                self.degistir_tumu();
                            }
                            self.geri_dugmesi(ui);
                        });
                    }
                    Kip::Ekle => {
                        ui.horizontal(|ui| {
                            ui.label("Kelime");
                            let genislik = (ui.available_width() - 420.0).max(120.0);
                            let r =
                                alan(ui, p, &mut self.panel.kelime, "Eklenecek kelime", genislik);
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.ekle(true);
                                r.request_focus();
                            }
                            ui.selectable_value(&mut self.ayar.konum, Konum::On, "Önüne");
                            ui.selectable_value(&mut self.ayar.konum, Konum::Arka, "Arkasına");
                            ui.toggle_value(&mut self.ayar.bosluk, "Boşluk")
                                .on_hover_text("Kelimeyle arasına boşluk koy");
                            let hazir = adet > 0 && !self.panel.kelime.trim().is_empty();
                            if ui
                                .add_enabled(hazir, egui::Button::new("Buna ekle"))
                                .clicked()
                            {
                                self.ekle(false);
                            }
                            if ui.add_enabled(hazir, birincil(p, "Tümüne ekle")).clicked() {
                                self.ekle(true);
                            }
                            self.geri_dugmesi(ui);
                        });
                        if !self.panel.kelime.trim().is_empty() && !self.panel.aranan.is_empty() {
                            let ara = if self.ayar.bosluk { " " } else { "" };
                            let ornek = match self.ayar.konum {
                                Konum::On => format!(
                                    "{}{ara}{}",
                                    self.panel.kelime.trim(),
                                    self.panel.aranan
                                ),
                                Konum::Arka => format!(
                                    "{}{ara}{}",
                                    self.panel.aranan,
                                    self.panel.kelime.trim()
                                ),
                            };
                            ui.colored_label(p.weak, format!("Sonuç: {ornek}"));
                        }
                    }
                }
            });
    }

    fn geri_dugmesi(&mut self, ui: &mut egui::Ui) {
        let var = self.geri.last().is_some_and(|(id, _)| *id == self.not().id);
        if ui
            .add_enabled(var, egui::Button::new("Geri al"))
            .on_hover_text("Son toplu işlemi geri alır")
            .clicked()
        {
            self.geri_al();
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let id = self.editor_id();
        let vurgula = self.panel.acik && !self.panel.aranan.is_empty();
        let aranan = self.panel.aranan.clone();
        let secenek = self.ayar.secenek;
        let sira = self.panel.sira;
        let vurgu = &mut self.vurgu;
        let font = FontId::proportional(15.0);

        let mut dizici = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, genislik: f32| {
            let metin = buf.as_str();
            let duz = TextFormat {
                font_id: font.clone(),
                color: p.text,
                ..Default::default()
            };
            let mut is = LayoutJob::default();
            let mut son = 0;
            if vurgula {
                for (i, r) in vurgu.guncelle(metin, &aranan, secenek).iter().enumerate() {
                    is.append(&metin[son..r.start], 0.0, duz.clone());
                    let (zemin, renk) = if i == sira {
                        (p.accent_text, p.on_accent)
                    } else {
                        (p.eslesme, p.text)
                    };
                    is.append(
                        &metin[r.clone()],
                        0.0,
                        TextFormat {
                            background: zemin,
                            color: renk,
                            ..duz.clone()
                        },
                    );
                    son = r.end;
                }
            }
            is.append(&metin[son..], 0.0, duz);
            is.wrap.max_width = genislik;
            ui.fonts_mut(|f| f.layout_job(is))
        };

        let not = &mut self.notlar[self.secili];
        let cikti = egui::ScrollArea::vertical()
            .id_salt(("kaydir", not.id))
            .auto_shrink(false)
            .show(ui, |ui| {
                let cikti = TextEdit::multiline(&mut not.metin)
                    .id(id)
                    .frame(Frame::NONE)
                    .hint_text("Yazmaya başla. İlk satır notun adı olur.")
                    .desired_width(f32::INFINITY)
                    .min_size(ui.available_size())
                    .lock_focus(true)
                    .layouter(&mut dizici)
                    .show(ui);
                if std::mem::take(&mut self.kaydir)
                    && let Some(r) = cikti.cursor_range
                {
                    let kutu = cikti
                        .galley
                        .pos_from_cursor(r.primary)
                        .translate(cikti.galley_pos.to_vec2());
                    ui.scroll_to_rect(kutu.expand(24.0), Some(Align::Center));
                }
                cikti
            })
            .inner;

        if std::mem::take(&mut self.editor_odak) {
            cikti.response.request_focus();
        }
        if cikti.response.changed() {
            self.degisti();
        }
        if let Some(r) = cikti.cursor_range {
            self.imlec =
                metin::satir_sutun(&self.notlar[self.secili].metin, r.primary.index.into());
        }
    }

    fn durum_cubugu(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let not = &self.notlar[self.secili];
        if (self.sayim.0, self.sayim.1) != (not.id, not.surum) {
            self.sayim = (not.id, not.surum, metin::say(&not.metin));
        }
        let kirli = self.notlar.iter().any(|n| n.kirli);
        let s = self.sayim.2;

        ui.horizontal_centered(|ui| {
            let mut geri_getir = false;
            match &self.mesaj {
                Some(m) => {
                    ui.colored_label(if m.hata { p.error_text } else { p.weak }, &m.metin);
                    if !m.hata && self.silinen.is_some() && ui.link("Geri getir").clicked() {
                        geri_getir = true;
                    }
                }
                None if self.depo.is_none() => {
                    ui.colored_label(p.error_text, "Kayıt kapalı");
                }
                None => {
                    ui.colored_label(
                        p.weak,
                        if kirli {
                            "Kaydediliyor…"
                        } else {
                            "Kaydedildi"
                        },
                    );
                }
            }
            if geri_getir {
                self.geri_getir();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.colored_label(
                    p.weak,
                    format!("Satır {}, Sütun {}", self.imlec.0, self.imlec.1),
                );
                ui.colored_label(p.weak, format!("{} karakter", s.harf));
                ui.colored_label(p.weak, format!("{} kelime", s.kelime));
                ui.colored_label(p.weak, format!("{} satır", s.satir));
            });
        });
    }
}

impl eframe::App for Uygulama {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.kisayollar(ctx);
        if self.notlar.iter().any(|n| n.kirli) {
            let gecen = self.son_degisim.elapsed();
            if gecen >= KAYIT_GECIKMESI {
                self.kaydet();
            } else {
                ctx.request_repaint_after(KAYIT_GECIKMESI - gecen);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let p = *tema::pal(ui.ctx());

        egui::Panel::bottom("durum")
            .exact_size(23.0)
            .frame(
                Frame::new()
                    .fill(p.status)
                    .inner_margin(Margin::symmetric(12, 0))
                    .stroke(Stroke::new(1.0, p.divider)),
            )
            .show(ui, |ui| self.durum_cubugu(ui, &p));

        egui::Panel::left("kenar")
            .resizable(true)
            .default_size(240.0)
            .min_size(180.0)
            .frame(
                Frame::new()
                    .fill(p.content)
                    .inner_margin(Margin::symmetric(8, 0))
                    .stroke(Stroke::new(1.0, p.line)),
            )
            .show(ui, |ui| self.kenar(ui, &p));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.content).inner_margin(Margin {
                left: 24,
                right: 16,
                top: 4,
                bottom: 8,
            }))
            .show(ui, |ui| {
                self.baslik_cubugu(ui);
                if self.panel.acik {
                    self.arama_paneli(ui, &p);
                    ui.add_space(8.0);
                }
                self.editor(ui, &p);
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.ayar.son_not = Some(self.not().id);
        eframe::set_value(storage, eframe::APP_KEY, &self.ayar);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.kaydet();
    }
}

fn birincil<'a>(p: &Pal, ad: &'a str) -> egui::Button<'a> {
    egui::Button::new(egui::RichText::new(ad).color(Color32::WHITE))
        .fill(p.accent)
        .stroke(Stroke::NONE)
}

fn select_all(ctx: &egui::Context, id: Id, metin: &str) {
    if let Some(mut d) = TextEdit::load_state(ctx, id) {
        let n = metin.chars().count();
        d.cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(n))));
        d.store(ctx, id);
    }
}

/// Tek satırlık giriş kutusu: alanın altında ince çizgi, odakta kalın vurgu çizgisi.
fn alan(
    ui: &mut egui::Ui,
    p: &Pal,
    metin: &mut String,
    ipucu: &str,
    genislik: f32,
) -> egui::Response {
    let cikti = TextEdit::singleline(metin)
        .hint_text(egui::RichText::new(ipucu).color(p.weak))
        .desired_width(genislik - 16.0)
        .frame(
            Frame::new()
                .fill(p.field)
                .corner_radius(4)
                .inner_margin(Margin::symmetric(8, 4))
                .stroke(if ui.ctx().theme() == egui::Theme::Light {
                    Stroke::new(1.0, p.line)
                } else {
                    Stroke::NONE
                }),
        )
        .show(ui);
    let r = cikti.response.response;
    let odak = r.has_focus();
    let (kalin, renk) = if odak {
        (2.0, p.accent_text)
    } else {
        (1.0, p.control_stroke)
    };
    let x = r.rect.x_range().shrink(2.0);
    ui.painter()
        .hline(x, r.rect.bottom() - kalin / 2.0, Stroke::new(kalin, renk));
    r
}

fn satir(ui: &mut egui::Ui, p: &Pal, baslik: &str, secili: bool) -> egui::Response {
    let (r, yanit) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let boya = ui.painter_at(r);
    let zemin = if secili {
        Some(p.nav_sel)
    } else if yanit.hovered() {
        Some(p.hover)
    } else {
        None
    };
    if let Some(z) = zemin {
        boya.rect_filled(r, 5, z);
    }
    if secili {
        let serit = egui::Rect::from_min_size(pos2(r.left(), r.center().y - 8.0), vec2(3.0, 16.0));
        boya.rect_filled(serit, 1.5, p.accent_text);
    }
    boya.text(
        pos2(r.left() + 12.0, r.center().y),
        Align2::LEFT_CENTER,
        baslik,
        FontId::proportional(14.0),
        if secili { p.text } else { p.weak },
    );
    yanit
}

fn main() -> eframe::Result {
    let secenek = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Hien")
            .with_app_id("Hien")
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([620.0, 380.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Hien",
        secenek,
        Box::new(|cc| Ok(Box::new(Uygulama::new(cc)))),
    )
}
