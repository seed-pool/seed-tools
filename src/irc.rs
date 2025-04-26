use irc::client::Client;
use irc::client::prelude::*;
use std::io::{self, Write}; // Use `std::io` for synchronous I/O
use futures_util::stream::StreamExt;
use tokio::sync::{mpsc, Mutex};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use serde::Deserialize;
use std::{fs, sync::Arc};
use crate::types::SeedpoolGeneralConfig;
use std::collections::HashMap;

#[derive(Deserialize)]
struct SeedpoolConfig {
    general: SeedpoolGeneralConfig, // Use the struct from types.rs
}

pub async fn launch_irc_client() -> Result<(), Box<dyn std::error::Error>> {
    // Dynamically determine the config path relative to the executable directory
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or("Failed to determine executable directory")?
        .to_path_buf();
    let config_path = exe_dir.join("config/trackers/seedpool.yaml");

    // Load the Seedpool configuration from the YAML file
    let seedpool_config: SeedpoolConfig = serde_yaml::from_str(&fs::read_to_string(&config_path)?)?;
    let passkey = seedpool_config.general.passkey.clone(); // Get the passkey
    let username = seedpool_config.general.username.clone(); // Get the username

    // Create the IRC client configuration
    let config = Config {
        nickname: Some(username.clone()), // Use the username as the nickname
        server: Some("irc.seedpool.org".to_string()),
        port: Some(6697), // Specify the port for TLS
        use_tls: Some(true), // Enable TLS
        channels: vec!["#lobby".to_string()],
        ..Default::default()
    };

    // Create the IRC client
    let client = Arc::new(Mutex::new(Client::from_config(config.clone()).await?));
    client.lock().await.identify()?; // Identify the client with the server
    println!("Connected to the server as {}.", client.lock().await.current_nickname());

    // Create a stream for incoming messages
    let mut stream = client.lock().await.stream()?;

    // Set up the terminal UI
    enable_raw_mode()?; // Enable raw mode for terminal
    let mut stdout = std::io::stdout(); // Use synchronous `std::io::stdout()`
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?; // Enter alternate screen
    let backend = CrosstermBackend::new(stdout); // Create the backend for tui
    let mut terminal = Terminal::new(backend)?; // Create the terminal instance

    // Channels for communication
    let (tx_display, mut rx_display) = mpsc::channel::<(String, String)>(100); // (channel, message)
    let (tx_input, mut rx_input) = mpsc::channel::<String>(10);

    // Clone `client` and `passkey` for the task
    let client_clone = Arc::clone(&client);
    let passkey_clone = passkey.clone();

    // Spawn a task to handle incoming messages
    tokio::spawn(async move {
        while let Some(message) = stream.next().await {
            if let Ok(message) = message {
                // Check for the RPL_WELCOME response to send the passkey
                if let Command::Response(Response::RPL_WELCOME, _) = message.command {
                    if let Err(e) = client_clone.lock().await.send_privmsg("SeedServ", &passkey_clone) {
                        log::error!("Failed to send passkey to SeedServ: {}", e);
                    } else {
                        log::info!("Passkey sent to SeedServ.");
                    }
                }

                let formatted_message = match &message.command {
                    Command::PRIVMSG(target, content) => {
                        if let Some(nickname) = message.source_nickname() {
                            format!("[{}] {}: {}", target, nickname, content)
                        } else {
                            format!("[{}] [Unknown]: {}", target, content)
                        }
                    }
                    Command::JOIN(channel, ..) => {
                        if let Some(nickname) = message.source_nickname() {
                            format!("* {} has joined {}", nickname, channel)
                        } else {
                            format!("* Unknown has joined {}", channel)
                        }
                    }
                    Command::PART(channel, ..) => {
                        if let Some(nickname) = message.source_nickname() {
                            format!("* {} has left {}", nickname, channel)
                        } else {
                            format!("* Unknown has left {}", channel)
                        }
                    }
                    Command::QUIT(reason) => {
                        if let Some(nickname) = message.source_nickname() {
                            format!("* {} has quit ({})", nickname, reason.as_deref().unwrap_or("No reason"))
                        } else {
                            "* Unknown has quit".to_string()
                        }
                    }
                    _ => format!("* Unhandled command: {:?}", message.command),
                };

                // Determine the target channel for the message
                let target_channel = match &message.command {
                    Command::PRIVMSG(target, _) => target.clone(),
                    Command::JOIN(channel, ..) => channel.clone(),
                    _ => "#server".to_string(), // Default to server messages
                };

                if tx_display.send((target_channel, formatted_message)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Message history and input buffer
    let mut messages: HashMap<String, Vec<String>> = HashMap::new(); // Store messages by channel
    let mut input = String::new();
    let mut active_channel = "#lobby".to_string(); // Default to the first channel

    // Create a periodic timer for refreshing the UI
    let mut ui_refresh_interval = tokio::time::interval(std::time::Duration::from_millis(100));

    let mut command_history: Vec<String> = Vec::new(); // Store command history
    let mut history_position: Option<usize> = None; 

    // Main UI loop
    let result = loop {
        tokio::select! {
            // Periodic UI refresh
            _ = ui_refresh_interval.tick() => {
                terminal.draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                        .split(f.size());

                    // Create a longer-lived empty vector
                    let empty_vec = vec![];

                    // Display messages for the active channel
                    let message_spans: Vec<Spans> = messages
                        .get(&active_channel)
                        .unwrap_or(&empty_vec) // Use the longer-lived `empty_vec`
                        .iter()
                        .map(|msg| Spans::from(parse_irc_colors(msg)))
                        .collect();
                    let message_widget = Paragraph::new(message_spans)
                        .block(Block::default().borders(Borders::ALL).title(Span::raw(active_channel.clone())));
                    f.render_widget(message_widget, chunks[0]);

                    // Display input
                    let input_widget = Paragraph::new(input.as_ref())
                        .style(Style::default().fg(Color::Yellow))
                        .block(Block::default().borders(Borders::ALL).title("Input"));
                    f.render_widget(input_widget, chunks[1]);
                })?;
            }

            // Handle keypress events
            _ = async {
                if crossterm::event::poll(std::time::Duration::from_millis(10))? {
                    if let Event::Key(key) = crossterm::event::read()? {
                        match key.code {
                            KeyCode::Char(c) => {
                                input.push(c);
                                history_position = None; // Reset history navigation when typing
                            }
                            KeyCode::Backspace => {
                                input.pop();
                                history_position = None; // Reset history navigation when typing
                            }
                            KeyCode::Enter => {
                                if !input.is_empty() {
                                    command_history.push(input.clone()); // Save the command to history
                                    if command_history.len() > 100 {
                                        command_history.remove(0); // Limit history size to 100 commands
                                    }
                                }
                                if tx_input.send(input.clone()).await.is_err() {
                                    return Ok(());
                                }
                                input.clear();
                                history_position = None; // Reset history navigation
                            }
                            KeyCode::Up => {
                                if let Some(pos) = history_position {
                                    if pos > 0 {
                                        history_position = Some(pos - 1);
                                    }
                                } else if !command_history.is_empty() {
                                    history_position = Some(command_history.len() - 1);
                                }
                                if let Some(pos) = history_position {
                                    input = command_history[pos].clone();
                                }
                            }
                            KeyCode::Down => {
                                if let Some(pos) = history_position {
                                    if pos + 1 < command_history.len() {
                                        history_position = Some(pos + 1);
                                    } else {
                                        history_position = None;
                                        input.clear();
                                    }
                                }
                                if let Some(pos) = history_position {
                                    input = command_history[pos].clone();
                                }
                            }
                            KeyCode::Esc | KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                // Exit on Esc or Ctrl+C
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
                // Explicitly redraw the UI after handling input
                terminal.draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                        .split(f.size());
    
                    // Display messages for the active channel
                    let empty_vec = vec![];
                    let message_spans: Vec<Spans> = messages
                        .get(&active_channel)
                        .unwrap_or(&empty_vec)
                        .iter()
                        .map(|msg| Spans::from(parse_irc_colors(msg)))
                        .collect();
                    let message_widget = Paragraph::new(message_spans)
                        .block(Block::default().borders(Borders::ALL).title(Span::raw(active_channel.clone())));
                    f.render_widget(message_widget, chunks[0]);
    
                    // Display input
                    let input_widget = Paragraph::new(input.as_ref())
                        .style(Style::default().fg(Color::Yellow))
                        .block(Block::default().borders(Borders::ALL).title("Input"));
                    f.render_widget(input_widget, chunks[1]);
                })?;
                Ok::<(), Box<dyn std::error::Error>>(())
            } => {}

            // Handle incoming messages
            Some((channel, message)) = rx_display.recv() => {
                messages.entry(channel.clone()).or_insert_with(Vec::new).push(message);
                if let Some(channel_messages) = messages.get_mut(&channel) {
                    if channel_messages.len() > 100 {
                        channel_messages.remove(0); // Keep the message history manageable
                    }
                }
            }

            // Handle user input
            Some(input) = rx_input.recv() => {
                if input.starts_with("/") {
                    // Handle commands (e.g., /join, /msg, /part, /quit)
                    let parts: Vec<&str> = input.splitn(3, ' ').collect();
                    match parts.as_slice() {
                        ["/join", channel] => {
                            client.lock().await.send_join(channel)?;
                            active_channel = channel.to_string(); // Switch to the new channel
                            messages.entry(active_channel.clone()).or_insert_with(Vec::new);
                            messages.get_mut(&active_channel).unwrap().push(format!("Joined channel: {}", channel));
                        }
                        ["/msg", target, message] => {
                            client.lock().await.send_privmsg(target, message)?;
                            messages.entry(target.to_string()).or_insert_with(Vec::new).push(format!("You to {}: {}", target, message));
                        }
                        ["/part", channel] => {
                            client.lock().await.send_part(channel)?;
                            messages.entry(channel.to_string()).or_insert_with(Vec::new).push(format!("Left channel: {}", channel));
                        }
                        ["/quit"] => {
                            client.lock().await.send_quit("")?;
                            messages.entry("#server".to_string()).or_insert_with(Vec::new).push("Disconnected from the server.".to_string());
                            break Ok(()); // Explicitly return Ok(()) when breaking
                        }
                        _ => {
                            messages.entry(active_channel.clone()).or_insert_with(Vec::new).push(format!("Unknown command: {}", input));
                        }
                    }
                } else {
                    // Send input as a message to the active channel
                    client.lock().await.send_privmsg(&active_channel, &input)?;
                    messages.entry(active_channel.clone()).or_insert_with(Vec::new).push(format!("You: {}", input));
                }
            }
        }
    };

    // Restore the terminal before exiting
    if let Err(e) = restore_terminal(&mut terminal) {
        log::error!("Failed to restore terminal: {}", e); // Use logging instead of eprintln!
    }

    result
}

// Function to restore the terminal
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?; // Disable raw mode
    execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?; // Leave alternate screen
    terminal.show_cursor()?; // Show the cursor again
    Ok(())
}

// Helper function to parse IRC color codes
fn parse_irc_colors(message: &str) -> Vec<Span> {
    use tui::style::{Color, Style};
    let mut spans = Vec::new();
    let mut chars = message.chars().peekable();
    let mut current_text = String::new();
    let mut current_style = Style::default();

    while let Some(c) = chars.next() {
        match c {
            '\x03' => { // IRC color code
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                }

                let mut fg_color = None;
                let mut bg_color = None;

                if let Some(next) = chars.peek() {
                    if next.is_ascii_digit() {
                        let mut color_code = String::new();
                        color_code.push(chars.next().unwrap());
                        if let Some(next) = chars.peek() {
                            if next.is_ascii_digit() {
                                color_code.push(chars.next().unwrap());
                            }
                        }
                        fg_color = Some(map_irc_color(color_code.parse::<u8>().unwrap_or(0)));
                    }
                }

                if let Some(',') = chars.peek() {
                    chars.next();
                    if let Some(next) = chars.peek() {
                        if next.is_ascii_digit() {
                            let mut color_code = String::new();
                            color_code.push(chars.next().unwrap());
                            if let Some(next) = chars.peek() {
                                if next.is_ascii_digit() {
                                    color_code.push(chars.next().unwrap());
                                }
                            }
                            bg_color = Some(map_irc_color(color_code.parse::<u8>().unwrap_or(0)));
                        }
                    }
                }

                current_style = current_style
                    .fg(fg_color.unwrap_or(Color::Reset))
                    .bg(bg_color.unwrap_or(Color::Reset));
            }
            '\x02' => { // Bold
                current_style = current_style.add_modifier(tui::style::Modifier::BOLD);
            }
            '\x1F' => { // Underline
                current_style = current_style.add_modifier(tui::style::Modifier::UNDERLINED);
            }
            '\x0F' => { // Reset
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                }
                current_style = Style::default();
            }
            _ => {
                current_text.push(c);
            }
        }
    }

    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    spans
}

// Map IRC color codes (0–15) to TUI colors
fn map_irc_color(code: u8) -> Color {
    match code {
        0 => Color::White,        // White
        1 => Color::Black,        // Black
        2 => Color::Blue,         // Blue
        3 => Color::Green,        // Green
        4 => Color::Red,          // Red
        5 => Color::Rgb(165, 42, 42), // Brown (custom RGB)
        6 => Color::Magenta,      // Magenta
        7 => Color::Rgb(255, 165, 0), // Orange (custom RGB)
        8 => Color::Yellow,       // Yellow
        9 => Color::LightGreen,   // Light Green
        10 => Color::Cyan,        // Cyan
        11 => Color::LightCyan,   // Light Cyan
        12 => Color::LightBlue,   // Light Blue
        13 => Color::Rgb(255, 192, 203), // Pink (custom RGB)
        14 => Color::Gray,        // Grey
        15 => Color::Rgb(211, 211, 211), // Light Grey (custom RGB)
        _ => Color::Reset,        // Default reset color
    }
}