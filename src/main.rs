mod calendar;
use calendar::Calendar;
use chrono::{Datelike, Local};
use clap::Parser;

#[derive(Parser)]
#[command(name = "calendar")]
#[command(about = "Display a calendar for a given year/month")]
struct Args {
    /// Show calendar for the entire year
    #[arg(short = 'y', long = "year")]
    show_year: bool,

    /// Year or month to display
    #[arg(value_name = "YEAR_OR_MONTH")]
    first_arg: Option<String>,

    /// Month to display (1-12)
    #[arg(value_name = "MONTH")]
    second_arg: Option<String>,
}

fn main() {
    let args = Args::parse();
    let now = Local::now();
    let date = now.date_naive();

    // Process the first argument to determine if it's a year or month
    let has_first_arg = args.first_arg.is_some();
    let (year, month) = if let Some(first_arg) = args.first_arg {
        match first_arg.parse::<u16>() {
            Ok(num) if num > 12 => (Some(num), args.second_arg.clone().and_then(|m| m.parse().ok())),
            Ok(num) => (None, Some(num as u8)),
            Err(_) => {
                eprintln!("Error: Invalid number format");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    let year = year.unwrap_or(date.year() as u16);

    if year < 1583 {
        eprintln!("Error: Year must be 1583 or later");
        std::process::exit(1);
    }

    if args.show_year || (has_first_arg && args.second_arg.is_none() && year != date.year() as u16) {
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
        cal.print();
    } else {
        let cal = Calendar {
            year,
            month: now.month0() as u8,
        };
        cal.print();
    }
}
