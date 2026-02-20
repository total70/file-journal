use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{Datelike, Timelike};

#[derive(Parser)]
#[command(name = "file-journal")]
#[command(about = "A CLI for creating journal entries")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new journal entry
    New {
        /// The title for the journal entry (should end with .md)
        title: String,
        /// The note content to store in the file
        note: Option<String>,
        /// Override the default journal path
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Initialize a new journal configuration
    Init {
        /// Path to the journal directory
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Get journal entries for a specific date
    Get {
        /// Day of month (1-31), defaults to today if not specified
        #[arg(short, long)]
        day: Option<u32>,
        /// Month (1-12), defaults to current month if not specified
        #[arg(short, long)]
        month: Option<u32>,
        /// Year (e.g., 2024), defaults to current year if not specified
        #[arg(short, long)]
        year: Option<i32>,
        /// Get entries for the current week (overrides day/month)
        #[arg(long, conflicts_with = "day")]
        week: bool,
        /// Override the default journal path
        #[arg(short, long)]
        path: Option<PathBuf>,
        /// Output format: 'paths' (default), 'content', or 'json'
        #[arg(short, long, default_value = "paths")]
        format: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Config {
    /// Default journal path
    pub default_path: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { title, note, path } => create_entry(title, note, path, cli.config),
        Commands::Init { path } => init_config(path),
        Commands::Get { day, month, year, week, path, format } => {
            get_entries(day, month, year, week, path, cli.config, format)
        }
    }
}

fn load_config(config_path: Option<PathBuf>) -> Option<Config> {
    // If config path is specified, use that
    if let Some(path) = config_path {
        if path.exists() {
            let content = fs::read_to_string(&path).ok()?;
            return toml::from_str(&content).ok();
        }
        return None;
    }

    // Try current directory .file-journal.toml
    let local_config = Path::new(".file-journal.toml");
    if local_config.exists() {
        let content = fs::read_to_string(local_config).ok()?;
        return toml::from_str(&content).ok();
    }

    // Try home directory ~/.config/file-journal/config.toml
    if let Some(home) = dirs::home_dir() {
        let home_config = home.join(".config").join("file-journal").join("config.toml");
        if home_config.exists() {
            let content = fs::read_to_string(&home_config).ok()?;
            return toml::from_str(&content).ok();
        }
    }

    None
}

fn get_journal_path(explicit_path: Option<PathBuf>, config: Option<Config>) -> Option<PathBuf> {
    // Explicit path takes priority
    if let Some(path) = explicit_path {
        return Some(path);
    }

    // Then config default_path
    if let Some(cfg) = config {
        if let Some(path) = cfg.default_path {
            return Some(path);
        }
    }

    None
}

fn resolve_target_dir(journal_path: PathBuf) -> Result<PathBuf, String> {
    let now = chrono::Local::now();
    let year = now.year().to_string();
    let month = format!("{:02}", now.month());
    let _day = now.day();

    // Build path: journal_path/YYYY/MM
    let target_dir = journal_path.join(&year).join(&month);

    // Create directories if they don't exist
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    // Validate the structure
    let month_folder = target_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Failed to get month folder name")?;

    if !is_valid_month(month_folder) {
        return Err(format!("Invalid month folder: {}", month_folder));
    }

    let year_folder = target_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|name| name.to_str())
        .ok_or("Failed to get year folder name")?;

    if !is_valid_year(year_folder) {
        return Err(format!("Invalid year folder: {}", year_folder));
    }

    Ok(target_dir)
}

fn create_entry(title: String, note: Option<String>, path: Option<PathBuf>, config_path: Option<PathBuf>) {
    // Check if title ends with .md
    if !title.ends_with(".md") {
        eprintln!("Error: Title must end with .md");
        std::process::exit(1);
    }

    // Load config
    let config = load_config(config_path);

    // Determine journal path
    let journal_path = match get_journal_path(path, config) {
        Some(p) => p,
        None => {
            // Fall back to current directory
            env::current_dir().expect("Failed to get current directory")
        }
    };

    // Resolve target directory (create year/month folders if needed)
    let target_dir = match resolve_target_dir(journal_path) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let now = chrono::Local::now();
    let year = now.year();
    let month = now.month();
    let day = now.day();
    let hour = now.hour();
    let minute = now.minute();
    let second = now.second();

    // Create filename: dd-HHMMSS-<title>.md for easy sorting
    let title_part = title.trim_end_matches(".md");
    let safe_title = sanitize_title(title_part);
    let filename = format!("{:02}-{:02}{:02}{:02}-{}.md", day, hour, minute, second, safe_title);
    let filepath = target_dir.join(&filename);

    // Check if file already exists
    if filepath.exists() {
        eprintln!("Error: File '{}' already exists", filename);
        std::process::exit(1);
    }

    // Create the file with a template (DD-MM-YYYY format)
    let note_content = note.unwrap_or_default();
    let template = format!(
        "# {}\n\nDate: {:02}-{:02}-{}\n\n{}\n",
        title.trim_end_matches(".md"),
        day,
        month,
        year,
        note_content
    );

    fs::write(&filepath, template).expect("Failed to create file");

    println!("Created journal entry: {}", filepath.display());
}

fn get_entries(
    day: Option<u32>,
    month: Option<u32>,
    year: Option<i32>,
    week: bool,
    path: Option<PathBuf>,
    config_path: Option<PathBuf>,
    format: String,
) {
    // Load config
    let config = load_config(config_path);

    // Determine journal path
    let journal_path = match get_journal_path(path, config) {
        Some(p) => p,
        None => {
            eprintln!("Error: No journal path specified. Use --path or set up config with 'init'");
            std::process::exit(1);
        }
    };

    // Debug output

    let entries = if week {
        match find_entries_week(&journal_path) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match find_entries(&journal_path, day, month, year) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Debug output
    for entry in &entries {
    }

    // Output results
    match format.as_str() {
        "json" => {
            let paths: Vec<String> = entries.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            match serde_json::to_string(&paths) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Error: Failed to serialize to JSON: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "content" => {
            for entry in &entries {
                println!("{}", entry.display());
                println!("{}", "-".repeat(40));
                match fs::read_to_string(entry) {
                    Ok(content) => println!("{}", content),
                    Err(e) => eprintln!("Error reading {}: {}", entry.display(), e),
                }
                println!();
            }
        }
        _ => {
            // Default: just paths
            for entry in &entries {
                println!("{}", entry.display());
            }
        }
    }

    // Exit with error code if no entries found (useful for scripts)
    if entries.is_empty() {
        std::process::exit(1);
    }
}

fn init_config(path: Option<PathBuf>) {
    let config_path = if let Some(p) = path {
        p
    } else if let Some(home) = dirs::home_dir() {
        home.join(".config").join("file-journal").join("config.toml")
    } else {
        eprintln!("Error: Could not determine config path");
        std::process::exit(1);
    };

    // Ask for default journal path
    println!("Enter the default journal path (e.g., /Users/t/Documents/journal):");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");
    let default_path = PathBuf::from(input.trim());

    let config = Config {
        default_path: Some(default_path),
    };

    // Create parent directories if needed
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create config directory");
    }

    let toml_string = toml::to_string_pretty(&config).expect("Failed to serialize config");
    fs::write(&config_path, toml_string).expect("Failed to write config");

    println!("Created config at: {}", config_path.display());
}

fn is_valid_month(folder_name: &str) -> bool {
    if folder_name.len() != 2 {
        return false;
    }

    match folder_name.parse::<u32>() {
        Ok(month) => month >= 1 && month <= 12,
        Err(_) => false,
    }
}

fn is_valid_year(folder_name: &str) -> bool {
    if folder_name.len() != 4 {
        return false;
    }

    folder_name.parse::<u32>().is_ok()
}

fn sanitize_title(title: &str) -> String {
    let mut safe = title
        .replace(' ', "-")
        .replace('/', "-")
        .replace('\\', "-")
        .replace(':', "-")
        .replace('?', "-")
        .replace('*', "-")
        .replace('"', "-")
        .replace('\'', "-")
        .replace('<', "-")
        .replace('>', "-")
        .replace('|', "-");

    // Collapse multiple hyphens
    while safe.contains("--") {
        safe = safe.replace("--", "-");
    }

    // Trim trailing hyphen
    safe.trim_end_matches('-').to_string()
}

/// Find journal entries matching the given criteria
fn find_entries(
    journal_path: &Path,
    day: Option<u32>,
    month: Option<u32>,
    year: Option<i32>,
) -> Result<Vec<PathBuf>, String> {
    let now = chrono::Local::now();
    let target_year = year.unwrap_or(now.year());
    let target_month = month.unwrap_or(now.month());
    let target_day = day;

    // Build search path
    let year_dir = journal_path.join(target_year.to_string());
    
    // Determine the search directory based on what was specified
    let search_dir = if year.is_some() && day.is_none() && month.is_none() {
        // Just year specified - search from year directory
        year_dir.clone()
    } else {
        // For today's entries (no params) or when day/month specified, use month directory
        year_dir.join(format!("{:02}", target_month))
    };

    // Collect matching entries
    let mut entries = Vec::new();

    if let Some(day_val) = target_day {
        // Looking for specific day
        let day_prefix = format!("{:02}", day_val);
        if let Ok(files) = fs::read_dir(&search_dir) {
            for file in files.flatten() {
                if let Some(filename) = file.file_name().to_str() {
                    if filename.starts_with(&day_prefix) && filename.ends_with(".md") {
                        entries.push(file.path());
                    }
                }
            }
        }
    } else if month.is_some() {
        // Looking for entire month - read all .md files in month dir
        if let Ok(files) = fs::read_dir(&search_dir) {
            for file in files.flatten() {
                if let Some(filename) = file.file_name().to_str() {
                    if filename.ends_with(".md") {
                        entries.push(file.path());
                    }
                }
            }
        }
    } else if year.is_some() {
        // Looking for entire year - iterate all months from year directory
        for m in 1..=12 {
            let month_dir = year_dir.join(format!("{:02}", m));
            if month_dir.exists() {
                if let Ok(files) = fs::read_dir(&month_dir) {
                    for file in files.flatten() {
                        if let Some(filename) = file.file_name().to_str() {
                            if filename.ends_with(".md") {
                                entries.push(file.path());
                            }
                        }
                    }
                }
            }
        }
    } else {
        // Default: today's entries
        let day_prefix = format!("{:02}", now.day());
        if let Ok(files) = fs::read_dir(&search_dir) {
            for file in files.flatten() {
                if let Some(filename) = file.file_name().to_str() {
                    if filename.starts_with(&day_prefix) && filename.ends_with(".md") {
                        entries.push(file.path());
                    }
                }
            }
        }
    }

    // Sort entries by path for consistent ordering
    entries.sort();
    Ok(entries)
}

/// Find journal entries for the current week (Monday to Sunday)
fn find_entries_week(journal_path: &Path) -> Result<Vec<PathBuf>, String> {
    let now = chrono::Local::now();
    let weekday = now.weekday().num_days_from_monday(); // 0 = Monday, 6 = Sunday
    
    // Calculate start of week (Monday)
    let start_of_week = now - chrono::Duration::days(weekday as i64);
    let start_day = start_of_week.day();
    let start_month = start_of_week.month();
    let start_year = start_of_week.year();
    
    // Calculate end of week (Sunday)
    let end_of_week = start_of_week + chrono::Duration::days(6);
    let end_day = end_of_week.day();
    let end_month = end_of_week.month();
    let end_year = end_of_week.year();
    
    let mut entries = Vec::new();
    
    // Helper function to collect entries from a specific day
    let mut collect_entries_for_day = |year: i32, month: u32, day: u32| {
        let month_dir = journal_path.join(year.to_string()).join(format!("{:02}", month));
        if month_dir.exists() {
            let day_prefix = format!("{:02}", day);
            if let Ok(files) = fs::read_dir(&month_dir) {
                for file in files.flatten() {
                    if let Some(filename) = file.file_name().to_str() {
                        if filename.starts_with(&day_prefix) && filename.ends_with(".md") {
                            entries.push(file.path());
                        }
                    }
                }
            }
        }
    };
    
    // Collect entries from start of week to end of week
    if start_year == end_year && start_month == end_month {
        // Same month - iterate days
        for day in start_day..=end_day {
            collect_entries_for_day(start_year, start_month, day);
        }
    } else {
        // Week spans multiple months
        // First, collect from start day to end of start month
        let days_in_start_month = match start_month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if (start_year % 4 == 0 && start_year % 100 != 0) || (start_year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        };
        
        for day in start_day..=days_in_start_month {
            collect_entries_for_day(start_year, start_month, day);
        }
        
        // Then collect from start of end month to end day
        for day in 1..=end_day {
            collect_entries_for_day(end_year, end_month, day);
        }
    }
    
    // Sort entries by path for consistent ordering
    entries.sort();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_valid_month_valid() {
        assert!(is_valid_month("01"));
        assert!(is_valid_month("06"));
        assert!(is_valid_month("12"));
    }

    #[test]
    fn test_is_valid_month_invalid() {
        assert!(!is_valid_month("00"));
        assert!(!is_valid_month("13"));
        assert!(!is_valid_month("1"));   // too short
        assert!(!is_valid_month("001")); // too long
        assert!(!is_valid_month("ab"));  // not a number
        assert!(!is_valid_month(""));    // empty
    }

    #[test]
    fn test_is_valid_year_valid() {
        assert!(is_valid_year("2024"));
        assert!(is_valid_year("2025"));
        assert!(is_valid_year("2026"));
        assert!(is_valid_year("1999"));
        assert!(is_valid_year("0001"));
    }

    #[test]
    fn test_is_valid_year_invalid() {
        assert!(!is_valid_year("202"));   // too short
        assert!(!is_valid_year("20245")); // too long
        assert!(!is_valid_year("abcd"));  // not a number
        assert!(!is_valid_year(""));      // empty
        assert!(!is_valid_year("2a24"));  // mixed
    }

    #[test]
    fn test_sanitize_title() {
        assert_eq!(sanitize_title("my daily notes"), "my-daily-notes");
        assert_eq!(sanitize_title("test: file/name"), "test-file-name");
        assert_eq!(sanitize_title("my/note: about something?"), "my-note-about-something");
        assert_eq!(sanitize_title("hello world"), "hello-world");
        assert_eq!(sanitize_title("file*name"), "file-name");
        assert_eq!(sanitize_title("test<path>"), "test-path");
        assert_eq!(sanitize_title("a|b|c"), "a-b-c");
        assert_eq!(sanitize_title("multi--hyphens"), "multi-hyphens");
        assert_eq!(sanitize_title("trailing?"), "trailing");
        assert_eq!(sanitize_title("?leading"), "-leading"); // leading is kept
    }

    #[test]
    fn test_filename_format_with_timestamp() {
        // Test that filename format is: dd-HHMMSS-title.md
        let day = 17u32;
        let hour = 8u32;
        let minute = 15u32;
        let second = 3u32;
        let title = "niet-lekker-geslapen.md";
        let title_part = title.trim_end_matches(".md");
        let safe_title = sanitize_title(title_part);
        let filename = format!("{:02}-{:02}{:02}{:02}-{}.md", day, hour, minute, second, safe_title);
        assert_eq!(filename, "17-081503-niet-lekker-geslapen.md");
    }

    #[test]
    fn test_date_format_in_template() {
        // Test that date format in file is DD-MM-YYYY
        let day = 17u32;
        let month = 2u32;
        let year = 2026i32;
        let title = "test-entry";
        let note_content = "Test note content";
        
        let template = format!(
            "# {}\n\nDate: {:02}-{:02}-{}\n\n{}\n",
            title,
            day,
            month,
            year,
            note_content
        );
        
        let expected = "# test-entry\n\nDate: 17-02-2026\n\nTest note content\n";
        assert_eq!(template, expected);
        assert!(template.contains("Date: 17-02-2026"));
    }

    // Tests for find_entries functionality
    fn create_test_journal_dir() -> tempfile::TempDir {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        
        // Create structure: 2026/02/ and 2026/03/
        let month_02 = temp_dir.path().join("2026").join("02");
        let month_03 = temp_dir.path().join("2026").join("03");
        let month_01_2025 = temp_dir.path().join("2025").join("01");
        
        fs::create_dir_all(&month_02).expect("Failed to create month dir");
        fs::create_dir_all(&month_03).expect("Failed to create month dir");
        fs::create_dir_all(&month_01_2025).expect("Failed to create month dir");
        
        // Create test entries for Feb 17, 2026
        fs::write(
            month_02.join("17-081503-note1.md"),
            "# Note 1\n\nDate: 17-02-2026\n\nContent 1"
        ).expect("Failed to write note");
        fs::write(
            month_02.join("17-101200-note2.md"),
            "# Note 2\n\nDate: 17-02-2026\n\nContent 2"
        ).expect("Failed to write note");
        fs::write(
            month_02.join("18-090000-note3.md"),
            "# Note 3\n\nDate: 18-02-2026\n\nContent 3"
        ).expect("Failed to write note");
        
        // Create test entry for March 1, 2026
        fs::write(
            month_03.join("01-120000-march-note.md"),
            "# March Note\n\nDate: 01-03-2026\n\nMarch content"
        ).expect("Failed to write note");
        
        // Create test entry for Jan 2025
        fs::write(
            month_01_2025.join("15-080000-2025-note.md"),
            "# 2025 Note\n\nDate: 15-01-2025\n\n2025 content"
        ).expect("Failed to write note");
        
        temp_dir
    }

    #[test]
    fn test_find_entries_by_day() {
        let temp_dir = create_test_journal_dir();
        let entries = find_entries(temp_dir.path(), Some(17), Some(2), Some(2026))
            .expect("Failed to find entries");
        
        assert_eq!(entries.len(), 2);
        assert!(entries[0].to_string_lossy().contains("17-081503-note1.md"));
        assert!(entries[1].to_string_lossy().contains("17-101200-note2.md"));
    }

    #[test]
    fn test_find_entries_by_month() {
        let temp_dir = create_test_journal_dir();
        let entries = find_entries(temp_dir.path(), None, Some(2), Some(2026))
            .expect("Failed to find entries");
        
        assert_eq!(entries.len(), 3);
        // Should include all Feb entries (17th and 18th)
        let filenames: Vec<String> = entries.iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(filenames.iter().any(|f| f.contains("note1")));
        assert!(filenames.iter().any(|f| f.contains("note2")));
        assert!(filenames.iter().any(|f| f.contains("note3")));
    }

    #[test]
    fn test_find_entries_by_year() {
        let temp_dir = create_test_journal_dir();
        let entries = find_entries(temp_dir.path(), None, None, Some(2026))
            .expect("Failed to find entries");
        
        assert_eq!(entries.len(), 4);
        // Should include all 2026 entries (Feb and March)
        let filenames: Vec<String> = entries.iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(filenames.iter().any(|f| f.contains("note1")));
        assert!(filenames.iter().any(|f| f.contains("note2")));
        assert!(filenames.iter().any(|f| f.contains("note3")));
        assert!(filenames.iter().any(|f| f.contains("march-note")));
    }

    #[test]
    fn test_find_entries_cross_year() {
        let temp_dir = create_test_journal_dir();
        let entries_2025 = find_entries(temp_dir.path(), None, None, Some(2025))
            .expect("Failed to find entries");
        
        assert_eq!(entries_2025.len(), 1);
        assert!(entries_2025[0].to_string_lossy().contains("2025-note"));
    }

    #[test]
    fn test_find_entries_empty_result() {
        let temp_dir = create_test_journal_dir();
        let entries = find_entries(temp_dir.path(), Some(25), Some(2), Some(2026))
            .expect("Failed to find entries");
        
        assert!(entries.is_empty());
    }

    #[test]
    fn test_find_entries_different_day_same_month() {
        let temp_dir = create_test_journal_dir();
        let entries = find_entries(temp_dir.path(), Some(18), Some(2), Some(2026))
            .expect("Failed to find entries");
        
        assert_eq!(entries.len(), 1);
        assert!(entries[0].to_string_lossy().contains("note3"));
    }
}
