use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::collections::BTreeSet;
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
    selected_packages: BTreeSet<usize>,
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
            selected_packages: BTreeSet::new(),
            list_state,
            search_query: String::new(),
            package_details: String::new(),
            state: AppState::Loading,
            matcher: SkimMatcherV2::default(),
        }
    }

    async fn load_packages(&mut self) -> Result<(), Box<dyn Error>> {
        // Query all remote packages with descriptions from xbps repositories.
        let output = Command::new("xbps-query")
            .args(["-Rs", ""])
            .output()
            .await?;

        if !output.status.success() {
            return Err("Failed to run xbps-query -Rs".into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        // Typical xbps-query -Rs line format:
        // [*] package-name-version - description
        for line in stdout.lines() {
            let raw = line.trim();
            if raw.is_empty() {
                continue;
            }

            let normalized = if raw.starts_with('[') {
                match raw.find(']') {
                    Some(end) => raw[end + 1..].trim(),
                    None => raw,
                }
            } else {
                raw
            };

            if normalized.is_empty() {
                continue;
            }

            let (id_part, description) = match normalized.split_once(" - ") {
                Some((id, desc)) => (id.trim(), desc.trim().to_string()),
                None => {
                    let mut parts = normalized.split_whitespace();
                    let id = parts.next().unwrap_or("").trim();
                    let desc = parts.collect::<Vec<_>>().join(" ");
                    (id, desc)
                }
            };

            if id_part.is_empty() {
                continue;
            }

            let (name, version) = split_name_and_version(id_part);
            if name.is_empty() {
                continue;
            }

            packages.push(Package {
                name,
                version,
                description,
                repository: "void".to_string(),
            });
        }

        self.packages = packages;
        self.filter_packages();
        self.state = AppState::Browsing;
        Ok(())
    }

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
                    let selected_count = self.selected_packages.len();
                    let selected_hint = if self.selected_packages.contains(&pkg_idx) {
                        "yes"
                    } else {
                        "no"
                    };
                    self.package_details = format!(
                        "Package: {}\nVersion: {}\nRepository: {}\nSelected: {}\nMarked packages: {}\nDescription: {}",
                        pkg.name,
                        pkg.version,
                        pkg.repository,
                        selected_hint,
                        selected_count,
                        pkg.description
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
                        .args(["xbps-install", "-y", &pkg.name])
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
        let mut targets: Vec<usize> = self.selected_packages.iter().copied().collect();

        // If nothing is marked, install the currently highlighted package.
        if targets.is_empty() {
            if let Some(selected) = self.list_state.selected() {
                if let Some(&pkg_idx) = self.filtered_packages.get(selected) {
                    targets.push(pkg_idx);
                }
            }
        }

        if targets.is_empty() {
            return Ok(());
        }

        for pkg_idx in &targets {
            if let Some(pkg) = self.packages.get(*pkg_idx) {
                println!("Installing package: {}", pkg.name);
                println!("Description: {}", pkg.description);
                println!();

                let status = Command::new("sudo")
                    .args(["xbps-install", &pkg.name])
                    .spawn()?
                    .wait()
                    .await?;

                if !status.success() {
                    println!("\nInstallation failed while processing '{}'.", pkg.name);
                    println!("Press Enter to continue...");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    self.state = AppState::Error("Installation failed".to_string());
                    return Ok(());
                }
            }
        }

        println!("\nInstalled {} package(s) successfully!", targets.len());
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        self.selected_packages.clear();
        self.state = AppState::Browsing;
        Ok(())
    }

    fn toggle_selected_current(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&pkg_idx) = self.filtered_packages.get(selected) {
                if self.selected_packages.contains(&pkg_idx) {
                    self.selected_packages.remove(&pkg_idx);
                } else {
                    self.selected_packages.insert(pkg_idx);
                }
            }
        }
        self.update_package_details();
    }

    fn select_all_visible(&mut self) {
        for pkg_idx in &self.filtered_packages {
            self.selected_packages.insert(*pkg_idx);
        }
        self.update_package_details();
    }

    fn clear_selection(&mut self) {
        self.selected_packages.clear();
        self.update_package_details();
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

fn split_name_and_version(id: &str) -> (String, String) {
    if let Some(idx) = id.rfind('-') {
        let name = id[..idx].trim();
        let version = id[idx + 1..].trim();

        // XBPS versions contain digits (example: 2.43.0_1). If suffix
        // does not look like a version, keep the entire string as name.
        if !name.is_empty() && version.chars().any(|c| c.is_ascii_digit()) {
            return (name.to_string(), version.to_string());
        }
    }

    (id.to_string(), "unknown".to_string())
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
                        KeyCode::Char(' ') => {
                            if app.state == AppState::Browsing {
                                app.toggle_selected_current();
                            }
                        }
                        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if app.state == AppState::Browsing {
                                app.select_all_visible();
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if app.state == AppState::Browsing {
                                app.clear_selection();
                            }
                        }
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
            let marker = if app.selected_packages.contains(&i) {
                "[x] "
            } else {
                "[ ] "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
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
        AppState::Browsing => "↑/↓ or j/k: navigate | Space: mark | Ctrl+a: mark all visible | Ctrl+c: clear marks | Enter: install marked/current | q: quit | Type to search",
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
