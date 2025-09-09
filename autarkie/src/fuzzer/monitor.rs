use ratatui::{layout::Flex, style::Color};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    ops::Range,
    os::unix::thread,
    panic,
    sync::{Arc, RwLock},
    thread::spawn,
    time::{Duration, Instant},
};

use colored::Colorize;
use crossterm::{
    cursor::{EnableBlinking, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use libafl::monitors::{
    stats::{ClientStatsManager, EdgeCoverage},
    Monitor,
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    style::Style,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
    Frame, Terminal,
};
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
};

use super::context::MutationMetadata;

#[derive(Clone, Debug)]
struct AutarkieStatsContext {
    global_stats: Vec<String>,
    mutation_stats: Vec<String>,
    runtime: String,
    mutations: BTreeMap<MutationMetadata, usize>,
}

impl AutarkieStatsContext {
    pub fn new() -> Self {
        Self {
            global_stats: vec![],
            mutation_stats: vec![],
            runtime: String::new(),
            mutations: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AutarkieMonitor {
    context: Arc<RwLock<AutarkieStatsContext>>,
}

impl AutarkieMonitor {
    pub fn new() -> Self {
        let context = Arc::new(RwLock::new(AutarkieStatsContext::new()));
        enable_raw_mode().unwrap();
        let stdout = unsafe { libc::dup(std::io::stdout().as_raw_fd()) };
        let stdout = unsafe { File::from_raw_fd(stdout) };
        run_tui_thread(
            context.clone(),
            AutarkieUI::new(),
            Duration::from_millis(250),
            move || stdout.try_clone().unwrap(),
        );
        Self { context }
    }
}

impl Monitor for AutarkieMonitor {
    fn display(
        &mut self,
        client_stats_manager: &mut ClientStatsManager,
        event_msg: &str,
        sender_id: libafl_bolts::ClientId,
    ) -> Result<(), libafl::Error> {
        let mut ctx = self.context.write().unwrap();
        let clients_num = client_stats_manager.client_stats().len().clone();
        let cov = client_stats_manager
            .edges_coverage()
            .clone()
            .map_or(
                "0%".to_string(),
                |EdgeCoverage {
                     edges_hit,
                     edges_total,
                 }| {
                    format!(
                        "{:.4}% [{}/{}]",
                        edges_hit as f32 * 100.0 / edges_total as f32,
                        edges_hit,
                        edges_total
                    )
                },
            )
            .clone();
        let stats = client_stats_manager.global_stats();
        ctx.global_stats = vec![
            format!("runtime: {}", stats.run_time_pretty),
            format!("clients: {}", clients_num),
            format!(""),
            format!("testcases: {}", stats.corpus_size),
            format!("objectives: {}", stats.objective_size),
            format!(""),
            format!("execs/sec: {}", stats.execs_per_sec_pretty),
            format!("execs: {}", stats.total_execs),
            format!(""),
            format!("edges: {}", cov),
        ];
        ctx.mutation_stats = {
            let mut ret = vec![];
            for (k, v) in client_stats_manager.aggregated().iter() {
                if k != "edges" {
                    ret.push(format!("{}: {}", k, v));
                }
            }
            ret
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AutarkieUI {}

impl AutarkieUI {
    pub fn new() -> Self {
        Self {}
    }
    pub fn draw(&mut self, f: &mut Frame, app: &Arc<RwLock<AutarkieStatsContext>>) {
        let ctx = app.read().unwrap();
        let mut view_port_area = f.area();
        view_port_area.width = 100;
        view_port_area.height = 100;
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(5),
                Constraint::Length(50),
                Constraint::Fill(1),
            ])
            .split(view_port_area);
        // Create the main layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(2)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .flex(Flex::Center)
            .split(outer_layout[1]);

        let global_text: Text = ctx
            .global_stats
            .clone()
            .into_iter()
            .map(|line| Line::from(line.to_string()))
            .collect();

        let mutation_text: Text = ctx
            .mutation_stats
            .clone()
            .into_iter()
            .map(|line| Line::from(line.to_string()))
            .collect();

        let autarkie_para = Paragraph::new(vec![])
            .block(
                Block::default()
                    .title("Autarkie")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .style(Style::default().fg(Color::Yellow));

        let global_paragraph = Paragraph::new(global_text)
            .block(
                Block::default()
                    .title("Global Stats")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            )
            .style(Style::default().fg(Color::Yellow));

        // Create the Mutation Stats paragraph widget
        let mutation_paragraph = Paragraph::new(mutation_text)
            .block(
                Block::default()
                    .title("Mutation Stats")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            )
            .style(Style::default().fg(Color::Cyan));

        // Render both paragraphs
        f.render_widget(autarkie_para, outer_layout[0]);
        f.render_widget(global_paragraph, chunks[0]);
        f.render_widget(mutation_paragraph, chunks[1]);
    }
}

fn run_tui_thread<W: Write + Send + Sync + 'static>(
    context: Arc<RwLock<AutarkieStatsContext>>,
    ui: AutarkieUI,
    tick_rate: Duration,
    stdout_provider: impl Send + Sync + 'static + Fn() -> W,
) {
    spawn(move || -> std::io::Result<()> {
        let mut ui = ui;
        // setup terminal
        let mut stdout = stdout_provider();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut last_tick = Instant::now();
        let mut cnt = 0;

        // Catching panics when the main thread dies
        let old_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let mut stdout = stdout_provider();
            disable_raw_mode().unwrap();
            execute!(
                stdout,
                LeaveAlternateScreen,
                DisableMouseCapture,
                Show,
                EnableBlinking,
            )
            .unwrap();
            old_hook(panic_info);
        }));
        let mut should_quit = false;
        loop {
            // to avoid initial ui glitches
            if cnt < 8 {
                drop(terminal.clear());
                cnt += 1;
            }
            terminal.draw(|f| ui.draw(f, &context))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char(c) => should_quit = true,
                        _ => {}
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
            if should_quit {
                // restore terminal
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;

                println!(
                                    "\nPress Control-C to stop the fuzzers, otherwise press Enter to resume the visualization\n"
                                );

                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line)?;

                // setup terminal
                let mut stdout = std::io::stdout();
                enable_raw_mode()?;
                execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

                cnt = 0;
                should_quit = false;
            }
        }
    });
}
