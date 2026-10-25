use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, CssProvider, Entry, EventControllerKey, Label,
    ListBox, ListBoxRow, Orientation, ScrolledWindow, SelectionMode,
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "io.github.ipvesp.ClipboardHistory";
const DEFAULT_HISTORY_LIMIT: usize = 100;

const POPUP_WIDTH: i32 = 420;
const POPUP_HEIGHT: i32 = 350;
const POPUP_OFFSET_X: i32 = 14;
const POPUP_OFFSET_Y: i32 = 14;
const POPUP_MARGIN: i32 = 10;

struct Popup {
    window: ApplicationWindow,
    search: Entry,
    list: ListBox,
    items: Rc<RefCell<Vec<ClipboardItem>>>,
    saved_size: Rc<Cell<(i32, i32)>>,
}

#[derive(Debug, Clone)]
struct ClipboardItem {
    id: i64,
    content: String,
}

#[derive(Debug, Deserialize)]
struct HyprCursorPos {
    x: f64,
    y: f64,
}

#[derive(Debug, Deserialize)]
struct HyprMonitor {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    focused: bool,
}

#[derive(Debug, Deserialize)]
struct HyprClient {
    address: String,
    class: Option<String>,
    size: [i32; 2],
    floating: bool,

    #[serde(rename = "initialClass")]
    initial_class: Option<String>,

    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    history_limit: usize,
    show_bar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_limit: DEFAULT_HISTORY_LIMIT,
            show_bar: true,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("ch") => {
            if let Err(err) = run_cli(&args[2..]) {
                eprintln!("{err}");
                std::process::exit(2);
            }
        }
        Some("store") => {
            if let Err(err) = store_from_stdin() {
                eprintln!("Store error: {err}");
            }
        }

        Some("clear") => {
            if let Err(err) = clear_history() {
                eprintln!("Clear error: {err}");
            }
        }

        _ => {
            run_ui();
        }
    }
}

fn config_path() -> PathBuf {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .expect("HOME is not set");
    base.join("clipboard-history/settings.json")
}

fn settings() -> Settings {
    fs::read(config_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
        .map(|mut settings| {
            settings.history_limit = settings.history_limit.clamp(5, 200);
            settings
        })
        .unwrap_or_default()
}

fn save_settings(value: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path();
    fs::create_dir_all(path.parent().ok_or("Invalid settings path")?)?;
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn run_cli(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut value = settings();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--help"] | ["help"] | [] => println!(
            "Usage: vesp ch <command>\n\n  open                 Open clipboard history\n  history-limit <5-200> Set saved item count\n  bar show|hide        Show or hide bar icon\n  status [--json]      Show current settings\n  clear                Clear clipboard history"
        ),
        ["open"] => run_ui(),
        ["clear"] => clear_history()?,
        ["history-limit", count] => {
            let count: usize = count.parse()?;
            if !(5..=200).contains(&count) {
                return Err("History limit must be 5–200".into());
            }
            value.history_limit = count;
            save_settings(&value)?;
            trim_history(count)?;
            println!("History limit: {count}");
        }
        ["bar", "show"] => {
            value.show_bar = true;
            save_settings(&value)?;
            println!("Bar icon: shown");
        }
        ["bar", "hide"] => {
            value.show_bar = false;
            save_settings(&value)?;
            println!("Bar icon: hidden");
        }
        ["status", "--json"] => println!("{}", serde_json::to_string(&value)?),
        ["status"] => println!(
            "History limit: {}\nBar icon: {}",
            value.history_limit,
            if value.show_bar { "shown" } else { "hidden" }
        ),
        _ => return Err("Unknown command. Run: vesp ch --help".into()),
    }
    Ok(())
}

fn database_path() -> PathBuf {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .expect("HOME bulunamadı");

    let directory = base.join("clipboard-history");

    if let Err(err) = fs::create_dir_all(&directory) {
        eprintln!("Veri klasörü oluşturulamadı: {err}");
    }

    let path = directory.join("history.db");
    if !path.exists() {
        let old = base.join("clipper/history.db");
        if old.exists() {
            if let Ok(source) = Connection::open(old) {
                let _ = source.execute("VACUUM INTO ?1", params![path.to_string_lossy()]);
            }
        }
    }
    path
}

fn open_database() -> rusqlite::Result<Connection> {
    let connection = Connection::open(database_path())?;

    connection.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS clipboard_items (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            content    TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE INDEX IF NOT EXISTS idx_clipboard_created
        ON clipboard_items(created_at DESC);
        ",
    )?;

    Ok(connection)
}

fn store_from_stdin() -> Result<(), Box<dyn std::error::Error>> {
    let mut content = String::new();
    io::stdin().read_to_string(&mut content)?;

    while content.ends_with('\n') || content.ends_with('\r') {
        content.pop();
    }

    if content.trim().is_empty() {
        return Ok(());
    }

    // Şimdilik 1 MB üzerindeki clipboard içeriklerini kaydetme.
    if content.len() > 1_000_000 {
        return Ok(());
    }

    let mut connection = open_database()?;
    let transaction = connection.transaction()?;

    // Aynı içerik tekrar kopyalanırsa eski kaydı silip en üste taşı.
    transaction.execute(
        "DELETE FROM clipboard_items WHERE content = ?1",
        params![content],
    )?;

    transaction.execute(
        "
        INSERT INTO clipboard_items(content, created_at)
        VALUES (?1, unixepoch())
        ",
        params![content],
    )?;

    // Keep the configured number of unique clipboard entries.
    transaction.execute(
        "
        DELETE FROM clipboard_items
        WHERE id NOT IN (
            SELECT id
            FROM clipboard_items
            ORDER BY created_at DESC, id DESC
            LIMIT ?1
        )
        ",
        params![settings().history_limit as i64],
    )?;

    transaction.commit()?;

    Ok(())
}

fn trim_history(limit: usize) -> rusqlite::Result<()> {
    let connection = open_database()?;
    connection.execute(
        "DELETE FROM clipboard_items WHERE id NOT IN
         (SELECT id FROM clipboard_items ORDER BY created_at DESC, id DESC LIMIT ?1)",
        params![limit as i64],
    )?;
    Ok(())
}

fn clear_history() -> rusqlite::Result<()> {
    let connection = open_database()?;
    connection.execute("DELETE FROM clipboard_items", [])?;

    println!("Clipboard geçmişi temizlendi.");

    Ok(())
}

fn load_items(search: &str) -> rusqlite::Result<Vec<ClipboardItem>> {
    let connection = open_database()?;
    let mut output = Vec::new();

    if search.trim().is_empty() {
        let mut statement = connection.prepare(
            "
            SELECT id, content
            FROM clipboard_items
            ORDER BY created_at DESC, id DESC
            LIMIT ?1
            ",
        )?;

        let rows = statement.query_map(params![settings().history_limit as i64], |row| {
            Ok(ClipboardItem {
                id: row.get(0)?,
                content: row.get(1)?,
            })
        })?;

        for row in rows {
            output.push(row?);
        }
    } else {
        let search = format!("%{}%", search);

        let mut statement = connection.prepare(
            "
            SELECT id, content
            FROM clipboard_items
            WHERE content LIKE ?1
            ORDER BY created_at DESC, id DESC
            LIMIT ?2
            ",
        )?;

        let rows =
            statement.query_map(params![search, settings().history_limit as i64], |row| {
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    content: row.get(1)?,
                })
            })?;

        for row in rows {
            output.push(row?);
        }
    }

    Ok(output)
}

fn delete_item(id: i64) -> rusqlite::Result<()> {
    let connection = open_database()?;

    connection.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])?;

    Ok(())
}

fn copy_to_clipboard(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut process = Command::new("wl-copy").stdin(Stdio::piped()).spawn()?;

    if let Some(stdin) = process.stdin.as_mut() {
        stdin.write_all(content.as_bytes())?;
    }

    let status = process.wait()?;

    if !status.success() {
        return Err("wl-copy başarısız oldu".into());
    }

    Ok(())
}

fn run_ui() {
    let application = Application::builder().application_id(APP_ID).build();

    let popup: Rc<RefCell<Option<Popup>>> = Rc::new(RefCell::new(None));
    let hold: Rc<RefCell<Option<gtk::gio::ApplicationHoldGuard>>> = Rc::new(RefCell::new(None));
    let css_provider: RefCell<Option<CssProvider>> = RefCell::new(None);
    application.connect_activate(move |app| {
        if css_provider.borrow().is_none() {
            let provider = CssProvider::new();
            if let Some(display) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            *css_provider.borrow_mut() = Some(provider);
        }
        if let Some(provider) = css_provider.borrow().as_ref() {
            load_css(provider);
        }

        if hold.borrow().is_none() {
            *hold.borrow_mut() = Some(app.hold());
        }

        if let Some(popup) = popup.borrow().as_ref() {
            let size = if popup.window.is_visible() {
                None
            } else {
                Some(popup.saved_size.get())
            };
            popup.window.present();
            position_popup(&popup.search, size);
            if popup.search.text().is_empty() {
                refresh_list(&popup.list, &popup.items, "");
            } else {
                popup.search.set_text("");
            }
        } else {
            *popup.borrow_mut() = Some(build_ui(app));
        }
    });

    application.run_with_args(&["clipboard-history"]);
}

fn build_ui(app: &Application) -> Popup {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Clipboard")
        .default_width(POPUP_WIDTH)
        .default_height(POPUP_HEIGHT)
        .resizable(true)
        .decorated(false)
        .build();

    window.add_css_class("clipper-window");
    let saved_size = Rc::new(Cell::new((POPUP_WIDTH, POPUP_HEIGHT)));

    let shell = GtkBox::new(Orientation::Horizontal, 0);
    let rail = GtkBox::new(Orientation::Vertical, 0);
    rail.add_css_class("clipper-rail");
    shell.append(&rail);

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_hexpand(true);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let title = Label::new(Some("CLIPBOARD HISTORY"));
    title.set_xalign(0.0);
    title.add_css_class("clipper-title");

    let search = Entry::new();
    search.set_placeholder_text(Some("Search"));
    search.add_css_class("clipper-search");

    let list = ListBox::new();
    list.set_selection_mode(SelectionMode::Single);
    list.add_css_class("clipper-list");
    let empty = Label::new(Some("No items"));
    empty.add_css_class("clipper-empty");
    list.set_placeholder(Some(&empty));

    let scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .child(&list)
        .build();

    root.append(&search);
    root.append(&title);
    root.append(&scroll);

    shell.append(&root);
    window.set_child(Some(&shell));

    let items: Rc<RefCell<Vec<ClipboardItem>>> = Rc::new(RefCell::new(Vec::new()));

    refresh_list(&list, &items, "");

    if let Some(first_row) = list.row_at_index(0) {
        list.select_row(Some(&first_row));
    }

    {
        let list = list.clone();
        let items = items.clone();

        search.connect_changed(move |entry| {
            refresh_list(&list, &items, entry.text().as_str());

            if let Some(first_row) = list.row_at_index(0) {
                list.select_row(Some(&first_row));
            }
        });
    }

    {
        let items = items.clone();
        let window = window.clone();
        let saved_size = saved_size.clone();

        list.connect_row_activated(move |_list, row| {
            let index = row.index();

            if index < 0 {
                return;
            }

            if let Some(item) = items.borrow().get(index as usize) {
                if let Err(err) = copy_to_clipboard(&item.content) {
                    eprintln!("Clipboard copy error: {err}");
                    return;
                }

                hide_popup(&window, &saved_size);
            }
        });
    }

    let key_controller = EventControllerKey::new();

    {
        let window = window.clone();
        let list = list.clone();
        let search = search.clone();
        let items = items.clone();
        let saved_size = saved_size.clone();

        key_controller.connect_key_pressed(move |_controller, key, _keycode, modifiers| {
            match key {
                gdk::Key::Escape => {
                    hide_popup(&window, &saved_size);
                    return glib::Propagation::Stop;
                }

                gdk::Key::Return | gdk::Key::KP_Enter => {
                    if let Some(row) = list.selected_row() {
                        let index = row.index();

                        if index >= 0 {
                            if let Some(item) = items.borrow().get(index as usize) {
                                if let Err(err) = copy_to_clipboard(&item.content) {
                                    eprintln!("Clipboard copy error: {err}");
                                } else {
                                    hide_popup(&window, &saved_size);
                                }
                            }
                        }
                    }

                    return glib::Propagation::Stop;
                }

                gdk::Key::Delete => {
                    if let Some(row) = list.selected_row() {
                        let index = row.index();

                        if index >= 0 {
                            let id = items.borrow().get(index as usize).map(|item| item.id);

                            if let Some(id) = id {
                                if let Err(err) = delete_item(id) {
                                    eprintln!("Delete error: {err}");
                                }

                                refresh_list(&list, &items, search.text().as_str());

                                if let Some(next_row) = list
                                    .row_at_index(index)
                                    .or_else(|| list.row_at_index(index - 1))
                                {
                                    list.select_row(Some(&next_row));
                                }
                            }
                        }
                    }

                    return glib::Propagation::Stop;
                }

                gdk::Key::Up => {
                    move_selection(&list, -1);
                    return glib::Propagation::Stop;
                }

                gdk::Key::Down => {
                    move_selection(&list, 1);
                    return glib::Propagation::Stop;
                }

                _ => {}
            }

            if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }

            glib::Propagation::Proceed
        });
    }

    window.add_controller(key_controller);

    {
        let saved_size = saved_size.clone();
        window.connect_close_request(move |window| {
            hide_popup(window, &saved_size);
            glib::Propagation::Stop
        });
    }

    window.present();
    position_popup(&search, Some((POPUP_WIDTH, POPUP_HEIGHT)));

    Popup {
        window,
        search,
        list,
        items,
        saved_size,
    }
}

fn position_popup(search: &Entry, size: Option<(i32, i32)>) {
    let search = search.clone();
    let mut attempts = 0;

    // Hyprland pencereyi kaydedene kadar konumlandırmayı yeniden dene.
    glib::timeout_add_local(Duration::from_millis(20), move || {
        attempts += 1;
        match move_clipper_near_cursor(size) {
            Ok(()) => {
                search.grab_focus();
                glib::ControlFlow::Break
            }
            Err(_) if attempts < 25 => glib::ControlFlow::Continue,
            Err(err) => {
                eprintln!("Popup position error: {err}");
                search.grab_focus();
                glib::ControlFlow::Break
            }
        }
    });
}

fn hide_popup(window: &ApplicationWindow, saved_size: &Cell<(i32, i32)>) {
    if let Ok(json) = hyprctl_output(&["clients", "-j"])
        && let Ok(clients) = serde_json::from_str::<Vec<HyprClient>>(&json)
        && let Some(client) = clients.into_iter().find(|client| {
            client.class.as_deref() == Some(APP_ID)
                || client.initial_class.as_deref() == Some(APP_ID)
        })
        && client.size[0] > 0
        && client.size[1] > 0
    {
        saved_size.set((client.size[0], client.size[1]));
    }

    window.hide();
}

fn refresh_list(list: &ListBox, items: &Rc<RefCell<Vec<ClipboardItem>>>, search: &str) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let loaded = match load_items(search) {
        Ok(items) => items,
        Err(err) => {
            eprintln!("Database read error: {err}");
            Vec::new()
        }
    };

    for item in &loaded {
        let row = create_row(item);
        list.append(&row);
    }

    *items.borrow_mut() = loaded;
}

fn create_row(item: &ClipboardItem) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("clipper-row");

    let preview = normalize_preview(&item.content);

    let label = Label::new(Some(&preview));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_margin_top(11);
    label.set_margin_bottom(11);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(46);
    label.set_single_line_mode(true);
    label.add_css_class("clipper-preview");

    row.set_child(Some(&label));

    row
}

fn normalize_preview(content: &str) -> String {
    let content = content
        .replace('\n', " ")
        .replace('\r', " ")
        .replace('\t', " ");

    let content = content.trim();

    const MAX_CHARS: usize = 90;

    if content.chars().count() <= MAX_CHARS {
        return content.to_string();
    }

    let mut preview: String = content.chars().take(MAX_CHARS).collect();

    preview.push('…');

    preview
}

fn move_selection(list: &ListBox, offset: i32) {
    let current = list.selected_row().map(|row| row.index()).unwrap_or(0);

    let target = (current + offset).max(0);

    if let Some(row) = list.row_at_index(target) {
        list.select_row(Some(&row));
    }
}

fn hyprctl_output(args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("hyprctl").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("hyprctl {:?} başarısız oldu: {}", args, stderr).into());
    }

    Ok(String::from_utf8(output.stdout)?)
}

fn move_clipper_near_cursor(
    requested_size: Option<(i32, i32)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cursor_json = hyprctl_output(&["cursorpos", "-j"])?;
    let monitors_json = hyprctl_output(&["monitors", "-j"])?;
    let clients_json = hyprctl_output(&["clients", "-j"])?;

    let cursor: HyprCursorPos = serde_json::from_str(&cursor_json)?;
    let monitors: Vec<HyprMonitor> = serde_json::from_str(&monitors_json)?;
    let clients: Vec<HyprClient> = serde_json::from_str(&clients_json)?;

    let monitor = monitors
        .iter()
        .find(|m| {
            cursor.x >= m.x as f64
                && cursor.x < (m.x + m.width) as f64
                && cursor.y >= m.y as f64
                && cursor.y < (m.y + m.height) as f64
        })
        .or_else(|| monitors.iter().find(|m| m.focused))
        .ok_or("Monitor bulunamadı")?;

    let client = clients
        .iter()
        .rev()
        .find(|c| {
            c.class.as_deref() == Some(APP_ID)
                || c.initial_class.as_deref() == Some(APP_ID)
                || c.title.as_deref() == Some("Clipboard")
        })
        .ok_or("Clipper window bulunamadı")?;

    let width = requested_size
        .map(|size| size.0)
        .unwrap_or(client.size[0])
        .max(1);
    let height = requested_size
        .map(|size| size.1)
        .unwrap_or(client.size[1])
        .max(1);

    let mut x = cursor.x as i32 + POPUP_OFFSET_X;
    let mut y = cursor.y as i32 + POPUP_OFFSET_Y;

    // Sağdan taşıyorsa cursor'ın soluna al.
    if x + width > monitor.x + monitor.width - POPUP_MARGIN {
        x = cursor.x as i32 - width - POPUP_OFFSET_X;
    }

    // Alttan taşıyorsa cursor'ın üstüne al.
    if y + height > monitor.y + monitor.height - POPUP_MARGIN {
        y = cursor.y as i32 - height - POPUP_OFFSET_Y;
    }

    // Monitör sınırlarını garanti et.
    if x < monitor.x + POPUP_MARGIN {
        x = monitor.x + POPUP_MARGIN;
    }

    if y < monitor.y + POPUP_MARGIN {
        y = monitor.y + POPUP_MARGIN;
    }

    let max_x = (monitor.x + monitor.width - width - POPUP_MARGIN).max(monitor.x + POPUP_MARGIN);
    let max_y = (monitor.y + monitor.height - height - POPUP_MARGIN).max(monitor.y + POPUP_MARGIN);

    x = x.min(max_x);
    y = y.min(max_y);

    // Hyprland 0.55+ pencereleri Lua dispatcher'larıyla yönetiyor.
    let float_script = format!(
        "hl.dispatch(hl.dsp.window.float({{ action = 'set', window = 'address:{}' }}))",
        client.address
    );
    let output = Command::new("hyprctl")
        .args(["eval", &float_script])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Pencere yüzdürülemedi: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Haritalama sırasında dispatcher başarılı dönse bile pencere hâlâ tiled olabilir.
    let current_json = hyprctl_output(&["clients", "-j"])?;
    let current_clients: Vec<HyprClient> = serde_json::from_str(&current_json)?;
    if !current_clients
        .iter()
        .any(|c| c.address == client.address && c.floating)
    {
        return Err("Pencere henüz yüzmüyor".into());
    }

    let resize = if requested_size.is_some() {
        format!(
            "hl.dispatch(hl.dsp.window.resize({{ x = {}, y = {}, window = w }})); ",
            width, height
        )
    } else {
        String::new()
    };
    let script = format!(
        "local w = 'address:{}'; \
         hl.dispatch(hl.dsp.window.fullscreen({{ action = 'unset', window = w }})); \
         {}\
         hl.dispatch(hl.dsp.window.set_prop({{ prop = 'rounding', value = 0, window = w }})); \
         hl.dispatch(hl.dsp.window.set_prop({{ prop = 'opacity', value = '1.0 override', window = w }})); \
         hl.dispatch(hl.dsp.window.move({{ x = {}, y = {}, window = w }}))",
        client.address, resize, x, y
    );

    let output = Command::new("hyprctl").args(["eval", &script]).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Pencere taşınamadı: {stderr}").into());
    }

    if let Some((target_width, target_height)) = requested_size {
        let json = hyprctl_output(&["clients", "-j"])?;
        let updated: Vec<HyprClient> = serde_json::from_str(&json)?;
        let ready = updated.iter().any(|c| {
            c.address == client.address
                && c.floating
                && (c.size[0] - target_width).abs() <= 2
                && (c.size[1] - target_height).abs() <= 2
        });
        if !ready {
            return Err("Pencere boyutu henüz uygulanmadı".into());
        }
    }

    Ok(())
}

fn theme_color(palette: Option<&toml::Value>, key: &str, fallback: &str) -> String {
    palette
        .and_then(|colors| colors.get(key))
        .and_then(toml::Value::as_str)
        .filter(|value| {
            value.len() == 7
                && value.starts_with('#')
                && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .unwrap_or(fallback)
        .to_string()
}

fn load_css(provider: &CssProvider) {
    let palette = env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local/state/omarchy/current/theme/colors.toml"))
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| toml::from_str::<toml::Value>(&content).ok());
    let palette = palette.as_ref();

    let mut css = r#"
    window.clipper-window {
        background: __BG__;
        color: __FG__;
        border: 1px solid __BORDER__;
        border-radius: 0;
    }

    .clipper-rail {
        background: __ACCENT__;
        min-width: 3px;
    }

    .clipper-title {
        font-size: 11px;
        font-weight: 700;
        letter-spacing: 1px;
        color: __MUTED__;
    }

    entry.clipper-search {
        min-height: 44px;
        border-radius: 0;
        padding-left: 2px;
        padding-right: 2px;
        background: transparent;
        color: __FG__;
        border: 0;
        border-bottom: 1px solid __BORDER__;
        box-shadow: none;
    }

    entry.clipper-search:focus,
    entry.clipper-search:focus-within {
        border-bottom: 2px solid __ACCENT__;
        outline: none;
        box-shadow: none;
    }

    .clipper-list {
        background: transparent;
    }

    .clipper-row {
        border-radius: 0;
        margin-top: 0;
        margin-bottom: 0;
        color: __FG__;
        background: transparent;
        border-bottom: 1px solid __BORDER__;
    }

    .clipper-row:hover {
        background: __SURFACE__;
    }

    .clipper-row:selected {
        background: __SELECTION__;
        border-left: 2px solid __ACCENT__;
    }

    .clipper-preview {
        color: __FG__;
        font-size: 13px;
    }

    .clipper-row:selected .clipper-preview {
        color: __FG__;
    }

    .clipper-empty {
        color: __MUTED__;
        padding: 32px;
    }
    "#
    .to_string();

    for (placeholder, key, fallback) in [
        ("__BG__", "background", "@theme_bg_color"),
        ("__FG__", "foreground", "@theme_fg_color"),
        ("__SURFACE__", "lighter_background", "@view_bg_color"),
        ("__BORDER__", "muted", "@borders"),
        ("__ACCENT__", "accent", "@theme_selected_bg_color"),
        ("__MUTED__", "light_foreground", "@insensitive_fg_color"),
        ("__SELECTION__", "selection", "@theme_selected_bg_color"),
    ] {
        css = css.replace(placeholder, &theme_color(palette, key, fallback));
    }

    provider.load_from_data(&css);
}
