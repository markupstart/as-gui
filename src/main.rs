use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io, time::Duration, thread};
use tokio::process::Command;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

#[derive(Debug, Clone)]
struct Package {
    name: String,
    version: String,
    description: String,
    repository: String,
}

#[derive(PartialEq)]
enum AppState {
    Loading,
    Browsing,
    Installing,
    Error(String),
}

struct App {
    packages: Vec<Package>,
    filtered_packages: Vec<usize>,
    list_state: ListState,
    search_query: String,
    package_details: String,
    state: AppState,
    matcher: SkimMatcherV2,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        Self {
            packages: Vec::new(),
            filtered_packages: Vec::new(),
            list_state,
            search_query: String::new(),
            package_details: String::new(),
            state: AppState::Loading,
            matcher: SkimMatcherV2::default(),
        }
    }

    async fn load_packages(&mut self) -> Result<(), Box<dyn Error>> {
        // Use pacman -Ss instead of -Sl for better performance and descriptions
        let output = Command::new("pacman")
            .args(["-Ss", ""])
            .output()
            .await?;

        if !output.status.success() {
            return Err("Failed to run pacman -Ss".into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();
        let mut current_package: Option<Package> = None;

        // Parse packages from -Ss output (different format than -Sl)
        for line in stdout.lines() {
            if line.starts_with(' ') {
                // This is a description line
                if let Some(ref mut pkg) = current_package {
                    pkg.description = line.trim().to_string();
                    packages.push(pkg.clone());
                    current_package = None;
                }
            } else {
                // This is a package line: repo/name version [status]
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let repo_name = parts[0];
                    let version = parts[1];
                    
                    if let Some(slash_pos) = repo_name.find('/') {
                        let repository = repo_name[..slash_pos].to_string();
                        let name = repo_name[slash_pos + 1..].to_string();
                        
                        current_package = Some(Package {
                            name,
                            version: version.to_string(),
                            description: "Loading...".to_string(),
                            repository,
                        });
                    }
                }
            }
        }
        
        // Handle last package if no description follows
        if let Some(pkg) = current_package {
            packages.push(pkg);
        }

        self.packages = packages;
        self.filter_packages();
        self.state = AppState::Browsing;
        Ok(())
    }

    // Remove this function as it's no longer needed with pacman -Ss

    fn filter_packages(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_packages = (0..self.packages.len()).collect();
        } else {
            let query = self.search_query.to_lowercase();
            let query_words: Vec<&str> = query.split_whitespace().collect();
            
            let mut results: Vec<(usize, u32)> = self.packages
                .iter()
                .enumerate()
                .filter_map(|(i, pkg)| {
                    let pkg_name_lower = pkg.name.to_lowercase();
                    let pkg_desc_lower = pkg.description.to_lowercase();
                    
                    // Check if any query word matches the package name or description
                    let matches = query_words.iter().any(|&word| {
                        // Simple substring matching (fast and intuitive)
                        pkg_name_lower.contains(word) || pkg_desc_lower.contains(word) ||
                        // Fuzzy matching as fallback for typos
                        self.matcher.fuzzy_match(&pkg_name_lower, word).is_some() ||
                        self.matcher.fuzzy_match(&pkg_desc_lower, word).is_some()
                    });
                    
                    if matches {
                        Some((i, self.calculate_relevance_score(pkg, &query_words)))
                    } else {
                        None
                    }
                })
                .collect();
            
            // Sort by relevance score (higher score = more relevant)
            results.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered_packages = results.into_iter().map(|(i, _)| i).collect();
        }
        
        if !self.filtered_packages.is_empty() {
            self.list_state.select(Some(0));
            self.update_package_details();
        } else {
            self.list_state.select(None);
            self.package_details = "No packages found matching your search.".to_string();
        }
    }
    
    fn calculate_relevance_score(&self, pkg: &Package, query_words: &[&str]) -> u32 {
        let mut score = 0u32;
        let pkg_name_lower = pkg.name.to_lowercase();
        let pkg_desc_lower = pkg.description.to_lowercase();
        
        for &word in query_words {
            // Exact name match gets highest score
            if pkg_name_lower == word {
                score += 1000;
            }
            // Name starts with query gets high score
            else if pkg_name_lower.starts_with(word) {
                score += 500;
            }
            // Name contains query gets medium score
            else if pkg_name_lower.contains(word) {
                score += 100;
            }
            // Description contains query gets lower score
            else if pkg_desc_lower.contains(word) {
                score += 10;
            }
            // Fuzzy match gets minimal score
            else if self.matcher.fuzzy_match(&pkg_name_lower, word).is_some() {
                score += 1;
            }
        }
        
        score
    }

    fn update_package_details(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&pkg_idx) = self.filtered_packages.get(selected) {
                if let Some(pkg) = self.packages.get(pkg_idx) {
                    self.package_details = format!(
                        "Package: {}\nVersion: {}\nRepository: {}\nDescription: {}",
                        pkg.name, pkg.version, pkg.repository, pkg.description
                    );
                }
            }
        }
    }

    async fn install_selected_package(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&pkg_idx) = self.filtered_packages.get(selected) {
                if let Some(pkg) = self.packages.get(pkg_idx) {
                    self.state = AppState::Installing;
                    
                    let status = Command::new("sudo")
                        .args(["pacman", "-S", "--noconfirm", &pkg.name])
                        .status()
                        .await?;

                    if status.success() {
                        self.state = AppState::Browsing;
                    } else {
                        self.state = AppState::Error("Installation failed".to_string());
                    }
                }
            }
        }
        Ok(())
    }

    async fn install_selected_package_interactive(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&pkg_idx) = self.filtered_packages.get(selected) {
                if let Some(pkg) = self.packages.get(pkg_idx) {
                    println!("Installing package: {}", pkg.name);
                    println!("Description: {}", pkg.description);
                    println!();
                    
                    let status = Command::new("sudo")
                        .args(["pacman", "-S", &pkg.name])
                        .spawn()?
                        .wait()
                        .await?;

                    if status.success() {
                        println!("\nPackage '{}' installed successfully!", pkg.name);
                        println!("Press Enter to continue...");
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        self.state = AppState::Browsing;
                    } else {
                        println!("\nInstallation failed!");
                        println!("Press Enter to continue...");
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        self.state = AppState::Error("Installation failed".to_string());
                    }
                }
            }
        }
        Ok(())
    }

    fn next_package(&mut self) {
        if !self.filtered_packages.is_empty() {
            let i = match self.list_state.selected() {
                Some(i) => (i + 1) % self.filtered_packages.len(),
                None => 0,
            };
            self.list_state.select(Some(i));
            self.update_package_details();
        }
    }

    fn previous_package(&mut self) {
        if !self.filtered_packages.is_empty() {
            let i = match self.list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.filtered_packages.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.list_state.select(Some(i));
            self.update_package_details();
        }
    }

    fn add_char_to_search(&mut self, c: char) {
        self.search_query.push(c);
        self.filter_packages();
    }

    fn remove_char_from_search(&mut self) {
        self.search_query.pop();
        self.filter_packages();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new();
    
    // Load packages before setting up terminal to avoid blocking the UI
    println!("Loading packages... This may take a moment.");
    if let Err(e) = app.load_packages().await {
        eprintln!("Failed to load packages: {}", e);
        return Err(e);
    }
    println!("Packages loaded successfully. Starting GUI...");
    
    // Setup terminal after packages are loaded
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal with better error handling
    let _ = disable_raw_mode(); // Don't fail if already disabled
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    if let Err(err) = result {
        println!("Error: {}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    use crossterm::event::poll;
    
    loop {
        terminal.draw(|f| ui(f, app))?;

        // Check for input with timeout for non-blocking
        if poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Down | KeyCode::Char('j') => app.next_package(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous_package(),
                        KeyCode::Enter => {
                            if app.state == AppState::Browsing {
                                // Temporarily exit TUI mode for installation
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;
                                terminal.show_cursor()?;
                                
                                // Perform installation
                                let install_result = app.install_selected_package_interactive().await;
                                
                                // Restore TUI mode with better error handling
                                // Small delay to ensure terminal state is settled
                                thread::sleep(Duration::from_millis(50));
                                
                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                io::Write::flush(terminal.backend_mut())?;
                                terminal.hide_cursor()?;
                                terminal.clear()?;
                                
                                if let Err(e) = install_result {
                                    app.state = AppState::Error(format!("Installation error: {}", e));
                                }
                            }
                        }
                        KeyCode::Backspace => app.remove_char_from_search(),
                        KeyCode::Char(c) => {
                            if app.state == AppState::Browsing {
                                app.add_char_to_search(c);
                            }
                        }
                        KeyCode::Esc => {
                            if matches!(app.state, AppState::Error(_)) {
                                app.state = AppState::Browsing;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(f.size());

    // Search bar
    let search_block = Block::default()
        .title("Search (type to filter packages)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let search_paragraph = Paragraph::new(app.search_query.as_str())
        .block(search_block)
        .style(Style::default().fg(Color::White));
    f.render_widget(search_paragraph, chunks[0]);

    // Package list
    let items: Vec<ListItem> = app
        .filtered_packages
        .iter()
        .map(|&i| {
            let pkg = &app.packages[i];
            ListItem::new(Line::from(vec![
                Span::styled(&pkg.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(&pkg.version, Style::default().fg(Color::Green)),
                Span::raw(" - "),
                Span::styled(&pkg.description, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let list_title = format!("Packages ({} of {})", app.filtered_packages.len(), app.packages.len());
    let packages_list = List::new(items)
        .block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(packages_list, chunks[1], &mut app.list_state.clone());

    // Package details
    let details_block = Block::default()
        .title("Package Details")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let details_paragraph = Paragraph::new(app.package_details.as_str())
        .block(details_block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));
    f.render_widget(details_paragraph, chunks[2]);

    // Status bar
    let status_text = match &app.state {
        AppState::Loading => "Loading packages...",
        AppState::Browsing => "↑/↓ or j/k: navigate | Enter: install | q: quit | Type to search",
        AppState::Installing => "Installing package...",
        AppState::Error(msg) => msg,
    };
    
    let status_color = match &app.state {
        AppState::Loading => Color::Yellow,
        AppState::Browsing => Color::Green,
        AppState::Installing => Color::Blue,
        AppState::Error(_) => Color::Red,
    };

    let status_paragraph = Paragraph::new(status_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(status_color)),
        )
        .style(Style::default().fg(status_color));
    f.render_widget(status_paragraph, chunks[3]);

    // Error popup
    if let AppState::Error(_) = app.state {
        let popup_area = centered_rect(60, 20, f.size());
        f.render_widget(Clear, popup_area);
        let error_popup = Paragraph::new("Press ESC to continue")
            .block(
                Block::default()
                    .title("Error")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .style(Style::default().fg(Color::Red));
        f.render_widget(error_popup, popup_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
