mod calendar;
mod db;
mod ui;
mod edit_event;

use calendar::Calendar;
use chrono::{Datelike, Local};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Command line arguments for the calendar application
#[derive(Parser)]
#[command(name = "calendar")]
#[command(about = "Display a calendar for a given year/month")]
struct Args {
    /// Show calendar for the entire year
    #[arg(short = 'y', long = "year")]
    show_year: bool,

    /// Year or month to display (can be either a year > 12 or a month 1-12)
    #[arg(value_name = "YEAR_OR_MONTH")]
    first_arg: Option<String>,

    /// Month to display (1-12) when first argument is a year
    #[arg(value_name = "MONTH")]
    second_arg: Option<String>,

    /// Show only one month instead of the default three
    #[arg(short = 's', long = "single-month")]
    single_month: bool,
    
    /// Run in interactive mode with ncurses UI
    #[arg(short = 'i', long = "interactive", action = clap::ArgAction::SetTrue)]
    interactive: bool,
    
    /// Custom path for SQLite database file (optional)
    #[arg(short = 'd', long = "database")]
    db_path: Option<String>,
}

/// Entry point of the calendar application
/// 
/// # Description
/// Parses command line arguments and displays calendar(s) based on the provided options:
/// - Can show an entire year
/// - Can show a single month
/// - Can show three consecutive months (default)
/// - Supports years from 1583 onwards
/// 
/// # Arguments
/// Command line arguments are parsed using the `Args` struct
/// 
/// # Examples
/// ```bash
/// # Show current month and adjacent months
/// calendar
/// 
/// # Show entire year
/// calendar -y 2024
/// 
/// # Show specific month
/// calendar 2024 12
/// 
/// # Show single month instead of three
/// calendar -s
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    if args.interactive {
        // Run in interactive mode with ncurses UI
        let db = Arc::new(Mutex::new(db::Database::connect(args.db_path.as_deref()).await?));
        let mut ui = ui::CalendarUI::new(db);
        
        ui.init().await?;
        let result = ui.run().await;
        ui.cleanup();
        
        return result.map_err(|e| e.into());
    }
    let now = Local::now();
    let date = now.date_naive();

    // Process the first argument to determine if it's a year or month
    let has_first_arg = args.first_arg.is_some();
    let (year, month) = if let Some(first_arg) = args.first_arg {
        match first_arg.parse::<u16>() {
            Ok(num) if num > 12 => (
                Some(num),
                args.second_arg.clone().and_then(|m| m.parse().ok()),
            ),
            Ok(num) => (None, Some(num as u8)),
            Err(_) => {
                eprintln!("Error: Invalid number format");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    let single = args.single_month;
    let year = year.unwrap_or(date.year() as u16);

    if year < 1583 {
        eprintln!("Error: Year must be 1583 or later");
        std::process::exit(1);
    }

    if args.show_year || (has_first_arg && args.second_arg.is_none() && year != date.year() as u16)
    {
        Calendar::print_entire_year(year);
    } else if let Some(month) = month {
        if !(1..=12).contains(&month) {
            eprintln!("Error: Month must be between 1 and 12");
            std::process::exit(1);
        }

        let cal = Calendar {
            year,
            month: month - 1,
        };
        if single {
            Calendar::print_single_month(cal);
        } else {
            cal.print();
        }
    } else {
        let cal = Calendar {
            year,
            month: now.month0() as u8,
        };
        if single {
            Calendar::print_single_month(cal);
        } else {
            cal.print();
        }
    }
    
    Ok(())
}
