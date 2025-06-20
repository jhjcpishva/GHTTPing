use std::time::{Duration, Instant};
use std::io::{stdout, Result};
use std::sync::{Arc, Mutex};
use clap::Parser;
use tokio::sync::mpsc::{unbounded_channel};
use tokio::time::sleep;
use reqwest::Client;
use crossterm::{execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode, disable_raw_mode}};
use crossterm::event::{self, Event as CEvent, KeyCode};
use ratatui::{prelude::*, widgets::{Axis, Block, Borders, Chart, Dataset}};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Interval between requests in seconds
    #[arg(short, long, default_value = "1")]
    interval: f64,

    /// URLs to ping
    urls: Vec<String>,
}

struct SharedData {
    samples: Vec<Vec<f64>>, // per host
    max_samples: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.urls.is_empty() {
        eprintln!("Please provide at least one URL");
        std::process::exit(1);
    }

    let interval = Duration::from_secs_f64(args.interval);
    let max_samples = 60usize;
    let data = Arc::new(Mutex::new(SharedData {
        samples: vec![Vec::new(); args.urls.len()],
        max_samples,
    }));

    let (tx, mut rx) = unbounded_channel::<(usize, Option<f64>)>();

    for (idx, url) in args.urls.iter().cloned().enumerate() {
        let tx = tx.clone();
        let url_clone = url.clone();
        tokio::spawn(async move {
            let client = Client::new();
            loop {
                let start = Instant::now();
                let res = client.get(&url_clone).send().await;
                let elapsed = match res {
                    Ok(_) => Some(start.elapsed().as_secs_f64() * 1000.0),
                    Err(_) => None,
                };
                if tx.send((idx, elapsed)).is_err() {
                    break;
                }
                sleep(interval).await;
            }
        });
    }
    drop(tx);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        while let Ok((idx, val)) = rx.try_recv() {
            let mut d = data.lock().unwrap();
            if let Some(v) = val { d.samples[idx].push(v); } else { d.samples[idx].push(f64::NAN); }
            if d.samples[idx].len() > d.max_samples { d.samples[idx].remove(0); }
        }

        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::vertical(
                args.urls.iter().map(|_| Constraint::Length(size.height / args.urls.len() as u16)).collect::<Vec<_>>()
            ).split(size);

            let d = data.lock().unwrap();
            for (i, area) in chunks.iter().enumerate() {
                let points: Vec<(f64, f64)> = d.samples[i]
                    .iter()
                    .enumerate()
                    .map(|(x, y)| (x as f64, *y))
                    .collect();
                let max_y = d.samples[i]
                    .iter()
                    .fold(0.0_f64, |a, b| if b.is_nan() { a } else { a.max(*b) });
                let dataset = Dataset::default()
                    .name(args.urls[i].as_str())
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::Cyan))
                    .data(&points);
                let chart = Chart::new(vec![dataset])
                    .block(Block::default().borders(Borders::ALL).title(args.urls[i].as_str()))
                    .x_axis(Axis::default().bounds([0.0, d.max_samples as f64]))
                    .y_axis(Axis::default().bounds([0.0, (max_y + 10.0).max(100.0)]));
                f.render_widget(chart, *area);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let CEvent::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    Ok(())
}
