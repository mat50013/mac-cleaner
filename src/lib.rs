pub mod app;
pub mod clean;
pub mod cli;
pub mod config;
pub mod event;
pub mod fs_util;
pub mod model;
pub mod privilege;
pub mod scan;
pub mod ui;

use crate::app::App;
use crate::cli::{Cli, Commands};
use crate::config::Config;
use crate::event::EventHandler;
use crate::model::Category;
use crate::privilege::maybe_elevate;
use anyhow::Result;
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::stdout;
use std::time::Duration;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;
    let auto_elevate = config.privilege.auto_elevate && !cli.no_elevate;
    let privilege = maybe_elevate(auto_elevate);

    match cli.command {
        None | Some(Commands::Tui) => run_tui(config, privilege, cli.dry_run)?,
        Some(Commands::Scan { json, categories }) => {
            run_headless_scan(&config, categories.as_deref(), json)?;
        }
        Some(Commands::Clean {
            categories,
            yes: _,
            permanent,
        }) => {
            run_headless_clean(&config, &categories, permanent, cli.dry_run)?;
        }
        Some(Commands::InitConfig) => {
            let path = config.save_default()?;
            println!("Wrote default config to {}", path.display());
        }
    }
    Ok(())
}

fn run_tui(config: Config, privilege: privilege::PrivilegeInfo, dry_run: bool) -> Result<()> {
    install_panic_hook();
    crate::ui::theme::init();
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    crate::ui::terminal::prepare(&mut terminal)?;

    let handler = EventHandler::new(Duration::from_millis(33));
    let worker = handler.sender();
    let categories: Vec<Category> = Category::ALL.to_vec();

    let mut app = App::new(config, privilege, worker, categories, dry_run);
    let result = app.run(&mut terminal, handler);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    crate::ui::terminal::restore();
    terminal.show_cursor()?;

    result?;
    Ok(())
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        crate::ui::terminal::restore();
        original(info);
    }));
}

fn run_headless_scan(config: &Config, cats: Option<&str>, json: bool) -> Result<()> {
    use crate::event::WorkerMsg;
    use crate::scan::{ScanContext, run_all};
    use crossbeam_channel::unbounded;
    use std::sync::Arc;

    let categories: Vec<Category> = cats
        .map(Cli::parse_categories)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| Category::ALL.to_vec());

    let (tx, rx) = unbounded();
    let worker = crate::event::WorkerSender::from_sender(tx.clone());
    let matchers = config.matchers()?;
    let ctx = ScanContext {
        config: Arc::new(config.clone()),
        matchers,
        tx: worker,
        categories: categories.clone(),
    };
    run_all(ctx);

    let mut results = model::ScanResults::new();
    let mut done = 0;
    while done < categories.len() {
        if let Ok(crate::event::Event::Worker(msg)) = rx.recv() {
            match msg {
                WorkerMsg::ScanDone { category, items } => {
                    results.ingest(category, items);
                    done += 1;
                }
                WorkerMsg::ScanSkipped { .. } => done += 1,
                _ => {}
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        for cat in &categories {
            println!("\n## {}", cat.title());
            for item in results.items_for(*cat) {
                println!(
                    "  {:>8}  {}  [{}]",
                    fs_util::human_size(item.real_bytes),
                    item.label,
                    item.tier.label()
                );
            }
        }
        println!(
            "\nTotal reclaimable: {}",
            fs_util::human_size(results.total_reclaimable())
        );
    }
    Ok(())
}

fn run_headless_clean(config: &Config, cats: &str, permanent: bool, dry_run: bool) -> Result<()> {
    use crate::clean::{CleanOptions, run_clean};
    use crate::config::DeleteMode;
    use crate::event::WorkerMsg;
    use crate::scan::{ScanContext, run_all};
    use crossbeam_channel::unbounded;
    use std::sync::Arc;

    let categories: Vec<Category> = Cli::parse_categories(cats);
    let (tx, rx) = unbounded();
    let worker = crate::event::WorkerSender::from_sender(tx.clone());
    let matchers = config.matchers()?;
    let ctx = ScanContext {
        config: Arc::new(config.clone()),
        matchers,
        tx: worker,
        categories: categories.clone(),
    };
    run_all(ctx);

    let mut results = model::ScanResults::new();
    let mut done = 0;
    while done < categories.len() {
        if let Ok(crate::event::Event::Worker(msg)) = rx.recv() {
            match msg {
                WorkerMsg::ScanDone { category, items } => {
                    results.ingest(category, items);
                    done += 1;
                }
                WorkerMsg::ScanSkipped { .. } => done += 1,
                _ => {}
            }
        }
    }

    let selected: Vec<_> = results
        .items
        .values()
        .flat_map(|v| v.iter())
        .filter(|i| i.selected && i.selectable())
        .cloned()
        .collect();

    if selected.is_empty() {
        println!("No safe items selected to clean.");
        return Ok(());
    }

    let mode = if permanent {
        DeleteMode::Permanent
    } else {
        config.delete_mode
    };

    let (clean_tx, clean_rx) = unbounded();
    let clean_worker = crate::event::WorkerSender::from_sender(clean_tx);
    run_clean(
        selected,
        CleanOptions {
            permanent,
            dry_run,
            mode,
        },
        clean_worker,
    );

    while let Ok(crate::event::Event::Worker(msg)) = clean_rx.recv() {
        if matches!(msg, WorkerMsg::CleanDone { .. }) {
            if let WorkerMsg::CleanDone { freed, failures } = msg {
                println!("Freed {}", fs_util::human_size(freed));
                for f in failures {
                    eprintln!("  error: {f}");
                }
            }
            break;
        }
    }
    Ok(())
}
