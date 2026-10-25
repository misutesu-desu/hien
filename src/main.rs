#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod i18n;
mod instance;
mod store;
mod text;
mod theme;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Key, KeyboardShortcut, Layout, Margin,
    Modifiers, Sense, Stroke, TextEdit, ThemePreference, pos2, text::CCursor, text::CCursorRange,
    text::LayoutJob, text::TextFormat, vec2,
};

use i18n::{Lang, Strings};
use store::{Note, Store};
use text::{MatchOptions, Side};
use theme::Pal;

const SAVE_DELAY: Duration = Duration::from_millis(400);
const UNDO_LIMIT: usize = 50;
/// Pointer distance from the anchor that still counts as standing still.
const AUTOSCROLL_DEAD_ZONE: f32 = 12.0;
/// Scroll speed in points per second for each point the pointer is past the dead zone.
const AUTOSCROLL_SPEED: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum ThemeChoice {
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::System => t.theme_system,
            Self::Dark => t.theme_dark,
            Self::Light => t.theme_light,
        }
    }

    fn preference(self) -> ThemePreference {
        match self {
            Self::System => ThemePreference::System,
            Self::Dark => ThemePreference::Dark,
            Self::Light => ThemePreference::Light,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    theme: ThemeChoice,
    lang: Lang,
    last_note: Option<u64>,
    options: MatchOptions,
    side: Side,
    space: bool,
    background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            lang: Lang::detect(),
            last_note: None,
            options: MatchOptions::default(),
            side: Side::Before,
            space: true,
            background: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Find,
    Replace,
    Insert,
}

struct FindPanel {
    open: bool,
    mode: Mode,
    needle: String,
    replacement: String,
    word: String,
    current: usize,
    focus: bool,
}

#[derive(Default)]
struct MatchCache {
    text: String,
    needle: String,
    options: MatchOptions,
    matches: Vec<Range<usize>>,
}

impl MatchCache {
    fn get(&mut self, text: &str, needle: &str, o: MatchOptions) -> &[Range<usize>] {
        if self.text != text || self.needle != needle || self.options != o {
            self.matches = text::find(text, needle, o);
            self.text.clear();
            self.text.push_str(text);
            self.needle.clear();
            self.needle.push_str(needle);
            self.options = o;
        }
        &self.matches
    }
}

/// Scrolling by pressing the mouse wheel and moving the pointer, as in browsers.
struct Autoscroll {
    anchor: egui::Pos2,
    started: f64,
    moved: bool,
}

struct Message {
    text: String,
    error: bool,
}

impl Message {
    fn info(text: impl Into<String>) -> Option<Self> {
        Some(Self {
            text: text.into(),
            error: false,
        })
    }

    fn error(text: String) -> Option<Self> {
        Some(Self { text, error: true })
    }
}

struct App {
    store: Option<Store>,
    notes: Vec<Note>,
    selected: usize,
    list_filter: String,
    settings: Settings,
    panel: FindPanel,
    matches: MatchCache,
    undo: Vec<(u64, String)>,
    deleted: Option<Note>,
    message: Option<Message>,
    last_edit: Instant,
    counts: (u64, u64, text::Counts),
    cursor: (usize, usize),
    scroll_to_cursor: bool,
    focus_editor: bool,
    focus_list: bool,
    window_title: String,
    inbox: Option<instance::Inbox>,
    next_file_id: u64,
    autoscroll: Option<Autoscroll>,
    editor_area: egui::Rect,
    scroll_y: f32,
    quitting: bool,
    autostart: bool,
}

impl App {
    fn new(
        ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        store: std::io::Result<Store>,
        file: Option<PathBuf>,
        inbox: Option<instance::Inbox>,
    ) -> Self {
        theme::install(ctx);
        let settings: Settings = storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        ctx.set_theme(settings.theme.preference());
        let t = settings.lang.strings();

        let mut message = None;
        let (store, mut notes) = match store {
            Ok(s) => match s.load() {
                Ok(n) => (Some(s), n),
                Err(e) => {
                    message = Message::error((t.err_read)(&e.to_string()));
                    (None, Vec::new())
                }
            },
            Err(e) => {
                message = Message::error((t.err_folder)(&e.to_string()));
                (None, Vec::new())
            }
        };
        if notes.is_empty() {
            notes.push(Note::new(store.as_ref().map_or(0, Store::new_id)));
        }
        let selected = settings
            .last_note
            .and_then(|id| notes.iter().position(|n| n.id == id))
            .unwrap_or(0);

        let mut app = Self {
            store,
            notes,
            selected,
            list_filter: String::new(),
            panel: FindPanel {
                open: false,
                mode: Mode::Find,
                needle: String::new(),
                replacement: String::new(),
                word: String::new(),
                current: 0,
                focus: false,
            },
            settings,
            matches: MatchCache::default(),
            undo: Vec::new(),
            deleted: None,
            message,
            last_edit: Instant::now(),
            counts: (u64::MAX, 0, text::Counts::default()),
            cursor: (1, 1),
            scroll_to_cursor: false,
            focus_editor: true,
            focus_list: false,
            window_title: "Hien".into(),
            inbox,
            next_file_id: 0,
            autoscroll: None,
            editor_area: egui::Rect::NOTHING,
            scroll_y: 0.0,
            quitting: false,
            autostart: autostart::is_enabled(),
        };
        if let Some(path) = file {
            app.open_file(&path);
        }
        app
    }

    fn open_file(&mut self, path: &Path) {
        let open = self
            .notes
            .iter()
            .position(|n| n.file.as_ref().is_some_and(|f| f.path == path));
        if let Some(i) = open {
            return self.select(i);
        }
        // Outside files get small ids. Note ids are millisecond timestamps, so the two never meet.
        self.next_file_id += 1;
        let note = match store::open_file(path, self.next_file_id) {
            Ok(note) => note,
            Err(e) => {
                let name = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                self.message = Message::error((self.t().err_open_file)(&format!("{name}: {e}")));
                return;
            }
        };
        self.save_notes();
        // A fresh start has one blank note. It is pointless next to the opened file.
        if self.notes.len() == 1 && self.note().file.is_none() && self.note().text.trim().is_empty()
        {
            let blank = self.notes.remove(0);
            if let Some(s) = &self.store {
                let _ = s.trash(blank.id);
            }
        }
        self.notes.insert(0, note);
        self.selected = 0;
        self.panel.current = 0;
        self.message = None;
        self.focus_editor = true;
    }

    fn t(&self) -> &'static Strings {
        self.settings.lang.strings()
    }

    fn note(&self) -> &Note {
        &self.notes[self.selected]
    }

    fn editor_id(&self) -> Id {
        Id::unique(("editor", self.note().id))
    }

    fn edited(&mut self) {
        let note = &mut self.notes[self.selected];
        note.dirty = true;
        note.version += 1;
        self.last_edit = Instant::now();
    }

    fn save_notes(&mut self) {
        for note in self.notes.iter_mut().filter(|n| n.dirty) {
            let result = match (&note.file, &self.store) {
                (Some(f), _) => store::save_file(note, f),
                (None, Some(s)) => s.save(note),
                (None, None) => continue,
            };
            match result {
                Ok(()) => note.dirty = false,
                Err(e) => {
                    self.message =
                        Message::error((self.settings.lang.strings().err_save)(&e.to_string()));
                    return;
                }
            }
        }
    }

    // An empty note is dropped when leaving it so it does not clutter the list.
    fn select(&mut self, index: usize) {
        if index == self.selected {
            return;
        }
        self.save_notes();
        let mut target = index;
        if self.notes.len() > 1 && self.note().file.is_none() && self.note().text.trim().is_empty()
        {
            let empty = self.notes.remove(self.selected);
            if let Some(s) = &self.store {
                let _ = s.trash(empty.id);
            }
            if target > self.selected {
                target -= 1;
            }
        }
        self.selected = target;
        self.panel.current = 0;
        self.message = None;
        self.focus_editor = true;
    }

    fn new_note(&mut self) {
        if self.note().file.is_none() && self.note().text.trim().is_empty() {
            self.focus_editor = true;
            return;
        }
        self.save_notes();
        let id = self.store.as_ref().map_or(0, Store::new_id);
        self.notes.insert(0, Note::new(id));
        self.selected = 0;
        self.list_filter.clear();
        self.message = None;
        self.focus_editor = true;
    }

    fn delete(&mut self) {
        if self.note().file.is_some() {
            return self.close_file();
        }
        let note = self.notes.remove(self.selected);
        if let Some(s) = &self.store
            && let Err(e) = s.trash(note.id)
        {
            self.message = Message::error((self.t().err_delete)(&e.to_string()));
            self.notes.insert(self.selected, note);
            return;
        }
        if self.notes.is_empty() {
            self.notes
                .push(Note::new(self.store.as_ref().map_or(0, Store::new_id)));
        }
        self.selected = self.selected.min(self.notes.len() - 1);
        self.message = Message::info(self.t().moved_to_trash);
        self.deleted = Some(note);
    }

    /// Takes an outside file off the list. The file itself stays where it is.
    fn close_file(&mut self) {
        self.save_notes();
        if self.note().dirty {
            return;
        }
        self.notes.remove(self.selected);
        if self.notes.is_empty() {
            self.notes
                .push(Note::new(self.store.as_ref().map_or(0, Store::new_id)));
        }
        self.selected = self.selected.min(self.notes.len() - 1);
        self.message = None;
        self.focus_editor = true;
    }

    fn restore(&mut self) {
        let Some(note) = self.deleted.take() else {
            return;
        };
        if let Some(s) = &self.store
            && let Err(e) = s.restore(note.id)
        {
            self.message = Message::error((self.t().err_restore)(&e.to_string()));
            return;
        }
        self.notes.insert(0, note);
        self.selected = 0;
        self.message = None;
    }

    fn find_all(&self) -> Vec<Range<usize>> {
        text::find(&self.note().text, &self.panel.needle, self.settings.options)
    }

    fn set_text(&mut self, new: String, message: String) {
        let id = self.note().id;
        let old = std::mem::replace(&mut self.notes[self.selected].text, new);
        self.undo.push((id, old));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.edited();
        self.message = Message::info(message);
    }

    fn undo_last(&mut self) {
        let id = self.note().id;
        if self.undo.last().is_some_and(|(u, _)| *u == id) {
            let (_, old) = self.undo.pop().unwrap();
            self.notes[self.selected].text = old;
            self.edited();
            self.message = Message::info(self.t().undone);
        }
    }

    fn step(&mut self, ctx: &egui::Context, by: isize) {
        let m = self.find_all();
        if m.is_empty() {
            return;
        }
        let n = m.len() as isize;
        self.panel.current = (self.panel.current as isize + by).rem_euclid(n) as usize;
        self.select_match(ctx, &m);
    }

    fn select_match(&mut self, ctx: &egui::Context, m: &[Range<usize>]) {
        let Some(r) = m.get(self.panel.current) else {
            return;
        };
        let text = &self.note().text;
        let start = text::char_index(text, r.start);
        let end = start + text[r.clone()].chars().count();
        let id = self.editor_id();
        let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(start),
            CCursor::new(end),
        )));
        state.store(ctx, id);
        self.scroll_to_cursor = true;
    }

    fn replace_one(&mut self) {
        let m = self.find_all();
        let Some(r) = m.get(self.panel.current.min(m.len().saturating_sub(1))) else {
            return;
        };
        let new = text::replace(
            &self.note().text,
            std::slice::from_ref(r),
            &self.panel.replacement,
        );
        self.set_text(new, (self.t().replaced_one)(m.len() - 1));
    }

    fn replace_all(&mut self) {
        let m = self.find_all();
        if m.is_empty() {
            return;
        }
        let new = text::replace(&self.note().text, &m, &self.panel.replacement);
        self.set_text(new, (self.t().replaced_all)(m.len()));
    }

    fn insert(&mut self, everywhere: bool) {
        let word = self.panel.word.trim();
        if word.is_empty() {
            return;
        }
        let m = self.find_all();
        let targets: &[Range<usize>] = if everywhere {
            &m
        } else {
            match m.get(self.panel.current) {
                Some(r) => std::slice::from_ref(r),
                None => return,
            }
        };
        if targets.is_empty() {
            return;
        }
        let s = &self.settings;
        let (new, count) =
            text::insert(&self.note().text, targets, word, s.side, s.space, s.options);
        let skipped = targets.len() - count;
        let t = self.t();
        if count == 0 {
            self.message = Message::info(t.already_there);
            return;
        }
        let message = if skipped > 0 {
            (t.inserted_skipped)(count, skipped)
        } else {
            (t.inserted)(count)
        };
        self.set_text(new, message);
    }

    fn selected_text(&self, ctx: &egui::Context) -> Option<String> {
        let r = TextEdit::load_state(ctx, self.editor_id())?
            .cursor
            .char_range()?;
        let (a, b): (usize, usize) = (
            r.primary.index.min(r.secondary.index).into(),
            r.primary.index.max(r.secondary.index).into(),
        );
        let s: String = self.note().text.chars().skip(a).take(b - a).collect();
        (!s.is_empty() && !s.contains('\n')).then_some(s)
    }

    fn open_panel(&mut self, ctx: &egui::Context, mode: Mode) {
        if let Some(s) = self.selected_text(ctx) {
            self.panel.needle = s;
            self.panel.current = 0;
        }
        self.panel.open = true;
        self.panel.mode = mode;
        self.panel.focus = true;
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let key = |m, k| KeyboardShortcut::new(m, k);
        let ctrl = Modifiers::COMMAND;
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));

        if pressed(key(ctrl, Key::N)) {
            self.new_note();
        }
        if pressed(key(ctrl, Key::Q)) {
            self.quit(ctx);
        }
        if pressed(key(ctrl, Key::S)) {
            self.save_notes();
        }
        if pressed(key(ctrl, Key::F)) {
            self.open_panel(ctx, Mode::Find);
        }
        if pressed(key(ctrl, Key::H)) {
            self.open_panel(ctx, Mode::Replace);
        }
        if pressed(key(ctrl, Key::E)) {
            self.open_panel(ctx, Mode::Insert);
        }
        if pressed(key(ctrl, Key::K)) {
            self.focus_list = true;
        }
        if pressed(key(Modifiers::SHIFT, Key::F3)) {
            self.step(ctx, -1);
        }
        if pressed(key(Modifiers::NONE, Key::F3)) {
            self.step(ctx, 1);
        }
        if pressed(key(Modifiers::ALT, Key::ArrowDown)) && self.selected + 1 < self.notes.len() {
            self.select(self.selected + 1);
        }
        if pressed(key(Modifiers::ALT, Key::ArrowUp)) && self.selected > 0 {
            self.select(self.selected - 1);
        }
        if self.panel.open && pressed(key(Modifiers::NONE, Key::Escape)) {
            self.panel.open = false;
            self.focus_editor = true;
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let t = self.t();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(t.new_note).on_hover_text("Ctrl+N").clicked() {
                self.new_note();
            }
        });
        ui.add_space(4.0);
        let search = field(
            ui,
            p,
            &mut self.list_filter,
            t.search_notes,
            ui.available_width(),
        );
        if std::mem::take(&mut self.focus_list) {
            search.request_focus();
        }
        ui.add_space(6.0);

        let footer = 40.0;
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - footer)
            .show(ui, |ui| {
                let q = self.list_filter.trim();
                let mut shown = 0;
                for (i, note) in self.notes.iter().enumerate() {
                    if !q.is_empty()
                        && text::find(&note.text, q, MatchOptions::default()).is_empty()
                    {
                        continue;
                    }
                    shown += 1;
                    let title = note.title().unwrap_or_else(|| t.untitled.to_owned());
                    if row(ui, p, &title, i == self.selected).clicked() {
                        clicked = Some(i);
                    }
                }
                if shown == 0 {
                    ui.add_space(8.0);
                    ui.colored_label(p.weak, t.no_matching_notes);
                }
            });
        if let Some(i) = clicked {
            self.select(i);
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.menu_button(t.settings, |ui| {
                    ui.label(egui::RichText::new(t.language).color(p.weak));
                    ui.radio_value(&mut self.settings.lang, Lang::En, "English");
                    ui.radio_value(&mut self.settings.lang, Lang::Tr, "Türkçe");
                    ui.separator();
                    ui.label(egui::RichText::new(t.theme).color(p.weak));
                    for choice in [ThemeChoice::System, ThemeChoice::Dark, ThemeChoice::Light] {
                        if ui
                            .radio_value(&mut self.settings.theme, choice, choice.label(t))
                            .changed()
                        {
                            ui.ctx().set_theme(choice.preference());
                        }
                    }
                    ui.separator();
                    ui.checkbox(&mut self.settings.background, t.background);
                    if ui.checkbox(&mut self.autostart, t.autostart).changed() {
                        let result = if self.autostart {
                            autostart::enable()
                        } else {
                            autostart::disable()
                        };
                        if let Err(e) = result {
                            self.autostart = !self.autostart;
                            self.message = Message::error((t.err_autostart)(&e.to_string()));
                        }
                    }
                    if ui
                        .add(egui::Button::new(t.quit).shortcut_text("Ctrl+Q"))
                        .clicked()
                    {
                        self.quit(ui.ctx());
                    }
                });
                if let Some(s) = &self.store
                    && ui
                        .button(t.open_folder)
                        .on_hover_text(s.dir().display().to_string())
                        .clicked()
                    && let Err(e) = std::process::Command::new("explorer").arg(s.dir()).spawn()
                {
                    self.message = Message::error((t.err_open_folder)(&e.to_string()));
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (label, hint) = match &self.note().file {
                    Some(f) => (t.close_file, f.path.display().to_string()),
                    None => (t.delete, t.delete_hint.to_owned()),
                };
                if ui.button(label).on_hover_text(hint).clicked() {
                    self.delete();
                }
                let ctx = ui.ctx().clone();
                for (label, shortcut, mode) in [
                    (t.insert_word, "Ctrl+E", Mode::Insert),
                    (t.replace, "Ctrl+H", Mode::Replace),
                    (t.find, "Ctrl+F", Mode::Find),
                ] {
                    let active = self.panel.open && self.panel.mode == mode;
                    if ui
                        .selectable_label(active, label)
                        .on_hover_text(shortcut)
                        .clicked()
                    {
                        if active {
                            self.panel.open = false;
                        } else {
                            self.open_panel(&ctx, mode);
                        }
                    }
                }
            });
        });
    }

    fn find_panel(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let t = self.t();
        let ctx = ui.ctx().clone();
        let total = self
            .matches
            .get(
                &self.notes[self.selected].text,
                &self.panel.needle,
                self.settings.options,
            )
            .len();
        if self.panel.current >= total {
            self.panel.current = 0;
        }
        let dark = ctx.theme() == egui::Theme::Dark;

        Frame::new()
            .fill(p.field.gamma_multiply(if dark { 0.75 } else { 1.0 }))
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t.find_label);
                    let width = (ui.available_width() - 330.0).max(140.0);
                    let r = field(ui, p, &mut self.panel.needle, t.find_hint, width);
                    if std::mem::take(&mut self.panel.focus) {
                        r.request_focus();
                        select_all(&ctx, r.id, &self.panel.needle);
                    }
                    if r.changed() {
                        self.panel.current = 0;
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let back = ui.input(|i| i.modifiers.shift);
                        self.step(&ctx, if back { -1 } else { 1 });
                        r.request_focus();
                    }

                    let counter = match (self.panel.needle.is_empty(), total) {
                        (true, _) => String::new(),
                        (false, 0) => t.no_results.into(),
                        (false, n) => format!("{}/{}", self.panel.current + 1, n),
                    };
                    let color = if total == 0 && !self.panel.needle.is_empty() {
                        p.error_text
                    } else {
                        p.weak
                    };
                    ui.add_sized(
                        [72.0, 24.0],
                        egui::Label::new(egui::RichText::new(counter).color(color)),
                    );

                    if ui
                        .add_enabled(total > 0, egui::Button::new("↑"))
                        .on_hover_text(t.previous)
                        .clicked()
                    {
                        self.step(&ctx, -1);
                    }
                    if ui
                        .add_enabled(total > 0, egui::Button::new("↓"))
                        .on_hover_text(t.next)
                        .clicked()
                    {
                        self.step(&ctx, 1);
                    }
                    ui.toggle_value(&mut self.settings.options.match_case, "Aa")
                        .on_hover_text(t.match_case);
                    ui.toggle_value(&mut self.settings.options.whole_word, t.whole_word);
                    if ui.button("✕").on_hover_text(t.close).clicked() {
                        self.panel.open = false;
                    }
                });

                match self.panel.mode {
                    Mode::Find => {}
                    Mode::Replace => {
                        ui.horizontal(|ui| {
                            ui.label(t.with_label);
                            let width = (ui.available_width() - 330.0).max(140.0);
                            let r = field(ui, p, &mut self.panel.replacement, t.with_hint, width);
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.replace_one();
                                r.request_focus();
                            }
                            if ui
                                .add_enabled(total > 0, egui::Button::new(t.replace_one))
                                .clicked()
                            {
                                self.replace_one();
                            }
                            if ui
                                .add_enabled(total > 0, primary(p, t.replace_all))
                                .clicked()
                            {
                                self.replace_all();
                            }
                            self.undo_button(ui);
                        });
                    }
                    Mode::Insert => {
                        ui.horizontal(|ui| {
                            ui.label(t.word_label);
                            let width = (ui.available_width() - 440.0).max(120.0);
                            let r = field(ui, p, &mut self.panel.word, t.word_hint, width);
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.insert(true);
                                r.request_focus();
                            }
                            ui.selectable_value(&mut self.settings.side, Side::Before, t.before);
                            ui.selectable_value(&mut self.settings.side, Side::After, t.after);
                            ui.toggle_value(&mut self.settings.space, t.space)
                                .on_hover_text(t.space_hint);
                            let ready = total > 0 && !self.panel.word.trim().is_empty();
                            if ui
                                .add_enabled(ready, egui::Button::new(t.insert_here))
                                .clicked()
                            {
                                self.insert(false);
                            }
                            if ui.add_enabled(ready, primary(p, t.insert_all)).clicked() {
                                self.insert(true);
                            }
                            self.undo_button(ui);
                        });
                        let word = self.panel.word.trim();
                        if !word.is_empty() && !self.panel.needle.is_empty() {
                            let gap = if self.settings.space { " " } else { "" };
                            let example = match self.settings.side {
                                Side::Before => format!("{word}{gap}{}", self.panel.needle),
                                Side::After => format!("{}{gap}{word}", self.panel.needle),
                            };
                            ui.colored_label(p.weak, format!("{} {example}", t.preview));
                        }
                    }
                }
            });
    }

    fn undo_button(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let available = self
            .undo
            .last()
            .is_some_and(|(id, _)| *id == self.note().id);
        if ui
            .add_enabled(available, egui::Button::new(t.undo))
            .on_hover_text(t.undo_hint)
            .clicked()
        {
            self.undo_last();
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let target = self.autoscroll_target(ui, p);
        let id = self.editor_id();
        let hint = self.t().editor_hint;
        let highlight = self.panel.open && !self.panel.needle.is_empty();
        let needle = self.panel.needle.clone();
        let options = self.settings.options;
        let current = self.panel.current;
        let cache = &mut self.matches;
        let font = FontId::proportional(15.0);

        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
            let text = buf.as_str();
            let plain = TextFormat {
                font_id: font.clone(),
                color: p.text,
                ..Default::default()
            };
            let mut job = LayoutJob::default();
            let mut last = 0;
            if highlight {
                for (i, r) in cache.get(text, &needle, options).iter().enumerate() {
                    job.append(&text[last..r.start], 0.0, plain.clone());
                    let (bg, fg) = if i == current {
                        (p.accent_text, p.on_accent)
                    } else {
                        (p.highlight, p.text)
                    };
                    job.append(
                        &text[r.clone()],
                        0.0,
                        TextFormat {
                            background: bg,
                            color: fg,
                            ..plain.clone()
                        },
                    );
                    last = r.end;
                }
            }
            job.append(&text[last..], 0.0, plain);
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|f| f.layout_job(job))
        };

        let note = &mut self.notes[self.selected];
        let mut area = egui::ScrollArea::vertical()
            .id_salt(("scroll", note.id))
            .auto_shrink(false);
        if let Some(y) = target {
            area = area.vertical_scroll_offset(y);
        }
        let scrolled = area.show(ui, |ui| {
            let output = TextEdit::multiline(&mut note.text)
                .id(id)
                .frame(Frame::NONE)
                .hint_text(hint)
                .desired_width(f32::INFINITY)
                .min_size(ui.available_size())
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            if std::mem::take(&mut self.scroll_to_cursor)
                && let Some(r) = output.cursor_range
            {
                let rect = output
                    .galley
                    .pos_from_cursor(r.primary)
                    .translate(output.galley_pos.to_vec2());
                ui.scroll_to_rect(rect.expand(24.0), Some(Align::Center));
            }
            output
        });
        self.editor_area = scrolled.inner_rect;
        self.scroll_y = scrolled.state.clamped_offset().y;
        let output = scrolled.inner;

        if std::mem::take(&mut self.focus_editor) {
            output.response.request_focus();
        }
        if output.response.changed() {
            self.edited();
        }
        if let Some(r) = output.cursor_range {
            self.cursor = text::line_col(&self.notes[self.selected].text, r.primary.index.into());
        }
    }

    /// Starts, stops and steps autoscroll. Returns the scroll offset to use this frame.
    fn autoscroll_target(&mut self, ui: &egui::Ui, p: &Pal) -> Option<f32> {
        use egui::PointerButton::Middle;
        let (pointer, now, dt, middle_down, middle_up, other_press, esc) = ui.input(|i| {
            (
                i.pointer.hover_pos(),
                i.time,
                i.stable_dt.min(0.1),
                i.pointer.button_pressed(Middle),
                i.pointer.button_released(Middle),
                i.pointer.any_pressed() && !i.pointer.button_pressed(Middle),
                i.key_pressed(Key::Escape),
            )
        });

        if let Some(a) = &self.autoscroll {
            // A quick click leaves it running until the next click. Holding the wheel
            // down scrolls only while it is held.
            let held = a.moved || now - a.started > 0.3;
            if other_press || esc || (middle_down && now > a.started) || (middle_up && held) {
                self.autoscroll = None;
                return None;
            }
        } else if middle_down
            && let Some(at) = pointer
            && self.editor_area.contains(at)
        {
            self.autoscroll = Some(Autoscroll {
                anchor: at,
                started: now,
                moved: false,
            });
        }

        let a = self.autoscroll.as_mut()?;
        let ctx = ui.ctx();
        ctx.request_repaint();
        let marker = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::unique("autoscroll"),
        ));
        marker.circle(a.anchor, 13.0, p.field, Stroke::new(1.0, p.control_stroke));
        for dir in [-1.0, 1.0] {
            let tip = a.anchor + vec2(0.0, 8.0 * dir);
            let base = a.anchor + vec2(0.0, 3.0 * dir);
            marker.add(egui::Shape::convex_polygon(
                vec![tip, base + vec2(-4.0, 0.0), base + vec2(4.0, 0.0)],
                p.text,
                Stroke::NONE,
            ));
        }

        let dy = pointer.map_or(0.0, |at| at.y - a.anchor.y);
        let past = dy.abs() - AUTOSCROLL_DEAD_ZONE;
        if past <= 0.0 {
            ctx.set_cursor_icon(egui::CursorIcon::AllScroll);
            return None;
        }
        a.moved = true;
        ctx.set_cursor_icon(if dy < 0.0 {
            egui::CursorIcon::ResizeNorth
        } else {
            egui::CursorIcon::ResizeSouth
        });
        Some((self.scroll_y + dy.signum() * past * AUTOSCROLL_SPEED * dt).max(0.0))
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let t = self.t();
        let note = &self.notes[self.selected];
        if (self.counts.0, self.counts.1) != (note.id, note.version) {
            self.counts = (note.id, note.version, text::count(&note.text));
        }
        let dirty = self.notes.iter().any(|n| n.dirty);
        let c = self.counts.2;

        ui.horizontal_centered(|ui| {
            let mut restore = false;
            match &self.message {
                Some(m) => {
                    ui.colored_label(if m.error { p.error_text } else { p.weak }, &m.text);
                    if !m.error && self.deleted.is_some() && ui.link(t.restore).clicked() {
                        restore = true;
                    }
                }
                None if self.store.is_none() => {
                    ui.colored_label(p.error_text, t.saving_off);
                }
                None => {
                    ui.colored_label(p.weak, if dirty { t.saving } else { t.saved });
                }
            }
            if restore {
                self.restore();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.colored_label(p.weak, (t.line_col)(self.cursor.0, self.cursor.1));
                ui.colored_label(p.weak, (t.chars)(c.chars));
                ui.colored_label(p.weak, (t.words)(c.words));
                ui.colored_label(p.weak, (t.lines)(c.lines));
            });
        });
    }
}

impl App {
    fn tick(&mut self, ctx: &egui::Context) {
        if let Some(inbox) = self.inbox.clone() {
            let requests = std::mem::take(&mut *inbox.lock().unwrap());
            if !requests.is_empty() {
                for path in requests.into_iter().flatten() {
                    self.open_file(&path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        self.shortcuts(ctx);
        // Closing only hides the window, so the next file opens without starting Hien again.
        if ctx.input(|i| i.viewport().close_requested())
            && self.settings.background
            && !self.quitting
        {
            self.save_notes();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if self.notes.iter().any(|n| n.dirty) {
            let elapsed = self.last_edit.elapsed();
            if elapsed >= SAVE_DELAY {
                self.save_notes();
            } else {
                ctx.request_repaint_after(SAVE_DELAY - elapsed);
            }
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let p = *theme::pal(ui.ctx());
        let title = match self.note().title() {
            Some(name) if self.note().file.is_some() => format!("{name} - Hien"),
            _ => "Hien".to_owned(),
        };
        if title != self.window_title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }

        egui::Panel::bottom("status")
            .exact_size(23.0)
            .frame(
                Frame::new()
                    .fill(p.status)
                    .inner_margin(Margin::symmetric(12, 0))
                    .stroke(Stroke::new(1.0, p.divider)),
            )
            .show(ui, |ui| self.status_bar(ui, &p));

        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(240.0)
            .min_size(180.0)
            .frame(
                Frame::new()
                    .fill(p.content)
                    .inner_margin(Margin::symmetric(8, 0))
                    .stroke(Stroke::new(1.0, p.line)),
            )
            .show(ui, |ui| self.sidebar(ui, &p));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.content).inner_margin(Margin {
                left: 24,
                right: 16,
                top: 4,
                bottom: 8,
            }))
            .show(ui, |ui| {
                self.toolbar(ui);
                if self.panel.open {
                    self.find_panel(ui, &p);
                    ui.add_space(8.0);
                }
                self.editor(ui, &p);
            });
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.last_note = Some(self.note().id);
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_notes();
    }
}

fn primary<'a>(p: &Pal, label: &'a str) -> egui::Button<'a> {
    egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
        .fill(p.accent)
        .stroke(Stroke::NONE)
}

fn select_all(ctx: &egui::Context, id: Id, text: &str) {
    if let Some(mut state) = TextEdit::load_state(ctx, id) {
        let n = text.chars().count();
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(n))));
        state.store(ctx, id);
    }
}

/// Single-line input: thin line under the box, thicker accent line when focused.
fn field(ui: &mut egui::Ui, p: &Pal, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let light = ui.ctx().theme() == egui::Theme::Light;
    let output = TextEdit::singleline(text)
        .hint_text(egui::RichText::new(hint).color(p.weak))
        .desired_width(width - 16.0)
        .frame(
            Frame::new()
                .fill(p.field)
                .corner_radius(4)
                .inner_margin(Margin::symmetric(8, 4))
                .stroke(if light {
                    Stroke::new(1.0, p.line)
                } else {
                    Stroke::NONE
                }),
        )
        .show(ui);
    let r = output.response.response;
    let (width, color) = if r.has_focus() {
        (2.0, p.accent_text)
    } else {
        (1.0, p.control_stroke)
    };
    let x = r.rect.x_range().shrink(2.0);
    ui.painter()
        .hline(x, r.rect.bottom() - width / 2.0, Stroke::new(width, color));
    r
}

fn row(ui: &mut egui::Ui, p: &Pal, title: &str, selected: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let painter = ui.painter_at(r);
    let bg = if selected {
        Some(p.nav_sel)
    } else if response.hovered() {
        Some(p.hover)
    } else {
        None
    };
    if let Some(bg) = bg {
        painter.rect_filled(r, 5, bg);
    }
    if selected {
        let stripe = egui::Rect::from_min_size(pos2(r.left(), r.center().y - 8.0), vec2(3.0, 16.0));
        painter.rect_filled(stripe, 1.5, p.accent_text);
    }
    painter.text(
        pos2(r.left() + 12.0, r.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.0),
        if selected { p.text } else { p.weak },
    );
    response
}

fn main() -> eframe::Result {
    let mut background = false;
    let mut file = None;
    for arg in std::env::args_os().skip(1) {
        if arg == autostart::BACKGROUND_FLAG {
            background = true;
        } else {
            file = Some(PathBuf::from(arg));
        }
    }
    // A sign-in launch stays quiet when Hien is already up. Any other launch
    // hands its file to the running window.
    let running = if background {
        instance::already_running()
    } else {
        instance::send(file.as_deref())
    };
    if running {
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Hien")
            .with_app_id("Hien")
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([620.0, 380.0])
            .with_visible(!background),
        ..Default::default()
    };
    eframe::run_native(
        "Hien",
        options,
        Box::new(|cc| {
            Ok(Box::new(App::new(
                &cc.egui_ctx,
                cc.storage,
                Store::open(),
                file,
                instance::listen(cc.egui_ctx.clone(), instance::window_of(cc)),
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        app: &mut App,
        events: Vec<egui::Event>,
    ) -> Vec<egui::ViewportCommand> {
        frame_with(ctx, app, events, false)
    }

    fn frame_with(
        ctx: &egui::Context,
        app: &mut App,
        events: Vec<egui::Event>,
        close: bool,
    ) -> Vec<egui::ViewportCommand> {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(1000.0, 700.0),
            )),
            events,
            ..Default::default()
        };
        if close {
            let root = input.viewports.entry(egui::ViewportId::ROOT).or_default();
            root.events.push(egui::ViewportEvent::Close);
        }
        let mut out = ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.draw(ui);
        });
        out.textures_delta.clear();
        out.viewport_output
            .remove(&egui::ViewportId::ROOT)
            .map_or(Vec::new(), |v| v.commands)
    }

    fn app_with_long_note(name: &str) -> (egui::Context, App, Id) {
        let dir = std::env::temp_dir().join(format!("hien-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::at(dir).unwrap();
        let note = Note {
            text: (1..=400)
                .map(|i| {
                    format!(
                        "line {i}
"
                    )
                })
                .collect(),
            ..Note::new(7)
        };
        store.save(&note).unwrap();
        let ctx = egui::Context::default();
        let app = App::new(&ctx, None, Ok(store), None, None);
        let id = app.editor_id();
        (ctx, app, id)
    }

    fn editor_top(ctx: &egui::Context, id: Id) -> f32 {
        ctx.read_response(id).unwrap().rect.top()
    }

    #[test]
    fn mouse_wheel_scrolls_the_editor() {
        let (ctx, mut app, id) = app_with_long_note("wheel");
        let over_editor = egui::Event::PointerMoved(pos2(600.0, 400.0));
        frame(&ctx, &mut app, vec![over_editor.clone()]);
        frame(&ctx, &mut app, vec![]);
        let before = editor_top(&ctx, id);
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, -5.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: egui::MouseWheelSource::Wheel,
        };
        frame(&ctx, &mut app, vec![over_editor, wheel]);
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![]);
        }
        assert!(
            editor_top(&ctx, id) < before - 50.0,
            "{before} -> {}",
            editor_top(&ctx, id)
        );
    }

    #[test]
    fn closing_hides_and_a_new_launch_shows_the_window() {
        let (ctx, mut app, _) = app_with_long_note("background");
        let inbox = instance::Inbox::default();
        app.inbox = Some(inbox.clone());
        frame(&ctx, &mut app, vec![]);

        let commands = frame_with(&ctx, &mut app, vec![], true);
        assert!(commands.contains(&egui::ViewportCommand::CancelClose));
        assert!(commands.contains(&egui::ViewportCommand::Visible(false)));

        inbox.lock().unwrap().push(None);
        let commands = frame(&ctx, &mut app, vec![]);
        assert!(commands.contains(&egui::ViewportCommand::Visible(true)));

        app.quit(&ctx);
        let commands = frame_with(&ctx, &mut app, vec![], true);
        assert!(!commands.contains(&egui::ViewportCommand::CancelClose));
    }

    fn middle(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Middle,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn middle_click_autoscrolls_until_next_click() {
        let (ctx, mut app, id) = app_with_long_note("autoscroll");
        let anchor = pos2(600.0, 300.0);
        let below = egui::Event::PointerMoved(pos2(600.0, 500.0));
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(anchor)]);
        frame(&ctx, &mut app, vec![middle(anchor, true)]);
        frame(&ctx, &mut app, vec![middle(anchor, false)]);
        let start = editor_top(&ctx, id);
        frame(&ctx, &mut app, vec![below]);
        for _ in 0..20 {
            frame(&ctx, &mut app, vec![]);
        }
        let scrolled = editor_top(&ctx, id);
        assert!(scrolled < start - 100.0, "{start} -> {scrolled}");

        let click = |pressed| egui::Event::PointerButton {
            pos: pos2(600.0, 500.0),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut app, vec![click(true)]);
        frame(&ctx, &mut app, vec![click(false)]);
        let stopped = editor_top(&ctx, id);
        for _ in 0..10 {
            frame(&ctx, &mut app, vec![]);
        }
        assert!(app.autoscroll.is_none());
        assert_eq!(editor_top(&ctx, id), stopped);
    }
}
