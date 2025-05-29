mod calendar;
mod db; // Added db module
use calendar::Calendar;
use chrono::{Datelike, Local, NaiveDate, Duration, Month};
use num_traits::FromPrimitive;
use pancurses::{initscr, endwin, noecho, echo, curs_set, newwin, Input, Window, napms, LINES, COLS};
use clap::Parser;

// Command line arguments for the calendar application
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
fn main() {
    let window = initscr();
    curs_set(0);
    noecho();
    window.keypad(true);

    let args = Args::parse();
    let now_local = Local::now();
    let mut selected_date = now_local.date_naive();

    // Override with command line arguments if provided
    // Command line month is 1-indexed, NaiveDate/chrono::Month is 1-indexed, Calendar struct is 0-indexed.
    if let Some(first_arg_str) = args.first_arg {
        match first_arg_str.parse::<i32>() { // Parse as i32 for year
            Ok(num) if num > 12 => { // Assumed to be a year
                let year_val = num;
                let month_val_opt = args.second_arg.clone().and_then(|m_str| m_str.parse::<u32>().ok());
                let month_val = month_val_opt.unwrap_or(selected_date.month());
                
                if year_val < 1583 {
                    endwin();
                    eprintln!("Error: Year must be 1583 or later");
                    std::process::exit(1);
                }
                if !(1..=12).contains(&month_val) {
                    endwin();
                    eprintln!("Error: Month must be between 1 and 12");
                    std::process::exit(1);
                }
                // Try to keep the day, but clamp to valid days in new month/year
                let mut day_val = selected_date.day();
                if let Some(new_date_check) = NaiveDate::from_ymd_opt(year_val, month_val, 1) {
                    let days_in_month = days_in_month(new_date_check.year() as u16, new_date_check.month0() as u8);
                    if day_val > days_in_month { day_val = days_in_month; }
                }
                selected_date = NaiveDate::from_ymd_opt(year_val, month_val, day_val)
                                .unwrap_or(NaiveDate::from_ymd_opt(year_val, month_val, 1).unwrap());
            }
            Ok(num) => { // Assumed to be a month (1-12)
                let month_val = num as u32;
                 if !(1..=12).contains(&month_val) {
                    endwin();
                    eprintln!("Error: Month must be between 1 and 12");
                    std::process::exit(1);
                }
                // Try to keep the day, but clamp to valid days in new month
                let mut day_val = selected_date.day();
                if let Some(new_date_check) = NaiveDate::from_ymd_opt(selected_date.year(), month_val, 1) {
                     let days_in_month = days_in_month(new_date_check.year() as u16, new_date_check.month0() as u8);
                    if day_val > days_in_month { day_val = days_in_month; }
                }
                selected_date = NaiveDate::from_ymd_opt(selected_date.year(), month_val, day_val)
                                .unwrap_or(NaiveDate::from_ymd_opt(selected_date.year(), month_val, 1).unwrap());
            }
            Err(_) => {
                endwin();
                eprintln!("Error: Invalid number format for year/month argument");
                std::process::exit(1);
            }
        }
    } else if args.second_arg.is_some() {
        endwin();
        eprintln!("Error: Cannot specify month without a year/month first argument.");
        std::process::exit(1);
    }


    if selected_date.year() < 1583 {
        window.mvaddstr(0, 0, "Error: Year must be 1583 or later (selected date check)");
        window.refresh();
        window.getch();
        endwin();
        std::process::exit(1);
    }
    
    let cal_win = newwin(20, 40, 1, 1); // height, width, y, x
    cal_win.keypad(true);

    let mut current_calendar_view_obj = Calendar {
        year: selected_date.year() as u16,
        month: selected_date.month0() as u8, // Calendar struct uses 0-indexed month
    };
    // Initial fetch of event dates for the current view
    let mut event_dates_for_current_view: Vec<NaiveDate> = Vec::new();
    match db::connect_db() {
        Ok(mut client) => {
            // Month for query needs to be 1-indexed
            match db::get_events_for_month(&mut client, current_calendar_view_obj.year as i32, current_calendar_view_obj.month as u32 + 1) {
                Ok(dates) => event_dates_for_current_view = dates,
                Err(e) => {
                    window.mvaddstr(LINES - 1, 0, &format!("Error fetching events: {}", e));
                    window.refresh();
                }
            }
        }
        Err(e) => {
            window.mvaddstr(LINES - 1, 0, &format!("DB conn error for events: {}", e));
            window.refresh();
        }
    }
    Calendar::print_one_month(&cal_win, current_calendar_view_obj, selected_date.day(), selected_date.month0() as u8, selected_date.year() as u16, &event_dates_for_current_view);

    loop {
        // Clear any previous temporary messages at the bottom of the main window
        window.mv(LINES -1, 0);
        window.clrtoeol();
        window.refresh();


        match cal_win.getch() { // Get input from the calendar window
            Some(Input::Character('q')) => break,
            Some(Input::KeyEnter) | Some(Input::Character('\n')) | Some(Input::Character('\r')) => {
                if let Some(event_text) = handle_event_input_dialog(&window, selected_date) {
                    match db::connect_db() {
                        Ok(mut client) => {
                            match db::save_event(&mut client, selected_date, &event_text) {
                                Ok(_) => {
                                    window.mvaddstr(LINES - 1, 0, &format!("Event saved for {}: {}", selected_date.format("%Y-%m-%d"), event_text));
                                }
                                Err(e) => {
                                    window.mvaddstr(LINES - 1, 0, &format!("Error saving event: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                             window.mvaddstr(LINES - 1, 0, &format!("DB connection error: {}", e));
                        }
                    }
                    window.refresh();
                }
                // Ensure calendar window is redrawn correctly after dialog closes
                // and that the main window (status messages) is also up to date.
                window.touchwin(); // Touch main window to ensure it's considered for refresh by pancurses
                cal_win.touchwin();
            }
            Some(Input::KeyLeft) => {
                if let Some(prev_day) = selected_date.pred_opt() {
                    if prev_day.year() >= 1583 {
                        selected_date = prev_day;
                    } else { pancurses::beep(); }
                } else { pancurses::beep(); }
            }
            Some(Input::KeyRight) => {
                if let Some(next_day) = selected_date.succ_opt() {
                    selected_date = next_day;
                }
            }
            Some(Input::KeyUp) => {
                if let Some(prev_week) = selected_date.checked_sub_signed(Duration::days(7)) {
                     if prev_week.year() >= 1583 {
                        selected_date = prev_week;
                     } else { pancurses::beep(); }
                } else { pancurses::beep(); }
            }
            Some(Input::KeyDown) => {
                 if let Some(next_week) = selected_date.checked_add_signed(Duration::days(7)) {
                    selected_date = next_week;
                }
            }
            Some(Input::KeyResize) => {
                window.clear(); // Clear main screen
                cal_win.clear(); // Clear calendar window
                // Potentially re-create or resize cal_win if needed
                window.refresh(); // Refresh main
                                  // Calendar will be redrawn below
            }
            _ => {
                // No operation for other keys
            }
        }

        // Update calendar view if month or year of selected_date has changed
        let month_changed = current_calendar_view_obj.month != selected_date.month0() as u8;
        let year_changed = current_calendar_view_obj.year != selected_date.year() as u16;

        if year_changed || month_changed {
            current_calendar_view_obj = Calendar {
                year: selected_date.year() as u16,
                month: selected_date.month0() as u8, // Calendar struct uses 0-indexed month
            };
            // Fetch events for the new month/year view
            event_dates_for_current_view.clear();
            match db::connect_db() {
                Ok(mut client) => {
                     // Month for query needs to be 1-indexed
                    match db::get_events_for_month(&mut client, current_calendar_view_obj.year as i32, current_calendar_view_obj.month as u32 + 1) {
                        Ok(dates) => event_dates_for_current_view = dates,
                        Err(e) => {
                            // Display error temporarily at the bottom of the main window
                            window.mv(LINES - 1, 0); // Move to last line
                            window.clrtoeol();      // Clear the line
                            window.addstr(&format!("Error fetching events: {}", e));
                            window.refresh();
                        }
                    }
                }
                Err(e) => {
                    window.mv(LINES - 1, 0);
                    window.clrtoeol();
                    window.addstr(&format!("DB conn error for events: {}", e));
                    window.refresh();
                }
            }
        }
        // Redraw the calendar with the new selected date (or same if only day changed within month)
        Calendar::print_one_month(&cal_win, current_calendar_view_obj, selected_date.day(), selected_date.month0() as u8, selected_date.year() as u16, &event_dates_for_current_view);
    }

    endwin();
}

fn handle_event_input_dialog(parent_win: &Window, date: NaiveDate) -> Option<String> {
    let dialog_height = 7;
    let dialog_width = COLS() - 20; // Make it somewhat responsive to terminal width
    let start_y = (LINES() - dialog_height) / 2;
    let start_x = (COLS() - dialog_width) / 2;

    let dialog_win = newwin(dialog_height, dialog_width, start_y, start_x);
    dialog_win.keypad(true);
    dialog_win.border(0,0,0,0,0,0,0,0);
    
    let date_str = date.format("%Y-%m-%d").to_string();
    dialog_win.mvaddstr(1, 2, &format!("Add Event for {}", date_str));
    dialog_win.mvaddstr(3, 2, "Event: ");
    dialog_win.refresh();

    echo(); // Enable echoing input
    let mut event_buffer = String::new();
    dialog_win.mv(3, 9); // Move cursor to input position

    // getnstr requires a mutable buffer and max length.
    // Using a loop with getch for more control, e.g. for Esc.
    // However, the prompt says getstr (or getnstr) is fine. Let's use getnstr for simplicity.
    // Max event length, e.g., 50 chars. dialog_width - 10 (for "Event: " and borders)
    let max_input_len = if dialog_width > 12 { (dialog_width - 12) as usize } else { 1 };
    
    // pancurses getnstr takes a &mut String, which is convenient.
    // It reads until newline.
    let result_code = dialog_win.getnstr(&mut event_buffer, max_input_len as i32);
    
    noecho(); // Disable echoing input

    parent_win.touchwin(); // Mark parent as needing refresh
    parent_win.refresh();
    // dialog_win.clear(); // Clear content before deleting, though delwin should handle it
    delwin(dialog_win);
    
    if result_code == pancurses::OK && !event_buffer.trim().is_empty() {
        Some(event_buffer.trim().to_string())
    } else {
        None // No input or error
    }
}

// Helper function to get days in month, useful for clamping day values.
// month_0_idx is 0-indexed (0 for Jan, 11 for Dec)
fn days_in_month(year: u16, month_0_idx: u8) -> u32 {
    if month_0_idx == 1 { // February
        if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
            29
        } else {
            28
        }
    } else if [3, 5, 8, 10].contains(&month_0_idx) { // Apr, Jun, Sep, Nov
        30
    } else {
        31
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_days_in_month_function() {
        // Non-leap year tests
        assert_eq!(days_in_month(2023, 0), 31); // Jan
        assert_eq!(days_in_month(2023, 1), 28); // Feb
        assert_eq!(days_in_month(2023, 3), 30); // Apr

        // Leap year tests
        assert_eq!(days_in_month(2024, 0), 31); // Jan
        assert_eq!(days_in_month(2024, 1), 29); // Feb (leap)
        assert_eq!(days_in_month(2024, 2), 31); // Mar
        
        // Century non-leap year
        assert_eq!(days_in_month(1900, 1), 28); // Feb
        // Century leap year
        assert_eq!(days_in_month(2000, 1), 29); // Feb
    }
}
