//! Throwaway: render the dashboard to an SVG screenshot for the README.

use mac_cleaner::app::App;
use mac_cleaner::config::Config;
use mac_cleaner::event::{DiskInfo, WorkerSender};
use mac_cleaner::model::{Category, MainView, SafetyTier, ScanItem};
use mac_cleaner::privilege::PrivilegeInfo;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use std::fmt::Write as _;
use std::path::PathBuf;

const COLS: u16 = 160;
const ROWS: u16 = 50;
const CW: f64 = 8.43;
const CH: f64 = 18.0;
const PAD: f64 = 24.0;
const BG: &str = "#12121a";
const DEFAULT_FG: &str = "#dcdce6";

fn hex(c: Color, fallback: &str) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => fallback.to_string(),
    }
}

fn item(label: &str, gb: f64, tier: SafetyTier, cat: Category, days: u32, note: &str) -> ScanItem {
    ScanItem::new(
        PathBuf::from(format!("/tmp/{label}")),
        label,
        (gb * 1_073_741_824.0) as u64,
        tier,
        cat,
    )
    .with_age(days)
    .with_note(note)
}

fn main() {
    unsafe { std::env::set_var("COLORTERM", "truecolor") };
    mac_cleaner::ui::theme::init();

    let privilege = PrivilegeInfo {
        is_root: false,
        limited: false,
        full_disk_access: true,
    };
    let mut app = App::new(
        Config::default(),
        privilege,
        WorkerSender::null(),
        Category::ALL.to_vec(),
        false,
    );
    app.disk = DiskInfo {
        total: 494 * 1_073_741_824,
        free: 102 * 1_073_741_824,
    };

    app.results.ingest(
        Category::Caches,
        vec![
            item(
                "Library/Caches/Google/Chrome",
                3.2,
                SafetyTier::Safe,
                Category::Caches,
                4,
                "browser re-creates this cache",
            ),
            item(
                "Library/Caches/pip",
                1.4,
                SafetyTier::Safe,
                Category::Caches,
                40,
                "pip re-downloads packages",
            ),
            item(
                "Library/Caches/Homebrew",
                0.9,
                SafetyTier::Safe,
                Category::Caches,
                12,
                "brew re-downloads bottles",
            ),
            item(
                "Library/Caches/com.spotify.client",
                0.6,
                SafetyTier::Safe,
                Category::Caches,
                2,
                "app re-creates this cache",
            ),
            item(
                "Library/Caches/JetBrains",
                0.2,
                SafetyTier::Moderate,
                Category::Caches,
                90,
                "IDE indexes rebuild on next open",
            ),
        ],
    );
    app.results.ingest(
        Category::DevArtifacts,
        vec![
            item(
                "Documents/proj/target",
                6.1,
                SafetyTier::Moderate,
                Category::DevArtifacts,
                30,
                "generated build artifact",
            ),
            item(
                "Library/Developer/Xcode/DerivedData",
                4.4,
                SafetyTier::Moderate,
                Category::DevArtifacts,
                200,
                "Xcode DerivedData — rebuildable",
            ),
            item(
                "Documents/web/node_modules",
                2.3,
                SafetyTier::Moderate,
                Category::DevArtifacts,
                120,
                "dependency folder — reinstall to restore",
            ),
        ],
    );
    app.results.ingest(
        Category::Duplicates,
        vec![item(
            "Downloads/installer (1).dmg",
            1.1,
            SafetyTier::Risky,
            Category::Duplicates,
            400,
            "duplicate of installer.dmg",
        )],
    );
    app.results.ingest(
        Category::Logs,
        vec![item(
            "Library/Logs/app.log",
            0.3,
            SafetyTier::Safe,
            Category::Logs,
            60,
            "old log file",
        )],
    );
    app.results.ingest(
        Category::ICloud,
        vec![item(
            "iCloud Drive/videos/talk.mp4",
            2.6,
            SafetyTier::Moderate,
            Category::ICloud,
            300,
            "evict — stays in iCloud",
        )],
    );
    app.results.ingest(
        Category::LargeFiles,
        vec![item(
            "Movies/screen-recording.mov",
            8.2,
            SafetyTier::Risky,
            Category::LargeFiles,
            500,
            "large file — review manually",
        )],
    );
    app.results.ingest(
        Category::Trash,
        vec![item(
            "old-download.zip",
            0.8,
            SafetyTier::Safe,
            Category::Trash,
            45,
            "already in Trash",
        )],
    );

    for (path, view, cursor) in [
        ("assets/dashboard.svg", MainView::Dashboard, 0),
        ("assets/detail.svg", MainView::Category(Category::Caches), 1),
    ] {
        app.view = view;
        app.selected_row = cursor;
        render_svg(&mut app, path);
    }
}

fn render_svg(app: &mut App, path: &str) {
    let backend = TestBackend::new(COLS, ROWS);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| mac_cleaner::ui::draw(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();

    let w = PAD * 2.0 + COLS as f64 * CW;
    let h = PAD * 2.0 + ROWS as f64 * CH;
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}">"#
    )
    .unwrap();
    writeln!(
        svg,
        r#"<rect width="100%" height="100%" rx="12" fill="{BG}"/>"#
    )
    .unwrap();

    // Background rects for cells whose bg differs from the canvas.
    for y in 0..ROWS {
        let mut x = 0u16;
        while x < COLS {
            let cell = &buffer[(x, y)];
            let bg = cell.style().bg.unwrap_or(Color::Reset);
            if matches!(bg, Color::Reset) {
                x += 1;
                continue;
            }
            let color = hex(bg, BG);
            let start = x;
            while x < COLS {
                let c = &buffer[(x, y)];
                if hex(c.style().bg.unwrap_or(Color::Reset), BG) != color {
                    break;
                }
                x += 1;
            }
            if color != BG {
                writeln!(
                    svg,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{CH}" fill="{color}"/>"#,
                    PAD + start as f64 * CW,
                    PAD + y as f64 * CH,
                    (x - start) as f64 * CW
                )
                .unwrap();
            }
        }
    }

    writeln!(
        svg,
        r#"<g font-family="Menlo, Monaco, 'DejaVu Sans Mono', monospace" font-size="14" xml:space="preserve">"#
    )
    .unwrap();

    for y in 0..ROWS {
        let mut x = 0u16;
        while x < COLS {
            let cell = &buffer[(x, y)];
            let style = cell.style();
            let fg = hex(style.fg.unwrap_or(Color::Reset), DEFAULT_FG);
            let bold = style.add_modifier.contains(Modifier::BOLD);
            let start = x;
            let mut text = String::new();
            while x < COLS {
                let c = &buffer[(x, y)];
                let s = c.style();
                if hex(s.fg.unwrap_or(Color::Reset), DEFAULT_FG) != fg
                    || s.add_modifier.contains(Modifier::BOLD) != bold
                {
                    break;
                }
                text.push_str(c.symbol());
                x += 1;
            }
            if text.trim().is_empty() {
                continue;
            }
            let escaped = text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let weight = if bold { r#" font-weight="bold""# } else { "" };
            writeln!(
                svg,
                r#"<text x="{:.1}" y="{:.1}" fill="{fg}"{weight} textLength="{:.1}">{escaped}</text>"#,
                PAD + start as f64 * CW,
                PAD + y as f64 * CH + CH - 4.5,
                text.chars().count() as f64 * CW
            )
            .unwrap();
        }
    }

    writeln!(svg, "</g></svg>").unwrap();
    std::fs::write(path, svg).unwrap();
    eprintln!("wrote {path}");
}
