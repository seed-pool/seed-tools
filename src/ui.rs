// --- External Crates ---
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::layout::Rect;
use walkdir::WalkDir;
use simplelog::*;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use notify::{Config as NotifyConfig, Watcher, RecursiveMode, RecommendedWatcher, Event as NotifyEvent, EventKind};
use serde::Deserialize;
// --- Standard Library ---
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, Once},
    thread,
    time::Duration,
};
use vte::{Parser, Perform};
use crate::types::PreflightCheckResult;
use crate::utils;
use std::fs::OpenOptions;
// --- Static Variables ---
static INIT_LOGGER: Once = Once::new();
#[derive(Deserialize)]
struct GeneralConfig {
    tmdb_api_key: String,
}

#[derive(Deserialize)]
struct PathsConfig {
    mediainfo: String,
    torrent_dir: String,
    screenshots_dir: String,
    ffmpeg: String,
    ffprobe: String,
    mkbrr: String,
}

#[derive(Deserialize)]
struct AppConfig {
    general: GeneralConfig,
    paths: PathsConfig,
}

fn load_config() -> AppConfig {
    serde_yaml::from_str(&std::fs::read_to_string("config/config.yaml").expect("Failed to read config file"))
        .expect("Failed to parse YAML config")
}
// --- Enum Definitions ---
/// Enum to wrap different widget types for rendering.
enum UIContent<'a> {
    List(List<'a>),
    Paragraph(Paragraph<'a>),
}

impl<'a> UIContent<'a> {
    /// Renders the UIContent (List or Paragraph) in the specified area.
    fn render(self, f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>, area: tui::layout::Rect) {
        match self {
            UIContent::List(list) => f.render_widget(list, area),
            UIContent::Paragraph(paragraph) => f.render_widget(paragraph, area),
        }
    }
}

struct TerminalEmulator {
    buffer: Arc<Mutex<Vec<String>>>,
}

impl TerminalEmulator {
    fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn feed(&self, data: &str) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push(data.to_string());
        if buffer.len() > 100 {
            buffer.remove(0); // Keep the buffer size manageable
        }
    }

    fn render(&self) -> Vec<String> {
        let buffer = self.buffer.lock().unwrap();
        buffer.clone()
    }
}

pub fn launch_ui() -> Result<(), Box<dyn std::error::Error>> {
    // Set up a panic hook to restore the terminal state on panic
    let original_hook = std::panic::take_hook();
    let config = load_config();

    // Extract the TMDB API key and mediainfo path
    let tmdb_api_key = config.general.tmdb_api_key;
    let mediainfo_path = config.paths.mediainfo.clone();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Enable raw mode and set up the terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize state variables
    let mut current_dir = std::env::current_dir()?;
    let mut file_list = get_files_in_dir(&current_dir);
    let mut selected_file_index = 0;
    let mut scroll_offset = 0;
    let mut tracker_scroll_offset = 0;
    let mut selected_trackers = Vec::<String>::new();
    let mut input_path = None::<PathBuf>;
    let mut exit_requested = false;
    let mut showing_log = false; // Flag to indicate if we're showing the log

    let tracker_options = vec!["✔️ Select All", "🐳 seedpool [SP]", "🐛 TorrentLeech [TL]"];
    let log_output = Arc::new(Mutex::new(Vec::<String>::new()));
    let log_scroll_offset = Arc::new(Mutex::new(0)); // Shared scroll offset for logs
    let mut preflight_check_result: Option<PreflightCheckResult> = None;
    let mut upload_running = false; // Tracks if the upload process is running
    let mut preflight_check_running = false;
    let terminal_emulator = Arc::new(TerminalEmulator::new());
    let log_file_path = "seed-tools.log";
    start_log_tail(Arc::clone(&terminal_emulator), log_file_path);
    // Channel for notifying the main loop of log updates
    let (tx, rx) = mpsc::channel::<()>();
    let mut terminal_scroll_offset = 0; 
    // Initial UI render
    terminal.draw(|f| {
        render_ui(
            f,
            &input_path,
            &selected_trackers,
            &file_list,
            selected_file_index,
            scroll_offset,
            tracker_scroll_offset,
            &tracker_options,
            showing_log,
            &terminal_emulator, // Pass the terminal emulator for logs
            &log_scroll_offset, // Add the missing argument
            &preflight_check_result,
            upload_running,
            preflight_check_running,
        );
    })?;

    // Main loop
    loop {
        if exit_requested {
            break;
        }

        // Check for log updates and redraw the UI if necessary
        if let Ok(_) = rx.try_recv() {
            terminal.draw(|f| {
                render_ui(
                    f,
                    &input_path,
                    &selected_trackers,
                    &file_list,
                    selected_file_index,
                    scroll_offset,
                    tracker_scroll_offset,
                    &tracker_options,
                    showing_log,
                    &terminal_emulator, // Pass the terminal emulator for logs
                    &log_scroll_offset, // Add the missing argument
                    &preflight_check_result,
                    upload_running,
                    preflight_check_running,
                );
            })?;
        }

        if let Event::Mouse(mouse_event) = event::read()? {
            match mouse_event.kind {
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    let y = mouse_event.row.saturating_sub(1); // Adjust for offset
                    let x = mouse_event.column;
        
                    // Define layout for click handling
                    let size = terminal.size()?;
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(5),  // Top section (Status + Buttons)
                            Constraint::Length(1),  // Section for "Files" and "Logs" buttons
                            Constraint::Min(1),     // Middle section (File List or Terminal + Tracker List)
                            Constraint::Length(5),  // Pre-flight Check section
                            Constraint::Length(3),  // Bottom section (Quit message)
                        ])
                        .split(size);
        
                    let top_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(80), // Status section
                            Constraint::Percentage(20), // Button section
                        ])
                        .split(chunks[0]);
        
                    let middle_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(80), // File List or Terminal content
                            Constraint::Percentage(20), // Tracker List
                        ])
                        .split(chunks[2]);
        
                    let files_logs_section = chunks[1]; // Section for "Files" and "Logs" buttons
                    let buttons_y = files_logs_section.y -1; // Fixed Y position for the buttons
        
                    // Define the X ranges for the buttons
                    let files_button_start_x = files_logs_section.x + 2; // Start X position of "🖥️ Files" button
                    let files_button_end_x = files_button_start_x + 5;  // End X position of "🖥️ Files" button
                    let logs_button_start_x = files_button_end_x + 5;   // Start X position of "📃 Logs" button
                    let logs_button_end_x = logs_button_start_x + 8;    // End X position of "📃 Logs" button
        
                    // Handle "Files" and "Logs" button clicks
                    if y == buttons_y {
                        if x >= files_button_start_x && x < files_button_end_x {
                            // "Files" button clicked
                            showing_log = false;
                        } else if x >= logs_button_start_x && x < logs_button_end_x {
                            // "Logs" button clicked
                            showing_log = true;
        
                            // Start tailing the log file in the terminal emulator
                            let log_file_path = "seed-tools.log";
                            start_log_tail(Arc::clone(&terminal_emulator), log_file_path);
                        }
                    }
        
                    // Handle button clicks in the top section
                    if x >= top_chunks[1].x && x < top_chunks[1].x + top_chunks[1].width && y >= top_chunks[1].y && y < top_chunks[1].y + top_chunks[1].height {
                        let relative_y = y - top_chunks[1].y;
                        if relative_y == 0 {
                            // Upload button clicked
                            if input_path.is_some() && !selected_trackers.is_empty() {
                                showing_log = true; // Switch to log view
                                upload_running = true; // Set spinner state to true
        
                                // Start tailing the log file in the terminal emulator
                                let log_file_path = "seed-tools.log";
                                start_log_tail(Arc::clone(&terminal_emulator), log_file_path);
        
                                // Start the upload process in a separate thread
                                let input_path = input_path.clone();
                                let selected_trackers = selected_trackers.clone();
                                thread::spawn({
                                    let log_output = Arc::clone(&log_output);
                                    move || {
                                        let _ = activate_upload(
                                            &input_path,
                                            &selected_trackers,
                                            &None,
                                            log_output,
                                        );
        
                                        // Reset spinner state and notify the main loop
                                        upload_running = false;
                                    }
                                });
                            } else {
                                log_output.lock().unwrap().push("Error: Input path or trackers not selected.".to_string());
                            }
                        } else if relative_y == 1 {
                            if let Some(input_path) = &input_path {
                                let input_path = input_path.clone();
                                let log_output = Arc::clone(&log_output);
        
                                thread::spawn(move || {
                                    log_output.lock().unwrap().push("Running Pre-flight Check...".to_string());
        
                                    // Define the pre-flight log file path
                                    let preflight_log_path = PathBuf::from("pre-flight.log");
        
                                    // Run the seed-tools command with --pre and redirect output to pre-flight.log
                                    let status = Command::new("./seed-tools")
                                        .arg("--pre")
                                        .arg(input_path.display().to_string())
                                        .stdout(Stdio::from(
                                            File::create(&preflight_log_path).expect("Failed to create pre-flight.log"),
                                        ))
                                        .stderr(Stdio::from(
                                            File::create(&preflight_log_path).expect("Failed to create pre-flight.log"),
                                        ))
                                        .status();
        
                                    match status {
                                        Ok(status) if status.success() => {
                                            log_output.lock().unwrap().push("Pre-flight Check completed.".to_string());
                                        }
                                        Ok(status) => {
                                            log_output.lock().unwrap().push(format!(
                                                "Pre-flight Check failed with exit code: {}",
                                                status.code().unwrap_or(-1)
                                            ));
                                        }
                                        Err(err) => {
                                            log_output.lock().unwrap().push(format!("Failed to run Pre-flight Check: {}", err));
                                        }
                                    }
                                });
                            } else {
                                log_output.lock().unwrap().push("Error: No input path selected.".to_string());
                            }
                        }
                    }
        
                    // Handle tracker list clicks
                    if x >= middle_chunks[1].x && x < middle_chunks[1].x + middle_chunks[1].width && y >= middle_chunks[1].y && y < middle_chunks[1].y + middle_chunks[1].height {
                        let relative_y = y - middle_chunks[1].y;
                        let clicked_index = tracker_scroll_offset + relative_y as usize;
                        if clicked_index < tracker_options.len() {
                            let tracker = tracker_options[clicked_index].to_string();
                            if tracker == "✔️ Select All" {
                                if selected_trackers.len() == tracker_options.len() - 1 {
                                    selected_trackers.clear(); // Deselect all trackers
                                } else {
                                    selected_trackers = tracker_options[1..]
                                        .iter()
                                        .map(|&t| t.to_string())
                                        .collect(); // Select all trackers
                                }
                            } else if selected_trackers.contains(&tracker) {
                                selected_trackers.retain(|t| t != &tracker); // Deselect the clicked tracker
                            } else {
                                selected_trackers.push(tracker); // Select the clicked tracker
                            }
                        }
                    }
        
                    // Handle file list clicks
                    if !showing_log && x < middle_chunks[0].x + middle_chunks[0].width && y >= middle_chunks[0].y && y < middle_chunks[0].y + middle_chunks[0].height {
                        let relative_y = y - middle_chunks[0].y;
                        let clicked_index = scroll_offset + relative_y as usize;
                        if clicked_index < file_list.len() {
                            selected_file_index = clicked_index;
                            let selected_path = current_dir.join(&file_list[selected_file_index]);
                            if file_list[selected_file_index] == "🗂️ .." {
                                if let Some(parent) = current_dir.parent() {
                                    current_dir = parent.to_path_buf();
                                    file_list = get_files_in_dir(&current_dir);
                                    selected_file_index = 0;
                                    scroll_offset = 0;
                                }
                            } else if selected_path.is_dir() {
                                current_dir = selected_path.clone();
                                file_list = get_files_in_dir(&current_dir);
                                selected_file_index = 0;
                                scroll_offset = 0;
                                input_path = Some(selected_path); // Set as input path
                            } else if selected_path.is_file() {
                                input_path = Some(selected_path);
                            }
                        }
                    }
        
                    // Redraw the UI after handling a click
                    terminal.draw(|f| {
                        render_ui(
                            f,
                            &input_path,
                            &selected_trackers,
                            &file_list,
                            selected_file_index,
                            scroll_offset,
                            tracker_scroll_offset,
                            &tracker_options,
                            showing_log,
                            &terminal_emulator, // Pass the terminal emulator for logs
                            &log_scroll_offset, // Add the missing argument
                            &preflight_check_result,
                            upload_running,
                            preflight_check_running,
                        );
                    })?;
                }
                crossterm::event::MouseEventKind::ScrollUp => {
                    if showing_log {
                        if terminal_scroll_offset > 0 {
                            terminal_scroll_offset -= 1; // Scroll up in the terminal window
                        }
                    } else if scroll_offset > 0 {
                        scroll_offset -= 1; // Scroll up in the file list
                    }
                }
                crossterm::event::MouseEventKind::ScrollDown => {
                    if showing_log {
                        let terminal_output = terminal_emulator.render();
                        if terminal_scroll_offset + 1 < terminal_output.len() {
                            terminal_scroll_offset += 1; // Scroll down in the terminal window
                        }
                    } else if scroll_offset + 1 < file_list.len() {
                        scroll_offset += 1; // Scroll down in the file list
                    }
                }
                _ => {}
            }
        
            // Redraw the UI after handling scroll events
            terminal.draw(|f| {
                render_ui(
                    f,
                    &input_path,
                    &selected_trackers,
                    &file_list,
                    selected_file_index,
                    scroll_offset,
                    tracker_scroll_offset,
                    &tracker_options,
                    showing_log,
                    &terminal_emulator, // Pass the terminal emulator for logs
                    &log_scroll_offset, // Add the missing argument
                    &preflight_check_result,
                    upload_running,
                    preflight_check_running,
                );
            })?;
        } else if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => {
                    exit_requested = true;
                }
                _ => {}
            }
        }
    }

    // Restore the terminal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn render_ui(
    f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>,
    input_path: &Option<PathBuf>,
    selected_trackers: &Vec<String>,
    file_list: &Vec<String>,
    selected_file_index: usize,
    scroll_offset: usize,
    tracker_scroll_offset: usize,
    tracker_options: &[&str],
    showing_log: bool,
    terminal_emulator: &Arc<TerminalEmulator>, // Pass terminal_emulator instead of log_output
    log_scroll_offset: &Arc<Mutex<usize>>,
    preflight_check_result: &Option<PreflightCheckResult>,
    upload_running: bool,
    preflight_check_running: bool,
) {
    // Define the layout
    let size = f.size();

    // Render a full-screen block with the background color
    let background_block = Block::default().style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color
    f.render_widget(background_block, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Top section (Status + Buttons)
            Constraint::Length(1),  // Section for "Files" and "Logs" buttons
            Constraint::Min(1),     // Middle section (File List + Tracker or Log Output)
            Constraint::Length(6),  // Pre-flight Check section
            Constraint::Length(3),  // Bottom section (Quit message)
        ])
        .split(size);

    // Split the top section into Status and Buttons
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80), // Status section
            Constraint::Percentage(20), // Button section
        ])
        .split(chunks[0]);

    // Split the middle section into File List and Tracker List or Log Output
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80), // File List or Log content
            Constraint::Percentage(20), // Tracker List
        ])
        .split(chunks[2]);

    // Render Status Section
    let mut status_lines = Vec::new();

    // Input Path
    if let Some(path) = input_path {
        if let Some(file_name) = path.file_name() {
            status_lines.push(Spans::from(vec![
                Span::styled(
                    "Input Path: ",
                    Style::default().fg(Color::DarkGray), // DarkGray for the label
                ),
                Span::styled(
                    file_name.to_string_lossy(),
                    Style::default().fg(Color::Green), // Green for the value
                ),
            ]));
        } else {
            status_lines.push(Spans::from(vec![
                Span::styled(
                    "Input Path: ",
                    Style::default().fg(Color::DarkGray), // DarkGray for the label
                ),
                Span::styled(
                    "Invalid path",
                    Style::default().fg(Color::Red), // Red for invalid path
                ),
            ]));
        }
    } else {
        status_lines.push(Spans::from(vec![
            Span::styled(
                "Input Path: ",
                Style::default().fg(Color::DarkGray), // DarkGray for the label
            ),
            Span::styled(
                "❌ None selected",
                Style::default().fg(Color::DarkGray), // DarkGray for no selection
            ),
        ]));
    }
    
    // Selected Trackers
    if selected_trackers.is_empty() {
        status_lines.push(Spans::from(vec![
            Span::styled(
                "Trackers: ",
                Style::default().fg(Color::DarkGray), // DarkGray for the label
            ),
            Span::styled(
                "❌ None selected",
                Style::default().fg(Color::DarkGray), // DarkGray for no selection
            ),
        ]));
    } else {
        status_lines.push(Spans::from(vec![
            Span::styled(
                "Trackers: ",
                Style::default().fg(Color::DarkGray), // DarkGray for the label
            ),
            Span::styled(
                selected_trackers.join(", "),
                Style::default().fg(Color::LightCyan), // LightCyan for the value
            ),
        ]));
    }
    
    // Render the status section in `top_chunks[0]`
    let status_paragraph = Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::ALL).title(" 🌀 Seed-Tools v0.42 "))
        .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color
    f.render_widget(status_paragraph, top_chunks[0]);
    
    // Render Button Section
    let button_lines = vec![
        Spans::from(vec![Span::styled(
            "🔺  ＵＰＬＯＡＤ ", // Upload button text
            Style::default()
                .fg(Color::White) // Text color
                .bg(Color::Red) // Background color
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::styled(
            "✅ ＰＲＥ-ＦＬＩＧＨＴ", // Pre-flight Check button text
            Style::default()
                .fg(Color::White) // Text color
                .bg(Color::Green) // Background color
                .add_modifier(Modifier::BOLD),
        )]),
    ];

    let button_paragraph = Paragraph::new(button_lines)
        .block(Block::default().borders(Borders::ALL).title(" 🕹️ Actions "))
        .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color

    f.render_widget(button_paragraph, top_chunks[1]);


    // Render "Files" and "Logs" Buttons Section
    let files_logs_spans = Spans::from(vec![
        Span::styled(
            " 🖥️ Files",
            Style::default()
                .fg(if !showing_log { Color::Yellow } else { Color::White })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "), // Add spacing between buttons
        Span::styled(
            " 📃 Logs",
            Style::default()
                .fg(if showing_log { Color::Yellow } else { Color::White })
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let files_logs_paragraph = Paragraph::new(files_logs_spans)
        .alignment(tui::layout::Alignment::Left) // Align to the left
        .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color

    // Render the buttons section in chunks[1]
    f.render_widget(files_logs_paragraph, chunks[1]);

    // Render File List or Log Section
    if showing_log {
        // Render the terminal emulator
        let mut terminal_scroll_offset = 0; 
    let terminal_output = terminal_emulator.render();
    let visible_lines = terminal_output
        .iter()
        .skip(terminal_scroll_offset) // Skip lines based on the scroll offset
        .take(middle_chunks[0].height as usize) // Take only the visible lines
        .map(|line| Spans::from(Span::raw(line.clone())))
        .collect::<Vec<_>>();

    let terminal_widget = Paragraph::new(visible_lines)
        .block(Block::default().borders(Borders::ALL)) // Remove the title
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(terminal_widget, middle_chunks[0]);
    } else {
        // Render the file list
        let mut visible_files = vec!["🗂️ ..".to_string()];
        visible_files.extend(
            file_list[1..]
                .iter()
                .skip(scroll_offset)
                .take((middle_chunks[0].height as usize).saturating_sub(1)) // Subtract 1 for the ".." entry
                .cloned(),
        );

        let file_list_widget = List::new(
            visible_files
                .iter()
                .enumerate()
                .map(|(i, file)| {
                    let style = if i == selected_file_index {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Span::styled(file, style))
                })
                .collect::<Vec<_>>(),
        )
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color
        f.render_widget(file_list_widget, middle_chunks[0]);
    }

    // Render Tracker List Section
    let visible_trackers = &tracker_options[tracker_scroll_offset
        ..(tracker_scroll_offset + middle_chunks[1].height as usize).min(tracker_options.len())];
    let tracker_list_widget = List::new(
        visible_trackers.iter().enumerate().map(|(i, tracker)| {
            let is_selected = selected_trackers.contains(&tracker.to_string());
            let tracker_name = if is_selected {
                format!("{} ✔️", tracker) // Append ✔️ to selected trackers
            } else {
                tracker.to_string()
            };

            // Split the tracker name into styled parts
            let styled_tracker_name = if tracker.contains("🆂") {
                Spans::from(vec![
                    Span::styled("🆂", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)), // Blue for 🆂🅿
                    Span::raw(tracker_name[4..].to_string()), // Clone the rest of the line
                ])
            } else if tracker.contains("🆃") {
                Spans::from(vec![
                    Span::styled("🆃", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)), // Green for 🆃🅻
                    Span::raw(tracker_name[4..].to_string()), // Clone the rest of the line
                ])
            } else {
                Spans::from(vec![Span::raw(tracker_name)]) // Default style for other trackers
            };

            ListItem::new(styled_tracker_name)
        }).collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::ALL).title("🌐 Trackers "))
    .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color
    f.render_widget(tracker_list_widget, middle_chunks[1]);

    // Render Pre-flight Check Section
    let mut preflight_lines = Vec::new();
    if let Some(preflight_log_path) = Some(PathBuf::from("pre-flight.log")) {
        if preflight_log_path.exists() {
            let (log_data, is_pending) = parse_preflight_log(&preflight_log_path);
    
            if is_pending {
                // Display hourglass emoji for all fields
                preflight_lines.push(Spans::from(vec![Span::styled(
                    "⏳ Running Pre-flight Check ...",
                    Style::default().fg(Color::Yellow),
                )]));
            } else {
                // Line 1: Title, Release Type, Audio Languages
                preflight_lines.push(Spans::from(vec![
                    // Title
                    Span::styled(
                        "Title: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    Span::styled(
                        log_data[0].replace("Title: ", ""),
                        Style::default().fg(Color::Yellow), // Yellow for the value
                    ),
                    Span::raw(" | "),
                    // Release Type
                    Span::styled(
                        "Type: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    {
                        let release_type = log_data[3].replace("Release Type: ", ""); // Store the result of `replace`
                        if release_type.contains("★") {
                            let (before_star, after_star) = release_type.split_once("★").unwrap_or(("", ""));
                            Span::styled(
                                format!(
                                    "{}★{}",
                                    before_star.trim(),
                                    after_star.trim()
                                ),
                                Style::default().fg(Color::Cyan), // Cyan for the text
                            )
                        } else {
                            Span::styled(
                                release_type,
                                Style::default().fg(Color::Cyan), // Cyan for the value
                            )
                        }
                    },
                    Span::raw(" | "),
                    // Audio Languages
                    Span::styled(
                        "Audio: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    Span::styled(
                        log_data[11].replace("Audio Languages: ", ""),
                        Style::default().fg(Color::LightMagenta), // Magenta for the value
                    ),
                ]));
    
                // Line 2: TMDB, IMDb, TVDB IDs, Season/Episode Numbers
                preflight_lines.push(Spans::from(vec![
                    // TMDB ID
                    Span::styled(
                        "TMDB: ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        log_data[6].replace("TMDB ID: ", ""),
                        Style::default().fg(Color::Cyan), // Turquoise for the value
                    ),
                    Span::raw(" | "),
                    // IMDb ID
                    Span::styled(
                        "IMDb: ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        log_data[7].replace("IMDb ID: ", ""),
                        Style::default().fg(Color::Cyan), // Turquoise for the value
                    ),
                    Span::raw(" | "),
                    // TVDB ID
                    Span::styled(
                        "TVDB: ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        log_data[8].replace("TVDB ID: ", ""),
                        Style::default().fg(Color::Cyan), // Turquoise for the value
                    ),
                    Span::raw(" | "),
                    // Season and Episode Numbers
                    Span::styled(
                        "Season: ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        log_data[4].replace("Season Number: ", ""),
                        Style::default().fg(Color::Cyan), // Turquoise for the value
                    ),
                    Span::raw(" "),
                    Span::styled(
                        "Episode: ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        log_data[5].replace("Episode Number: ", ""),
                        Style::default().fg(Color::Cyan), // Turquoise for the value
                    ),
                ]));
    
                // Line 3: Release Name
                preflight_lines.push(Spans::from(vec![
                    // Label: "Release Name:"
                    Span::styled(
                        "Release Name: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    // Value: The actual release name
                    Span::styled(
                        log_data[1].replace("Release Name: ", ""),
                        Style::default().fg(Color::Rgb(255, 153, 51)), // Vibrant orange for the value
                    ),
                ]));
    
                // Line 4: Dupe Check, Strip From Videos, Album Cover
                preflight_lines.push(Spans::from(vec![
                    // Dupe Check
                    Span::styled(
                        "Dupe Check: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    Span::styled(
                        if log_data[2].contains("N/A") {
                            "N/A" // Display N/A for music preflight checks
                        } else if log_data[2].contains("PASS") {
                            "✔️ PASS"
                        } else {
                            "❌ FAIL"
                        },
                        Style::default().fg(if log_data[2].contains("N/A") {
                            Color::DarkGray // DarkGray for N/A
                        } else if log_data[2].contains("PASS") {
                            Color::Green // Green for PASS
                        } else {
                            Color::Red // Red for FAIL
                        }),
                    ),
                    Span::raw(" | "),
                    // Strip From Videos (Excluded Files)
                    Span::styled(
                        "Stripshit From Videos: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    Span::styled(
                        if log_data[10].contains("N/A") {
                            "N/A" // Display N/A for music preflight checks
                        } else if log_data[10].contains("Enabled") {
                            "✔️ Enabled"
                        } else if log_data[10].contains("Disabled") {
                            "❌ Disabled"
                        } else {
                            "N/A"
                        },
                        Style::default().fg(if log_data[10].contains("N/A") {
                            Color::DarkGray // DarkGray for N/A
                        } else if log_data[10].contains("Enabled") {
                            Color::Green // Green for Enabled
                        } else if log_data[10].contains("Disabled") {
                            Color::Red // Red for Disabled
                        } else {
                            Color::DarkGray // DarkGray for N/A
                        }),
                    ),
                    Span::raw(" | "),
                    // Album Cover
                    Span::styled(
                        "Album Cover: ",
                        Style::default().fg(Color::DarkGray), // DarkGray for the label
                    ),
                    Span::styled(
                        if log_data[9].contains("Available") {
                            "✔️ Available"
                        } else if log_data[9].contains("Not Found") {
                            "❌ Not Found"
                        } else {
                            "N/A"
                        },
                        Style::default().fg(if log_data[9].contains("Available") {
                            Color::Green // Green for Available
                        } else if log_data[9].contains("Not Found") {
                            Color::Red // Red for Not Found
                        } else {
                            Color::DarkGray // DarkGray for N/A
                        }),
                    ),
                ]));
            }
        } else {
            preflight_lines.push(Spans::from(Span::styled(
                "Pre-flight Check: No results available",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    
    let preflight_paragraph = Paragraph::new(preflight_lines)
        .block(Block::default().borders(Borders::ALL).title(" ✅ Pre-flight Check "))
        .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color
    f.render_widget(preflight_paragraph, chunks[3]);

    // Render Bottom Section
    let bottom_lines = vec![Spans::from(vec![Span::styled(
        "Spam [ESC] to Quit ❌",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )])];
    let bottom_paragraph = Paragraph::new(bottom_lines)
        .block(Block::default().borders(Borders::ALL).title(" ⌨  Keys "))
        .alignment(tui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(8, 8, 32))); // Background color
    f.render_widget(bottom_paragraph, chunks[4]);
}

fn activate_upload(
    input_path: &Option<PathBuf>,
    selected_trackers: &Vec<String>,
    custom_category_type: &Option<String>,
    log_output: Arc<Mutex<Vec<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if input_path.is_none() {
        log_output.lock().unwrap().push("Error: No input path selected.".to_string());
        return Err("Error: No input path selected.".into());
    }

    if selected_trackers.is_empty() {
        log_output.lock().unwrap().push("Error: No trackers selected.".to_string());
        return Err("Error: No trackers selected.".into());
    }

    let log_file_path = Path::new("seed-tools.log");
    File::create(log_file_path)?; // Open in write mode to truncate the file
    log_output.lock().unwrap().push("Cleared seed-tools.log for fresh logs.".to_string());

    let input_path = input_path.as_ref().unwrap();
    let mut args = vec![input_path.display().to_string()];

    for tracker in selected_trackers {
        match tracker.as_str() {
            "🐳 seedpool [SP]" => args.push("--SP".to_string()),
            "🐛 TorrentLeech [TL]" => args.push("--TL".to_string()),
            _ => {}
        }
    }

    if let Some(category) = custom_category_type {
        args.push("--custom-cat-type".to_string());
        args.push(category.clone());
    }

    // Specify the full path to seed-tools
    let seed_tools_path = std::env::current_dir()?
        .join("seed-tools"); // Adjust the relative path as needed
    log_output.lock().unwrap().push(format!("Using seed-tools path: {:?}", seed_tools_path));

    // Start the seed-tools process with piped stdout and stderr
    let mut child = Command::new(seed_tools_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Spawn a thread to read stdout
    let log_output_clone = Arc::clone(&log_output);
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                log_output_clone.lock().unwrap().push(line);
            }
        }
    });

    // Spawn a thread to read stderr
    let log_output_clone = Arc::clone(&log_output);
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                log_output_clone.lock().unwrap().push(format!("ERROR: {}", line));
            }
        }
    });

    // Wait for the process to complete
    let status = child.wait()?;
    if status.success() {
        log_output.lock().unwrap().push("Upload completed successfully.".to_string());
    } else {
        log_output.lock().unwrap().push(format!(
            "Upload failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    // Ensure threads finish processing
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    Ok(())
}

fn help_message(on_main_screen: bool, in_tracker_selection: bool) -> String {
    if in_tracker_selection {
        "Use UP/DOWN to navigate, F to toggle trackers, ENTER to confirm.".to_string()
    } else if on_main_screen {
        "Press F to select input path, C to set category, U to upload.".to_string()
    } else {
        "Use UP/DOWN to navigate, F to select, ENTER to confirm.".to_string()
    }
}

fn get_files_in_dir(dir: &Path) -> Vec<String> {
    let mut visible_entries: Vec<String> = Vec::new();
    let mut hidden_entries: Vec<String> = Vec::new();

    for entry in WalkDir::new(dir).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        let file_name = entry.file_name().to_string_lossy().to_string();

        if entry.path() == dir {
            continue; // Skip the current directory itself
        }

        if file_name.starts_with('.') {
            // Add hidden files and folders to the hidden list
            if entry.path().is_dir() {
                hidden_entries.push(format!("{}/", file_name));
            } else {
                hidden_entries.push(file_name);
            }
        } else {
            // Add visible files and folders to the visible list
            if entry.path().is_dir() {
                visible_entries.push(format!("{}/", file_name));
            } else {
                visible_entries.push(file_name);
            }
        }
    }

    // Sort both lists alphabetically
    visible_entries.sort();
    hidden_entries.sort();

    // Combine visible entries first, then hidden entries
    let mut entries = visible_entries;
    entries.extend(hidden_entries);

    // Ensure ".." is always at the top
    if dir.parent().is_some() {
        entries.insert(0, "🗂️ ..".to_string());
    }

    entries
}

fn tracker_select(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    tracker_options: &[&str],
    selected_tracker_index: &mut usize,
    tracker_scroll_offset: &mut usize,
    selected_trackers: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let size = terminal.size()?;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
            .split(size);

        let content_area_height = chunks[0].height.saturating_sub(1) as usize;

        // Ensure scrolling logic
        if *tracker_scroll_offset > tracker_options.len().saturating_sub(content_area_height) {
            *tracker_scroll_offset = tracker_options.len().saturating_sub(content_area_height);
        }

        let visible_trackers = &tracker_options[*tracker_scroll_offset
            ..(*tracker_scroll_offset + content_area_height).min(tracker_options.len())];

        // Draw the tracker selection UI
        terminal.draw(|f| {
            let tracker_list = List::new(
                visible_trackers
                    .iter()
                    .enumerate()
                    .map(|(i, tracker)| {
                        let style = if i + *tracker_scroll_offset == *selected_tracker_index {
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else if selected_trackers.contains(&tracker.to_string()) {
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(Span::styled(*tracker, style))
                    })
                    .collect::<Vec<_>>(),
            )
            .block(Block::default().borders(Borders::ALL).title("Select Tracker"));

            f.render_widget(tracker_list, chunks[0]);

            // Render help message
            let help_message = "Use UP/DOWN to navigate, F to toggle trackers, ENTER to confirm.";
            let help_paragraph = Paragraph::new(help_message)
                .block(Block::default().borders(Borders::ALL).title("Help"));
            f.render_widget(help_paragraph, chunks[1]);
        })?;

        // Handle keypress events
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up => {
                    if *selected_tracker_index > 0 {
                        *selected_tracker_index -= 1;
                        if *selected_tracker_index < *tracker_scroll_offset {
                            *tracker_scroll_offset -= 1;
                        }
                    }
                }
                KeyCode::Down => {
                    if *selected_tracker_index < tracker_options.len() - 1 {
                        *selected_tracker_index += 1;
                        if *selected_tracker_index >= *tracker_scroll_offset + content_area_height {
                            *tracker_scroll_offset += 1;
                        }
                    }
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    let tracker = tracker_options[*selected_tracker_index].to_string();
                    if tracker == "✔️ Select All" {
                        if selected_trackers.len() == tracker_options.len() - 1 {
                            selected_trackers.clear();
                        } else {
                            *selected_trackers = tracker_options[1..]
                                .iter()
                                .map(|&s| s.to_string())
                                .collect();
                        }
                    } else if selected_trackers.contains(&tracker) {
                        selected_trackers.retain(|t| t != &tracker);
                    } else {
                        selected_trackers.push(tracker);
                    }
                }
                KeyCode::Enter => {
                    // Confirm tracker selection and exit
                    return Ok(()); // Exit the tracker selection loop
                }
                KeyCode::Esc => {
                    // Exit tracker selection without changes
                    return Ok(()); // Exit the tracker selection loop
                }
                _ => {}
            }
        }
    }
}

fn read_log_file(log_file_path: &Path, log_output: Arc<Mutex<Vec<String>>>) {
    if let Ok(file) = File::open(log_file_path) {
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().filter_map(|line| line.ok()).collect();

        let mut log_output_guard = log_output.lock().unwrap();
        *log_output_guard = lines;
    }
}

fn start_log_refresh(
    log_file_path: PathBuf,
    log_output: Arc<Mutex<Vec<String>>>,
    tx: mpsc::Sender<()>, // Notify the main loop to redraw the UI
    log_scroll_offset: Arc<Mutex<usize>>, // Shared scroll offset for logs
) {
    thread::spawn(move || {
        let mut file = match File::open(&log_file_path) {
            Ok(file) => file,
            Err(_) => return, // Exit if the file cannot be opened
        };

        let _ = file.seek(SeekFrom::End(0)); // Start tailing from the end of the file
        let mut reader = BufReader::new(file);

        loop {
            let mut buffer = String::new();
            let mut new_lines = Vec::new();

            // Read multiple lines in a batch
            for _ in 0..10 {
                match reader.read_line(&mut buffer) {
                    Ok(0) => break, // No new data
                    Ok(_) => {
                        // Filter only `[INFO]` messages
                        if buffer.contains("[INFO]") {
                            new_lines.push(buffer.trim_end().to_string());
                        }
                        buffer.clear();
                    }
                    Err(_) => break, // Exit on error
                }
            }

            if !new_lines.is_empty() {
                // Add the new lines to the log output
                let mut log_output_guard = log_output.lock().unwrap();
                log_output_guard.extend(new_lines);

                // Automatically scroll to the bottom if the user hasn't manually scrolled
                let mut log_scroll_offset_guard = log_scroll_offset.lock().unwrap();
                let total_lines = log_output_guard.len();
                let visible_lines = 15; // Adjust this to match the height of your log view
                if *log_scroll_offset_guard >= total_lines.saturating_sub(visible_lines) {
                    *log_scroll_offset_guard = total_lines.saturating_sub(visible_lines);
                }

                // Notify the main loop to redraw the UI
                let _ = tx.send(());
            }

            // Sleep briefly to avoid excessive CPU usage
            thread::sleep(Duration::from_millis(50));
        }
    });
}

fn parse_preflight_log(preflight_log_path: &Path) -> (Vec<String>, bool) {
    let mut log_data = vec![
        "Title: N/A".to_string(),
        "Release Name: N/A".to_string(),
        "Dupe Check: N/A".to_string(),
        "Release Type: N/A".to_string(),
        "Season Number: N/A".to_string(),
        "Episode Number: N/A".to_string(),
        "TMDB ID: N/A".to_string(),
        "IMDb ID: N/A".to_string(),
        "TVDB ID: N/A".to_string(),
        "Album Cover: N/A".to_string(), // Default value for Album Cover
        "Excluded Files: N/A".to_string(), // Default value for Excluded Files
        "Audio Languages: N/A".to_string(),
    ];

    let mut is_pending = true; // Assume pending until we find meaningful data
    let mut is_music_log = false; // Flag to detect music preflight logs

    if let Ok(file) = File::open(preflight_log_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().filter_map(|line| line.ok()) {
            is_pending = false; // Mark as not pending if we find any data

            if line.starts_with("Log Type: Music") {
                is_music_log = true; // Identify this as a music preflight log
            } else if line.starts_with("Title:") && !line.contains("Pre-flight Check Results:") {
                log_data[0] = line;
            } else if line.starts_with("Release Name:") {
                log_data[1] = line;
            } else if line.starts_with("Dupe Check:") {
                log_data[2] = line;
            } else if line.starts_with("Release Type:") {
                log_data[3] = line;
            } else if line.starts_with("Season Number:") {
                log_data[4] = line;
            } else if line.starts_with("Episode Number:") {
                log_data[5] = line;
            } else if line.starts_with("TMDB ID:") {
                log_data[6] = line;
            } else if line.starts_with("IMDb ID:") {
                log_data[7] = line;
            } else if line.starts_with("TVDB ID:") {
                log_data[8] = line;
            } else if line.starts_with("Album Cover:") {
                // Handle "Album Cover:" field for both music and non-music logs
                let cleaned_line = line.replace("Album Cover: ", "").trim().to_string(); // Remove redundant prefix and trim whitespace
                let value = if cleaned_line.eq_ignore_ascii_case("Available") {
                    "Album Cover: ✔️ Available".to_string()
                } else if cleaned_line.eq_ignore_ascii_case("Not Available")
                    || cleaned_line.eq_ignore_ascii_case("Not Found")
                {
                    "Album Cover: ❌ Not Found".to_string() // Use "Not Found" for music logs
                } else {
                    "Album Cover: N/A".to_string() // Use "N/A" for non-music logs
                };
                log_data[9] = value; // Store Album Cover in index 9
            } else if line.starts_with("Excluded Files:") {
                // Handle "Excluded Files:" field for both music and non-music logs
                let value = if is_music_log {
                    "Strip From Videos: N/A".to_string() // Set to N/A for music logs
                } else if line.contains("Yes") {
                    "Strip From Videos: ✔️ Enabled".to_string()
                } else {
                    "Strip From Videos: ❌ Disabled".to_string()
                };
                log_data[10] = value; // Store Excluded Files in index 10
            } else if line.starts_with("Audio Languages:") {
                // Parse the audio languages field and remove brackets/quotes
                let audio_line = line.replace("Audio Languages: ", "");
                let audio_cleaned = audio_line
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .replace('"', "");
                log_data[11] = format!("Audio Languages: {}", audio_cleaned);
            }
        }
    }

    // If it's a music log but no Album Cover field was found, set it to "Not Found"
    if is_music_log && log_data[9] == "Album Cover: N/A" {
        log_data[9] = "Album Cover: ❌ Not Found".to_string();
    }

    (log_data, is_pending)
}

fn start_log_tail(terminal_emulator: Arc<TerminalEmulator>, log_file_path: &str) {
    let log_file_path = log_file_path.to_string(); // Clone the path into a String
    thread::spawn(move || {
        let mut child = Command::new("tail")
            .arg("-f")
            .arg(log_file_path) // Use the cloned String
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start tail process");

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    terminal_emulator.feed(&line);
                }
            }
        }
    });
}