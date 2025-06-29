// --- External Crates ---
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, MouseEvent, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
// --- Standard Library ---
use std::{
    io::{self},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

// --- Internal Modules ---
use crate::{
    core::{Config, PreflightCheckResult, TorrentInfo},
    processing::{preflight::preflight_check, process_builder},
    media::detector::detect_media_type,
    trackers::seedpool::parse_seedpool_category_type,
};

// --- State Management ---
#[derive(Debug, Clone, PartialEq)]
enum UIState {
    Main,
    FileSelection,
    TrackerSelection,
    CategoryInput,
    ViewingLog,
}

#[derive(Debug, Clone)]
enum ActionAvailability {
    Available,
    RequiresInput,    // Needs input path
    RequiresTracker,  // Needs tracker selection
    RequiresBoth,     // Needs both input and tracker
    InProgress,       // Currently running
}

struct AppState {
    current_state: UIState,
    current_dir: PathBuf,
    file_list: Vec<String>,
    filtered_file_list: Vec<String>,
    selected_file_index: usize,
    scroll_offset: usize,
    selected_trackers: Vec<String>,
    input_path: Option<PathBuf>,
    category_code: Option<String>,
    log_output: Arc<Mutex<Vec<String>>>,
    log_scroll_offset: usize,
    preflight_result: Option<PreflightCheckResult>,
    upload_running: bool,
    preflight_running: bool,
    input_buffer: String,
    show_help: bool,
    dry_run: bool,
    preflight_receiver: Option<std::sync::mpsc::Receiver<PreflightCheckResult>>,
    file_filter: String,
    filter_active: bool,
    last_click_time: std::time::Instant,
    last_clicked_item: Option<usize>,
    file_list_lowercase: Vec<String>, // Pre-computed lowercase versions for faster filtering
    filter_debounce_time: std::time::Instant,
}

impl AppState {
    fn new() -> io::Result<Self> {
        let current_dir = std::env::current_dir()?;
        let file_list = get_files_in_dir(&current_dir);
        let filtered_file_list = file_list.clone();
        let file_list_lowercase = file_list.iter().map(|s| s.to_lowercase()).collect();
        
        Ok(Self {
            current_state: UIState::Main,
            current_dir,
            file_list,
            filtered_file_list,
            selected_file_index: 0,
            scroll_offset: 0,
            selected_trackers: vec![],
            input_path: None,
            category_code: None,
            log_output: Arc::new(Mutex::new(Vec::new())),
            log_scroll_offset: 0,
            preflight_result: None,
            upload_running: false,
            preflight_running: false,
            input_buffer: String::new(),
            show_help: false,
            dry_run: false,
            preflight_receiver: None,
            file_filter: String::new(),
            filter_active: false,
            last_click_time: std::time::Instant::now(),
            last_clicked_item: None,
            file_list_lowercase,
            filter_debounce_time: std::time::Instant::now(),
        })
    }

    fn apply_file_filter(&mut self) {
        if self.file_filter.is_empty() {
            self.filtered_file_list = self.file_list.clone();
            self.filter_active = false;
        } else {
            let filter = self.file_filter.to_lowercase();
            
            // Collect matches with priority scoring
            let mut matches: Vec<(usize, u8)> = Vec::new(); // (index, priority)
            
            for (i, lowercase_name) in self.file_list_lowercase.iter().enumerate() {
                if lowercase_name.contains(&filter) {
                    let priority = if lowercase_name.starts_with(&filter) {
                        1 // Highest priority: starts with filter
                    } else if lowercase_name.split_whitespace().any(|word| word.starts_with(&filter)) {
                        2 // Medium priority: word starts with filter
                    } else {
                        3 // Lowest priority: contains filter somewhere
                    };
                    matches.push((i, priority));
                }
            }
            
            // Sort by priority, then alphabetically within each priority
            matches.sort_by(|a, b| {
                match a.1.cmp(&b.1) {
                    std::cmp::Ordering::Equal => {
                        // Same priority - sort alphabetically
                        self.file_list[a.0].to_lowercase().cmp(&self.file_list[b.0].to_lowercase())
                    }
                    other => other
                }
            });
            
            // Build the filtered list
            self.filtered_file_list = matches
                .into_iter()
                .map(|(i, _)| self.file_list[i].clone())
                .collect();
            
            self.filter_active = true;
        }
        self.selected_file_index = 0;
        self.scroll_offset = 0;
    }

    fn apply_file_filter_debounced(&mut self) {
        // Only apply filter if enough time has passed since last keystroke
        let now = Instant::now();
        self.filter_debounce_time = now;
        
        // Apply filter immediately for very short lists, or after debounce for large lists
        if self.file_list.len() < 100 {
            self.apply_file_filter();
        } else {
            // For large directories, we'll check the debounce in the main loop
            // This prevents lag on every keystroke
        }
    }

    fn check_and_apply_filter_debounce(&mut self) {
        if self.filter_active || !self.file_filter.is_empty() {
            let now = Instant::now();
            if now.duration_since(self.filter_debounce_time) > Duration::from_millis(150) {
                self.apply_file_filter();
            }
        }
    }

    fn update_file_list(&mut self, new_dir: &std::path::PathBuf) {
        self.file_list = get_files_in_dir(new_dir);
        self.file_list_lowercase = self.file_list.iter().map(|s| s.to_lowercase()).collect();
        // Clear filter when navigating to new directory
        self.file_filter.clear();
        self.apply_file_filter();
    }

    fn get_current_file_list(&self) -> &Vec<String> {
        &self.filtered_file_list
    }

    fn add_log(&self, message: String) {
        self.log_output.lock().unwrap().push(message);
    }

    fn clear_logs(&self) {
        self.log_output.lock().unwrap().clear();
    }

    fn get_action_availability(&self, action: &str) -> ActionAvailability {
        match action {
            "preflight" => {
                if self.preflight_running {
                    ActionAvailability::InProgress
                } else if self.input_path.is_none() {
                    ActionAvailability::RequiresInput
                } else {
                    ActionAvailability::Available
                }
            }
            "upload" | "upload_dry" => {
                if self.upload_running {
                    ActionAvailability::InProgress
                } else if self.input_path.is_none() && self.selected_trackers.is_empty() {
                    ActionAvailability::RequiresBoth
                } else if self.input_path.is_none() {
                    ActionAvailability::RequiresInput
                } else if self.selected_trackers.is_empty() {
                    ActionAvailability::RequiresTracker
                } else {
                    ActionAvailability::Available
                }
            }
            "select_file" | "select_tracker" | "view_log" | "clear_log" | "help" => {
                ActionAvailability::Available
            }
            _ => ActionAvailability::Available
        }
    }
}

// --- Main UI Function ---
pub fn launch_ui() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging if not already initialized
    use log::LevelFilter;
    use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger};
    use std::fs::OpenOptions;
    
    let log_path = std::path::Path::new("seedbrr.log");
    let _ = CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Debug,
        SimpleLogConfig::default(),
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?,
    )]);
    
    // Log UI startup
    log::info!("🚀 seedbrr UI launched");
    
    // Set up panic hook
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Enable raw mode and set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize state
    let mut state = AppState::new()?;
    
    // Load config for preflight checks
    let config = load_config()?;

    // Main loop
    loop {
        // Check for preflight results
        if let Some(ref receiver) = state.preflight_receiver {
            if let Ok(result) = receiver.try_recv() {
                state.preflight_result = Some(result);
                state.preflight_running = false;
                state.preflight_receiver = None; // Clear the receiver
            }
        }
        
        // Draw UI
        terminal.draw(|f| render_ui(f, &state, &config))?;

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    match state.current_state {
                        UIState::Main => handle_main_input(key, &mut state)?,
                        UIState::FileSelection => handle_file_selection_input(key, &mut state)?,
                        UIState::TrackerSelection => handle_tracker_selection_input(key, &mut state)?,
                        UIState::CategoryInput => handle_category_input(key, &mut state)?,
                        UIState::ViewingLog => handle_log_view_input(key, &mut state)?,
                    }
                    
                    // Global key handlers
                    match key.code {
                        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::F(1) => state.show_help = !state.show_help,
                        _ => {}
                    }
                },
                Event::Mouse(mouse) => {
                    match state.current_state {
                        UIState::Main => handle_mouse_input(mouse, &mut state)?,
                        UIState::FileSelection => handle_file_selection_mouse(mouse, &mut state)?,
                        UIState::TrackerSelection => handle_tracker_selection_mouse(mouse, &mut state)?,
                        UIState::CategoryInput => handle_category_input_mouse(mouse, &mut state)?,
                        UIState::ViewingLog => handle_log_view_mouse(mouse, &mut state)?,
                    }
                },
                _ => {}
            }
        }
        
        // Check for debounced filter updates (for large directories)
        state.check_and_apply_filter_debounce();
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    
    Ok(())
}

// --- Input Handlers ---
fn handle_main_input(key: event::KeyEvent, state: &mut AppState) -> io::Result<()> {
    match key.code {
        KeyCode::Char('f') | KeyCode::Char('F') => {
            state.current_state = UIState::FileSelection;
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            state.current_state = UIState::TrackerSelection;
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            state.current_state = UIState::CategoryInput;
            state.input_buffer.clear();
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            let availability = state.get_action_availability("preflight");
            match availability {
                ActionAvailability::Available => start_preflight_check(state)?,
                ActionAvailability::InProgress => state.add_log("⏳ Preflight check already running".to_string()),
                ActionAvailability::RequiresInput => state.add_log("❌ Please select an input path first (press F)".to_string()),
                _ => {}
            }
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            let availability = state.get_action_availability("upload");
            match availability {
                ActionAvailability::Available => start_upload(state)?,
                ActionAvailability::InProgress => state.add_log("⏳ Upload already running".to_string()),
                ActionAvailability::RequiresInput => state.add_log("❌ Please select an input path first (press F)".to_string()),
                ActionAvailability::RequiresTracker => state.add_log("❌ Please select at least one tracker (press T)".to_string()),
                ActionAvailability::RequiresBoth => state.add_log("❌ Please select both input path (F) and trackers (T)".to_string()),
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            state.current_state = UIState::ViewingLog;
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            state.dry_run = !state.dry_run;
            state.add_log(format!("🔄 Dry-run mode: {}", if state.dry_run { "ENABLED" } else { "DISABLED" }));
        }
        _ => {}
    }
    Ok(())
}

fn handle_mouse_input(mouse: MouseEvent, state: &mut AppState) -> io::Result<()> {
    if let MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
        // The UI layout has:
        // - Header: lines 0-2 (3 lines)
        // - Info panel: lines 3-10 (8 lines) 
        // - Preflight area: lines 11-22+ (Min 12 lines)
        // - Actions area: lines 23+ (10 lines)
        
        // Check if click is in the info panel area (lines 4-9, accounting for borders)
        if mouse.row >= 4 && mouse.row <= 9 {
            match mouse.row {
                4 => {
                    // Input path line clicked (first line in info panel)
                    state.current_state = UIState::FileSelection;
                }
                5 => {
                    // Trackers line clicked (second line in info panel)
                    state.current_state = UIState::TrackerSelection;
                }
                6 => {
                    // Category code line clicked (third line in info panel)
                    state.current_state = UIState::CategoryInput;
                    state.input_buffer.clear();
                }
                7 => {
                    // Dry-run mode line clicked (fourth line in info panel)
                    state.dry_run = !state.dry_run;
                    state.add_log(format!("🔄 Dry-run mode: {}", if state.dry_run { "ENABLED" } else { "DISABLED" }));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

// Continue with remaining input handlers...
fn handle_file_selection_input(key: event::KeyEvent, state: &mut AppState) -> io::Result<()> {
    match key.code {
        KeyCode::Up => {
            if state.selected_file_index > 0 {
                state.selected_file_index -= 1;
                if state.selected_file_index < state.scroll_offset {
                    state.scroll_offset = state.selected_file_index;
                }
            }
        }
        KeyCode::Down => {
            let current_file_list = state.get_current_file_list();
            if state.selected_file_index < current_file_list.len().saturating_sub(1) {
                state.selected_file_index += 1;
                let visible_height = 15; // Approximate visible items
                if state.selected_file_index >= state.scroll_offset + visible_height {
                    state.scroll_offset = state.selected_file_index - visible_height + 1;
                }
            }
        }
        KeyCode::Enter => {
            let current_file_list = state.get_current_file_list();
            if let Some(selected) = current_file_list.get(state.selected_file_index) {
                let path = state.current_dir.join(selected);
                if path.is_dir() && selected == ".." {
                    if let Some(parent) = state.current_dir.parent() {
                        state.current_dir = parent.to_path_buf();
                        state.update_file_list(&state.current_dir.clone());
                        state.selected_file_index = 0;
                        state.scroll_offset = 0;
                    }
                } else if path.is_dir() {
                    state.current_dir = path;
                    state.update_file_list(&state.current_dir.clone());
                    state.selected_file_index = 0;
                    state.scroll_offset = 0;
                } else {
                    state.input_path = Some(path);
                    state.current_state = UIState::Main;
                    
                    // Auto-detect media type
                    if let Ok(media_files) = detect_media_type(&state.input_path.as_ref().unwrap().to_string_lossy()) {
                        if !media_files.is_empty() {
                            state.add_log(format!("Detected {} media files", media_files.len()));
                            // Show dominant media type
                            if let Some(first) = media_files.first() {
                                state.add_log(format!("Primary type: {:?}", first.media_type));
                            }
                        }
                    }
                }
            }
        }
        KeyCode::Char(' ') => {
            // Space to select current directory
            let current_file_list = state.get_current_file_list();
            if let Some(selected) = current_file_list.get(state.selected_file_index) {
                let path = state.current_dir.join(selected);
                if path.is_dir() && selected != ".." {
                    state.input_path = Some(path);
                    state.current_state = UIState::Main;
                    
                    // Auto-detect media type for directory
                    if let Ok(media_files) = detect_media_type(&state.input_path.as_ref().unwrap().to_string_lossy()) {
                        if !media_files.is_empty() {
                            state.add_log(format!("Detected {} media files", media_files.len()));
                            // Show dominant media type
                            if let Some(first) = media_files.first() {
                                state.add_log(format!("Primary type: {:?}", first.media_type));
                            }
                        }
                    }
                }
            }
        }
        KeyCode::Char('/') => {
            // Start filter mode
            state.input_buffer = state.file_filter.clone();
            state.current_state = UIState::CategoryInput; // Reuse category input for filter
        }
        KeyCode::Char(c) if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' => {
            // Quick filter by typing (allow common filename characters)
            state.file_filter.push(c);
            state.apply_file_filter_debounced();
        }
        KeyCode::Backspace => {
            // Remove last character from filter
            if !state.file_filter.is_empty() {
                state.file_filter.pop();
                state.apply_file_filter_debounced();
            }
        }
        KeyCode::Esc => {
            if !state.file_filter.is_empty() {
                // Clear filter first if active
                state.file_filter.clear();
                state.apply_file_filter();
            } else {
                // Exit to main if no filter
                state.current_state = UIState::Main;
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_tracker_selection_input(key: event::KeyEvent, state: &mut AppState) -> io::Result<()> {
    let trackers = vec!["seedpool", "torrentleech"];
    
    match key.code {
        KeyCode::Char(' ') => {
            // Toggle selection
            if let Some(index) = state.selected_file_index.checked_sub(1) {
                if index < trackers.len() {
                    let tracker = trackers[index].to_string();
                    if let Some(pos) = state.selected_trackers.iter().position(|x| x == &tracker) {
                        state.selected_trackers.remove(pos);
                    } else {
                        state.selected_trackers.push(tracker);
                    }
                }
            } else if state.selected_file_index == 0 {
                // Select all
                if state.selected_trackers.len() == trackers.len() {
                    state.selected_trackers.clear();
                } else {
                    state.selected_trackers = trackers.iter().map(|s| s.to_string()).collect();
                }
            }
        }
        KeyCode::Up => {
            if state.selected_file_index > 0 {
                state.selected_file_index -= 1;
            }
        }
        KeyCode::Down => {
            if state.selected_file_index <= trackers.len() {
                state.selected_file_index += 1;
            }
        }
        KeyCode::Enter | KeyCode::Esc => {
            state.current_state = UIState::Main;
            state.selected_file_index = 0;
        }
        _ => {}
    }
    Ok(())
}

fn handle_category_input(key: event::KeyEvent, state: &mut AppState) -> io::Result<()> {
    match key.code {
        KeyCode::Char(c) if c.is_numeric() => {
            if state.input_buffer.len() < 4 {
                state.input_buffer.push(c);
            }
        }
        KeyCode::Backspace => {
            state.input_buffer.pop();
        }
        KeyCode::Enter => {
            if state.input_buffer.len() == 4 {
                state.category_code = Some(state.input_buffer.clone());
                state.add_log(format!("Category code set to: {}", state.input_buffer));
            }
            state.current_state = UIState::Main;
        }
        KeyCode::Esc => {
            state.current_state = UIState::Main;
        }
        _ => {}
    }
    Ok(())
}

fn handle_log_view_input(key: event::KeyEvent, state: &mut AppState) -> io::Result<()> {
    let log_len = state.log_output.lock().unwrap().len();
    match key.code {
        KeyCode::Up => {
            if state.log_scroll_offset > 0 {
                state.log_scroll_offset -= 1;
            }
        }
        KeyCode::Down => {
            if state.log_scroll_offset < log_len.saturating_sub(1) {
                state.log_scroll_offset += 1;
            }
        }
        KeyCode::PageUp => {
            state.log_scroll_offset = state.log_scroll_offset.saturating_sub(10);
        }
        KeyCode::PageDown => {
            state.log_scroll_offset = (state.log_scroll_offset + 10).min(log_len.saturating_sub(1));
        }
        KeyCode::Esc | KeyCode::Char('l') | KeyCode::Char('L') => {
            state.current_state = UIState::Main;
        }
        _ => {}
    }
    Ok(())
}

// --- Mouse Input Handlers ---
fn handle_file_selection_mouse(mouse: MouseEvent, state: &mut AppState) -> io::Result<()> {
    if let MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
        // File selection area starts after header (3 lines) and has borders
        // Content area is inside the border, so clickable area is from row 4 to area height - 1
        if mouse.row >= 4 {
            let clicked_index = (mouse.row - 4) as usize + state.scroll_offset;
            let current_file_list_len = state.get_current_file_list().len();
            
            if clicked_index < current_file_list_len {
                let now = Instant::now();
                let double_click_threshold = Duration::from_millis(500); // 500ms for double-click
                
                // Check if this is a double-click on the same item
                let is_double_click = if let Some(last_item) = state.last_clicked_item {
                    last_item == clicked_index && now.duration_since(state.last_click_time) < double_click_threshold
                } else {
                    false
                };
                
                // Update click tracking
                state.last_click_time = now;
                state.last_clicked_item = Some(clicked_index);
                state.selected_file_index = clicked_index;
                
                // Get the selected file name without holding the borrow
                let selected_file = state.get_current_file_list().get(clicked_index).cloned();
                
                if let Some(selected) = selected_file {
                    let path = state.current_dir.join(&selected);
                    
                    if path.is_file() {
                        // Files are always selected on single click
                        state.input_path = Some(path);
                        state.current_state = UIState::Main;
                        
                        // Auto-detect media type
                        if let Ok(media_files) = detect_media_type(&state.input_path.as_ref().unwrap().to_string_lossy()) {
                            if !media_files.is_empty() {
                                state.add_log(format!("Detected {} media files", media_files.len()));
                                if let Some(first) = media_files.first() {
                                    state.add_log(format!("Primary type: {:?}", first.media_type));
                                }
                            }
                        }
                    } else if path.is_dir() {
                        if selected == ".." {
                            // Parent directory - always navigate on single click for convenience
                            if let Some(parent) = state.current_dir.parent() {
                                state.current_dir = parent.to_path_buf();
                                state.update_file_list(&state.current_dir.clone());
                                state.selected_file_index = 0;
                                state.scroll_offset = 0;
                            }
                        } else if is_double_click {
                            // Double-click on folder - navigate into it
                            state.current_dir = path;
                            state.update_file_list(&state.current_dir.clone());
                            state.selected_file_index = 0;
                            state.scroll_offset = 0;
                        }
                        // Single click on folder just highlights it (selection index already updated above)
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_tracker_selection_mouse(mouse: MouseEvent, state: &mut AppState) -> io::Result<()> {
    if let MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
        let trackers = vec!["seedpool", "torrentleech"];
        
        // Tracker selection area starts after header (3 lines) and has borders
        if mouse.row >= 4 {
            let clicked_index = (mouse.row - 4) as usize;
            
            if clicked_index == 0 {
                // "Select All" clicked
                if state.selected_trackers.len() == trackers.len() {
                    state.selected_trackers.clear();
                } else {
                    state.selected_trackers = trackers.iter().map(|s| s.to_string()).collect();
                }
            } else if let Some(tracker_index) = clicked_index.checked_sub(1) {
                if tracker_index < trackers.len() {
                    let tracker = trackers[tracker_index].to_string();
                    if let Some(pos) = state.selected_trackers.iter().position(|x| x == &tracker) {
                        state.selected_trackers.remove(pos);
                    } else {
                        state.selected_trackers.push(tracker);
                    }
                }
            }
            
            // Update selected index for visual feedback
            state.selected_file_index = clicked_index;
        }
    }
    Ok(())
}

fn handle_category_input_mouse(_mouse: MouseEvent, _state: &mut AppState) -> io::Result<()> {
    // Category input is primarily text-based, no special mouse handling needed
    Ok(())
}

fn handle_log_view_mouse(mouse: MouseEvent, state: &mut AppState) -> io::Result<()> {
    if let MouseEventKind::ScrollUp = mouse.kind {
        state.log_scroll_offset = state.log_scroll_offset.saturating_sub(3);
    } else if let MouseEventKind::ScrollDown = mouse.kind {
        let log_len = state.log_output.lock().unwrap().len();
        state.log_scroll_offset = (state.log_scroll_offset + 3).min(log_len.saturating_sub(1));
    }
    Ok(())
}

// --- Rendering Functions ---
fn render_ui(f: &mut ratatui::Frame, state: &AppState, _config: &Config) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Status bar
        ])
        .split(f.size());

    // Header
    let header = Paragraph::new("🌱 Seedbrr - Media Upload Manager")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Main content area
    match state.current_state {
        UIState::Main => render_main_view(f, chunks[1], state),
        UIState::FileSelection => render_file_selection(f, chunks[1], state),
        UIState::TrackerSelection => render_tracker_selection(f, chunks[1], state),
        UIState::CategoryInput => render_category_input(f, chunks[1], state),
        UIState::ViewingLog => render_log_view(f, chunks[1], state),
    }

    // Status bar
    render_status_bar(f, chunks[2], state);
}

fn render_main_view(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Info panel (clickable)
            Constraint::Min(12),    // Preflight results area - increased from 8 to Min(12)
            Constraint::Length(10), // Actions panel - decreased from Min(5) to Length(10)
        ])
        .split(area);

    // Info panel
    let mut info_lines = vec![
        Line::from(vec![
            Span::raw("📁 Input Path: "),
            Span::styled(
                state.input_path.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Not selected".to_string()),
                Style::default().fg(if state.input_path.is_some() { Color::Green } else { Color::Yellow })
            ),
            Span::styled(" [Click to change]", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]),
        Line::from(vec![
            Span::raw("🎯 Trackers: "),
            Span::styled(
                if state.selected_trackers.is_empty() {
                    "None selected".to_string()
                } else {
                    state.selected_trackers.join(", ")
                },
                Style::default().fg(if state.selected_trackers.is_empty() { Color::Yellow } else { Color::Green })
            ),
            Span::styled(" [Click to change]", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]),
        Line::from(vec![
            Span::raw("🏷️  Category Code: "),
            Span::styled(
                state.category_code.as_ref()
                    .unwrap_or(&"Auto-detect".to_string())
                    .clone(),
                Style::default().fg(Color::Cyan)
            ),
            Span::styled(" [Click to change]", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]),
        Line::from(vec![
            Span::raw("🚫 Dry-Run Mode: "),
            Span::styled(
                if state.dry_run { "ENABLED" } else { "DISABLED" },
                Style::default().fg(if state.dry_run { Color::Yellow } else { Color::Gray })
            ),
            Span::styled(" [Click to toggle]", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]),
    ];

    if let Some(ref _result) = state.preflight_result {
        info_lines.push(Line::from(""));
        info_lines.push(Line::from(vec![
            Span::raw("✈️  Preflight: "),
            Span::styled(
                "Completed".to_string(),
                Style::default().fg(Color::Blue)
            ),
        ]));
    }

    let info_panel = Paragraph::new(info_lines)
        .block(Block::default()
            .title("Upload Information (Click items to edit)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(info_panel, chunks[0]);

    // Preflight results area
    let mut preflight_lines = vec![];
    if let Some(ref result) = state.preflight_result {
        preflight_lines.push(Line::from(vec![
            Span::raw("📝 Title: "),
            Span::styled(&result.release_name, Style::default().fg(Color::White)),
        ]));
        
        preflight_lines.push(Line::from(vec![
            Span::raw("🏷️  Generated: "),
            Span::styled(&result.generated_release_name, Style::default().fg(Color::Cyan)),
        ]));
        
        preflight_lines.push(Line::from(vec![
            Span::raw("🎯 Type: "),
            Span::styled(&result.release_type, Style::default().fg(Color::Blue)),
        ]));
        
        if result.is_boxset {
            preflight_lines.push(Line::from(vec![
                Span::raw("📦 Format: "),
                Span::styled("Season Pack/Boxset", Style::default().fg(Color::Magenta)),
            ]));
        } else if result.episode_number.is_some() && result.episode_number != Some(0) {
            preflight_lines.push(Line::from(vec![
                Span::raw("📺 Format: "),
                Span::styled("Single Episode", Style::default().fg(Color::Green)),
            ]));
        }
        
        // Show season/episode info
        if let Some(season) = result.season_number {
            if let Some(episode) = result.episode_number {
                if episode > 0 {
                    preflight_lines.push(Line::from(vec![
                        Span::raw("📺 Season/Episode: "),
                        Span::styled(format!("S{:02}E{:02}", season, episode), Style::default().fg(Color::Yellow)),
                    ]));
                } else {
                    preflight_lines.push(Line::from(vec![
                        Span::raw("📺 Season: "),
                        Span::styled(format!("{}", season), Style::default().fg(Color::Yellow)),
                    ]));
                }
            }
        }
        
        preflight_lines.push(Line::from(vec![
            Span::raw("🔍 Dupe Check: "),
            Span::styled(&result.dupe_check, Style::default().fg(
                if result.dupe_check.contains("PASS") { Color::Green } else { Color::Red }
            )),
        ]));
        
        // Show external IDs
        if result.tmdb_id > 0 || result.imdb_id.is_some() || result.tvdb_id.is_some() {
            preflight_lines.push(Line::from(vec![
                Span::raw("🆔 External IDs: "),
            ]));
            if result.tmdb_id > 0 {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   TMDB: "),
                    Span::styled(format!("{}", result.tmdb_id), Style::default().fg(Color::Cyan)),
                ]));
            }
            if let Some(ref imdb_id) = result.imdb_id {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   IMDB: "),
                    Span::styled(imdb_id, Style::default().fg(Color::Cyan)),
                ]));
            }
            if let Some(tvdb_id) = result.tvdb_id {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   TVDB: "),
                    Span::styled(format!("{}", tvdb_id), Style::default().fg(Color::Cyan)),
                ]));
            }
        }
        
        // Show IGDB data for games
        if result.release_type.contains("Game") && result.igdb_id.is_some() {
            preflight_lines.push(Line::from(vec![
                Span::raw("🎮 IGDB Info: "),
            ]));
            
            if let Some(igdb_id) = result.igdb_id {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   ID: "),
                    Span::styled(format!("{}", igdb_id), Style::default().fg(Color::Cyan)),
                ]));
            }
            
            if let Some(ref developer) = result.igdb_developer {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   Developer: "),
                    Span::styled(developer, Style::default().fg(Color::Yellow)),
                ]));
            }
            
            if let Some(ref publisher) = result.igdb_publisher {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   Publisher: "),
                    Span::styled(publisher, Style::default().fg(Color::Yellow)),
                ]));
            }
            
            if let Some(ref genres) = result.igdb_genres {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   Genres: "),
                    Span::styled(genres, Style::default().fg(Color::Magenta)),
                ]));
            }
            
            if let Some(rating) = result.igdb_rating {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   Rating: "),
                    Span::styled(format!("{:.1}/100", rating), Style::default().fg(
                        if rating >= 70.0 { Color::Green } 
                        else if rating >= 50.0 { Color::Yellow } 
                        else { Color::Red }
                    )),
                ]));
            }
            
            if let Some(ref platforms) = result.igdb_platforms {
                if !platforms.is_empty() {
                    let platform_str = if platforms.len() > 3 {
                        format!("{} and {} more", platforms[..3].join(", "), platforms.len() - 3)
                    } else {
                        platforms.join(", ")
                    };
                    preflight_lines.push(Line::from(vec![
                        Span::raw("   Platforms: "),
                        Span::styled(platform_str, Style::default().fg(Color::Blue)),
                    ]));
                }
            }
        }
        
        // Show audio languages (or platforms for games without IGDB data)
        if !result.audio_languages.is_empty() && !result.release_type.contains("Game") {
            preflight_lines.push(Line::from(vec![
                Span::raw("🔊 Audio: "),
                Span::styled(result.audio_languages.join(", "), Style::default().fg(Color::Green)),
            ]));
        }
        
        // Show album cover info for ebooks/music
        if result.album_cover != "N/A" {
            preflight_lines.push(Line::from(vec![
                Span::raw("🖼️  Cover: "),
                Span::styled(&result.album_cover, Style::default().fg(
                    if result.album_cover.contains("Available") { Color::Green } else { Color::Red }
                )),
            ]));
        }
        
        if !result.tracker_categories.is_empty() {
            preflight_lines.push(Line::from(vec![
                Span::raw("📋 Tracker Mappings: "),
            ]));
            for (tracker, category) in &result.tracker_categories {
                preflight_lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(tracker, Style::default().fg(Color::Yellow)),
                    Span::raw(" → "),
                    Span::styled(category, Style::default().fg(Color::Cyan)),
                ]));
            }
        }
    } else {
        preflight_lines.push(Line::from(vec![
            Span::styled("No preflight check run yet. Press ", Style::default().fg(Color::Gray)),
            Span::styled("P", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" to run preflight check.", Style::default().fg(Color::Gray)),
        ]));
    }
    
    let preflight_panel = Paragraph::new(preflight_lines)
        .block(Block::default()
            .title("Preflight Results")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(
                if state.preflight_result.is_some() { Color::Green } else { Color::Gray }
            )));
    f.render_widget(preflight_panel, chunks[1]);

    // Actions panel with availability indicators
    let actions = vec![
        Line::from(vec![
            Span::styled("F", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Select input file/folder"),
        ]),
        Line::from(vec![
            Span::styled("T", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Select trackers"),
        ]),
        Line::from(vec![
            Span::styled("C", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Set category code (4 digits)"),
        ]),
        Line::from(vec![
            Span::styled("D", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Toggle dry-run mode"),
        ]),
        Line::from({
            let availability = state.get_action_availability("preflight");
            match availability {
                ActionAvailability::Available => vec![
                    Span::styled("P", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Run preflight check"),
                ],
                ActionAvailability::InProgress => vec![
                    Span::styled("P", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::styled(" - Preflight check running...", Style::default().fg(Color::Blue)),
                ],
                ActionAvailability::RequiresInput => vec![
                    Span::styled("P", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
                    Span::styled(" - Run preflight check", Style::default().fg(Color::DarkGray)),
                    Span::styled(" (requires input path)", Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC)),
                ],
                _ => vec![],
            }
        }),
        Line::from({
            let availability = state.get_action_availability("upload");
            let upload_text = if state.dry_run { " - Start upload (DRY-RUN)" } else { " - Start upload" };
            match availability {
                ActionAvailability::Available => vec![
                    Span::styled("U", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(upload_text),
                ],
                ActionAvailability::InProgress => vec![
                    Span::styled("U", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::styled(" - Upload in progress...", Style::default().fg(Color::Blue)),
                ],
                ActionAvailability::RequiresInput => vec![
                    Span::styled("U", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
                    Span::styled(upload_text, Style::default().fg(Color::DarkGray)),
                    Span::styled(" (requires input path)", Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC)),
                ],
                ActionAvailability::RequiresTracker => vec![
                    Span::styled("U", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
                    Span::styled(upload_text, Style::default().fg(Color::DarkGray)),
                    Span::styled(" (requires tracker selection)", Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC)),
                ],
                ActionAvailability::RequiresBoth => vec![
                    Span::styled("U", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
                    Span::styled(upload_text, Style::default().fg(Color::DarkGray)),
                    Span::styled(" (requires input path and tracker)", Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC)),
                ],
            }
        }),
        Line::from(vec![
            Span::styled("L", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - View logs"),
        ]),
    ];

    let actions_panel = Paragraph::new(actions)
        .block(Block::default()
            .title("Available Actions")
            .borders(Borders::ALL));
    f.render_widget(actions_panel, chunks[2]);
}

// Placeholder implementations for other rendering functions
fn render_file_selection(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &AppState) {
    // Split area to show filter at the bottom if active
    let chunks = if state.filter_active || !state.file_filter.is_empty() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),      // File list
                Constraint::Length(3),   // Filter display
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(area)
    };

    let current_file_list = state.get_current_file_list();
    let items: Vec<ListItem> = current_file_list
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(chunks[0].height as usize - 2)
        .map(|(idx, item)| {
            let style = if idx == state.selected_file_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            let prefix = if state.current_dir.join(item).is_dir() {
                "📁 "
            } else {
                "📄 "
            };
            
            ListItem::new(format!("{}{}", prefix, item)).style(style)
        })
        .collect();

    let title = if state.filter_active {
        format!("Select File - {} (Filtered: {} items) [Esc to clear filter]", 
                state.current_dir.display(), current_file_list.len())
    } else {
        format!("Select File - {} [Double-click folders, type to filter, Esc to exit]", 
                state.current_dir.display())
    };

    let file_list = List::new(items)
        .block(Block::default()
            .title(title)
            .borders(Borders::ALL));
    
    f.render_widget(file_list, chunks[0]);

    // Render filter area if active
    if state.filter_active || !state.file_filter.is_empty() {
        let filter_text = if state.file_filter.is_empty() {
            "Filter: (empty) - Press Backspace to clear, type to filter".to_string()
        } else {
            format!("Filter: \"{}\" - {} matches", state.file_filter, current_file_list.len())
        };
        
        let filter_paragraph = Paragraph::new(filter_text)
            .block(Block::default()
                .title("Filter")
                .borders(Borders::ALL))
            .style(Style::default().fg(Color::Yellow));
        
        f.render_widget(filter_paragraph, chunks[1]);
    }
}

fn render_tracker_selection(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &AppState) {
    let trackers = vec!["seedpool", "torrentleech"];
    let mut items = vec![
        ListItem::new("[ ] Select All").style(
            if state.selected_file_index == 0 {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            }
        )
    ];

    for (idx, tracker) in trackers.iter().enumerate() {
        let selected = state.selected_trackers.contains(&tracker.to_string());
        let item = ListItem::new(format!(
            "[{}] {}",
            if selected { "✓" } else { " " },
            tracker
        )).style(
            if state.selected_file_index == idx + 1 {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            }
        );
        items.push(item);
    }

    let tracker_list = List::new(items)
        .block(Block::default()
            .title("Select Trackers (Click to toggle selections)")
            .borders(Borders::ALL));
    
    f.render_widget(tracker_list, area);
}

fn render_category_input(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &AppState) {
    let content = vec![
        Line::from(vec![
            Span::raw("Enter 4-digit category code (e.g., 0740): "),
            Span::styled(&state.input_buffer, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  0740 - Comic (E-Book)"),
        Line::from("  0203 - TV Show Encode"),
        Line::from("  0144 - Movie WEB-DL"),
    ];

    let paragraph = Paragraph::new(content)
        .block(Block::default()
            .title("Category Input")
            .borders(Borders::ALL));
    
    f.render_widget(paragraph, area);
}

fn render_log_view(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &AppState) {
    // Read all lines from seedbrr.log file
    let mut all_logs = Vec::new();
    
    // First add UI logs
    let ui_logs = state.log_output.lock().unwrap();
    all_logs.extend(ui_logs.iter().cloned());
    
    // Then read from seedbrr.log file
    if let Ok(log_content) = std::fs::read_to_string("seedbrr.log") {
        for line in log_content.lines() {
            if !line.trim().is_empty() {
                all_logs.push(line.to_string());
            }
        }
    }
    
    let items: Vec<ListItem> = all_logs
        .iter()
        .skip(state.log_scroll_offset)
        .take(area.height as usize - 2)
        .map(|log| {
            // Sanitize log lines for display
            let sanitized = log
                .replace('\n', " ")  // Replace newlines with spaces
                .replace('\r', "")   // Remove carriage returns
                .replace('\t', "  ") // Replace tabs with spaces
                .chars()
                .take(area.width as usize - 4) // Truncate to terminal width
                .collect::<String>();
            ListItem::new(sanitized)
        })
        .collect();

    let log_list = List::new(items)
        .block(Block::default()
            .title(format!("Logs ({} lines) - Scroll with mouse wheel", all_logs.len()))
            .borders(Borders::ALL));
    
    f.render_widget(log_list, area);
}

fn render_status_bar(f: &mut ratatui::Frame, area: ratatui::layout::Rect, _state: &AppState) {
    let status = Paragraph::new("Press Ctrl+Q to quit, F1 for help")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(status, area);
}

// --- Helper Functions ---
fn get_files_in_dir(dir: &PathBuf) -> Vec<String> {
    let mut entries = Vec::new();
    
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(name);
        }
    }
    
    entries.sort();
    entries.insert(0, "..".to_string());
    entries
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = "config/config.yaml";
    let content = std::fs::read_to_string(config_path)?;
    let config: Config = serde_yaml::from_str(&content)?;
    Ok(config)
}

fn start_preflight_check(state: &mut AppState) -> io::Result<()> {
    // Check if input path is available
    if state.input_path.is_none() {
        state.add_log("❌ No input path selected!".to_string());
        return Ok(());
    }
    
    state.preflight_running = true;
    state.add_log("Starting preflight check...".to_string());
    
    let input_path = state.input_path.clone().unwrap();
    let log_output = Arc::clone(&state.log_output);
    let dry_run = state.dry_run;
    
    // Load config for preflight check
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            log_output.lock().unwrap().push(format!("❌ Failed to load config: {}", e));
            state.preflight_running = false;
            return Ok(());
        }
    };
    
    // Create a channel to communicate results
    let (tx, rx) = std::sync::mpsc::channel::<PreflightCheckResult>();
    state.preflight_receiver = Some(rx);
    
    thread::spawn(move || {
        log_output.lock().unwrap().push(format!("🔍 Running preflight for: {}", input_path.display()));
        
        let path_str = match input_path.to_str() {
            Some(s) => s,
            None => {
                log_output.lock().unwrap().push("❌ Invalid path encoding".to_string());
                return;
            }
        };
        
        match preflight_check(path_str, &config, dry_run) {
            Ok(result) => {
                let mut logs = log_output.lock().unwrap();
                logs.push("\n===== PREFLIGHT CHECK RESULTS =====".to_string());
                logs.push(format!("📁 Path: {}", input_path.display()));
                logs.push(format!("📝 Release Name: {}", result.release_name));
                logs.push(format!("🏷️  Generated Name: {}", result.generated_release_name));
                logs.push(format!("🎯 Type: {}", result.release_type));
                
                if result.is_boxset {
                    logs.push("📦 Format: Season Pack/Boxset".to_string());
                } else if result.episode_number.is_some() && result.episode_number != Some(0) {
                    logs.push("📺 Format: Single Episode".to_string());
                }
                
                if let Some(season) = result.season_number {
                    if let Some(episode) = result.episode_number {
                        logs.push(format!("📺 Season: {} | Episode: {}", season, episode));
                    } else {
                        logs.push(format!("📺 Season: {}", season));
                    }
                }
                
                logs.push(format!("\n🔍 Duplicate Check: {}", result.dupe_check));
                
                logs.push("\n🆔 External IDs:".to_string());
                if result.tmdb_id > 0 {
                    logs.push(format!("  TMDB: {}", result.tmdb_id));
                }
                if let Some(ref imdb_id) = result.imdb_id {
                    logs.push(format!("  IMDB: {}", imdb_id));
                }
                if let Some(ref tvdb_id) = result.tvdb_id {
                    logs.push(format!("  TVDB: {}", tvdb_id));
                }
                
                // Add IGDB data for games
                if result.release_type.contains("Game") && result.igdb_id.is_some() {
                    logs.push("\n🎮 IGDB Information:".to_string());
                    if let Some(igdb_id) = result.igdb_id {
                        logs.push(format!("  ID: {}", igdb_id));
                    }
                    if let Some(ref developer) = result.igdb_developer {
                        logs.push(format!("  Developer: {}", developer));
                    }
                    if let Some(ref publisher) = result.igdb_publisher {
                        logs.push(format!("  Publisher: {}", publisher));
                    }
                    if let Some(ref genres) = result.igdb_genres {
                        logs.push(format!("  Genres: {}", genres));
                    }
                    if let Some(rating) = result.igdb_rating {
                        logs.push(format!("  Rating: {:.1}/100", rating));
                    }
                    if let Some(ref platforms) = result.igdb_platforms {
                        logs.push(format!("  Platforms: {}", platforms.join(", ")));
                    }
                }
                
                if !result.audio_languages.is_empty() {
                    logs.push(format!("\n🔊 Audio Languages: {}", result.audio_languages.join(", ")));
                } else if !result.release_type.contains("Game") {
                    logs.push("\n🔊 Audio Languages: Unknown".to_string());
                }
                
                if !result.tracker_categories.is_empty() {
                    logs.push("\n📋 Tracker Category Mappings:".to_string());
                    for (tracker, category) in &result.tracker_categories {
                        logs.push(format!("  {} → {}", tracker, category));
                    }
                }
                
                logs.push("\n✅ Preflight check completed!".to_string());
                
                // Send the result through the channel
                let _ = tx.send(result);
            }
            Err(e) => {
                log_output.lock().unwrap().push(format!("❌ Preflight check failed: {}", e));
                // Still need to complete the operation
                // Send a dummy result to unblock the UI
                let _ = tx.send(PreflightCheckResult {
                    release_name: "Error".to_string(),
                    generated_release_name: "Error".to_string(),
                    dupe_check: format!("Error: {}", e),
                    tmdb_id: 0,
                    imdb_id: None,
                    tvdb_id: None,
                    excluded_files: "N/A".to_string(),
                    album_cover: "N/A".to_string(),
                    audio_languages: Vec::new(),
                    release_type: "Error".to_string(),
                    season_number: None,
                    episode_number: None,
                    is_boxset: false,
                    tracker_categories: Vec::new(),
                    // IGDB fields
                    igdb_id: None,
                    igdb_genres: None,
                    igdb_developer: None,
                    igdb_publisher: None,
                    igdb_rating: None,
                    igdb_summary: None,
                    igdb_platforms: None,
                });
            }
        }
        
        // Log thread completion
        log_output.lock().unwrap().push("🏁 Preflight thread completed".to_string());
    });
    
    // Don't switch to log view, stay on main
    Ok(())
}

fn start_upload(state: &mut AppState) -> io::Result<()> {
    if state.input_path.is_none() || state.selected_trackers.is_empty() {
        state.add_log("Error: Input path and trackers must be selected".to_string());
        return Ok(());
    }

    state.upload_running = true;
    state.clear_logs();
    state.add_log("Starting upload process...".to_string());
    
    let input_path = state.input_path.clone().unwrap();
    let selected_trackers = state.selected_trackers.clone();
    let category_code = state.category_code.clone();
    let log_output = Arc::clone(&state.log_output);
    let dry_run = state.dry_run;
    
    // Load config for the upload
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            state.add_log(format!("❌ Failed to load config: {}", e));
            return Ok(());
        }
    };
    
    thread::spawn(move || {
        let path_str = input_path.to_string_lossy().to_string();
        
        // Log what we're doing
        log_output.lock().unwrap().push(format!("\n📁 Processing: {}", path_str));
        log_output.lock().unwrap().push(format!("🎯 Trackers: {}", selected_trackers.join(", ")));
        
        if dry_run {
            log_output.lock().unwrap().push("🚫 DRY-RUN MODE - No actual uploads will occur".to_string());
        }
        
        if let Some(ref code) = category_code {
            log_output.lock().unwrap().push(format!("🏷️  Category Code: {}", code));
        } else {
            log_output.lock().unwrap().push("🤖 Using auto-detection for media type".to_string());
        }
        
        // Process the upload using ProcessBuilder
        let result = if let Some(code) = category_code {
            // Parse the 4-digit code
            match parse_seedpool_category_type(&code) {
                Ok(torrent_info) => {
                    log_output.lock().unwrap().push(format!("📋 Classification: {}", torrent_info.description()));
                    
                    // Process with explicit category/type using ProcessBuilder
                    match process_builder::upload_builder(&path_str, std::sync::Arc::new(config.clone()))
                        .force_category(format!("{}Category::{}", 
                            if torrent_info.is_video_category() { "Video" }
                            else if torrent_info.is_audio_category() { "Audio" }
                            else if torrent_info.is_ebook_category() { "Ebook" }
                            else if torrent_info.is_game_category() { "Game" }
                            else { "Hobby" },
                            torrent_info.category_name()
                        ))
                        .dry_run(dry_run)
                        .build() {
                        Ok(_result) => Ok(()),
                        Err(e) => Err(e),
                    }
                }
                Err(e) => {
                    Err(format!("Failed to parse category code: {}", e))
                }
            }
        } else {
            // Auto-detect and process using ProcessBuilder
            match process_builder::upload_builder(&path_str, std::sync::Arc::new(config.clone()))
                .dry_run(dry_run)
                .build() {
                Ok(_result) => Ok(()),
                Err(e) => Err(e),
            }
        };
        
        // Handle result
        match result {
            Ok(_) => {
                if dry_run {
                    log_output.lock().unwrap().push("\n✅ Dry-run completed successfully!".to_string());
                    log_output.lock().unwrap().push("ℹ️  No files were actually uploaded".to_string());
                } else {
                    log_output.lock().unwrap().push("\n✅ Upload completed successfully!".to_string());
                }
                
                // Show success for each selected tracker
                for tracker in &selected_trackers {
                    if dry_run {
                        log_output.lock().unwrap().push(format!("  ✓ {} upload would succeed", tracker));
                    } else {
                        log_output.lock().unwrap().push(format!("  ✓ {} upload complete", tracker));
                    }
                }
            }
            Err(e) => {
                log_output.lock().unwrap().push(format!("\n❌ {} failed: {}", if dry_run { "Dry-run" } else { "Upload" }, e));
            }
        }
    });
    
    state.current_state = UIState::ViewingLog;
    Ok(())
}