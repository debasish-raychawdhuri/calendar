use chrono::{Datelike, Local, NaiveDate};
// use colored::*; // pancurses will handle colors
use std::{fmt::Display, str::FromStr};
use pancurses::{Window, Attribute, chtype};


/// Represents a calendar for a specific year and month
pub struct Calendar {
    pub month: u8,  // Month (0-based, 0-11)
    pub year: u16,  // Year (1583 or later)
}

/// Represents days of the week
#[derive(Debug, PartialEq)]
pub enum DayOfWeek {
    Sun,
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
}

impl DayOfWeek {
    /// Converts a day number to its corresponding day of the week
    /// 
    /// # Arguments
    /// * `day` - The day number (any integer)
    /// 
    /// # Returns
    /// * The corresponding `DayOfWeek`
    fn from_day_number(day: u32) -> Self {
        match day % 7 {
            0 => DayOfWeek::Sun,
            1 => DayOfWeek::Mon,
            2 => DayOfWeek::Tue,
            3 => DayOfWeek::Wed,
            4 => DayOfWeek::Thu,
            5 => DayOfWeek::Fri,
            6 => DayOfWeek::Sat,
            _ => DayOfWeek::Fri,
        }
    }
}

/// Implements string representation for DayOfWeek
impl Display for DayOfWeek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let day_str = match &self {
            DayOfWeek::Sun => "Sun",
            DayOfWeek::Mon => "Mon",
            DayOfWeek::Tue => "Tue",
            DayOfWeek::Wed => "Wed",
            DayOfWeek::Thu => "Thu",
            DayOfWeek::Fri => "Fri",
            DayOfWeek::Sat => "Sat",
        };
        write!(f, "{}", day_str)
    }
}

impl Calendar {
    /// Gets today's date as a tuple `(day, month, year)`.
    ///
    /// # Returns
    /// * A tuple containing the current day, month (0-based), and year.
    pub fn get_today() -> (u32, u8, u16) {
        let now = Local::now().date_naive();
        let cal = Calendar {
            year: now.year() as u16,
            month: now.month0() as u8,
        };
        let today = now.day();
        (today, cal.month, cal.year)
    }

    /// Calculates the first day of the year relative to year 0
    /// This is used as a base for calculating specific dates
    /// 
    /// # Returns
    /// * The number of days from year 0 to the start of the current year
    pub fn get_year_base_day(&self) -> u32 {
        let year = (self.year - 1) as u32;
        let base_days_for_year = year * 365;
        let leap_days_for_year = year / 4;
        let leap_misses_for_century = year / 100;
        let leap_hits_for_century = year / 400;
        base_days_for_year + leap_days_for_year - leap_misses_for_century + leap_hits_for_century
    }

    /// Checks if the current year is a leap year
    /// Uses the Gregorian calendar rules:
    /// - Years divisible by 4 are leap years
    /// - Century years must be divisible by 400 to be leap years
    pub fn is_leap_year(&self) -> bool {
        if self.year % 100 == 0 {
            self.year % 400 == 0
        } else {
            self.year % 4 == 0
        }
    }

    /// Calculates the base day of the current month (number of days since year 0).
    ///
    /// # Returns
    /// * The base day of the month as a `u32`.
    pub fn get_month_base_day(&self) -> u32 {
        let year_first_day = self.get_year_base_day();
        let month_days: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let month = (self.month) as usize;
        let month_days: u32 = month_days.into_iter().take(month).sum();
        if self.is_leap_year() && month > 1 {
            year_first_day + month_days + 1
        } else {
            year_first_day + month_days
        }
    }

    /// Gets the day of the week for a given day of the month.
    ///
    /// # Arguments
    /// * `day` - The day of the month.
    ///
    /// # Returns
    /// * The `DayOfWeek` corresponding to the given day.
    pub fn get_day_of_week(&self, day: u32) -> DayOfWeek {
        DayOfWeek::from_day_number(self.get_month_base_day() + day)
    }

    /// Helper function to add padding spaces based on number width
    /// 
    /// # Arguments
    /// * `v` - The day number
    /// * `has_event` - Whether the day has an event (affects padding for asterisk)
    /// 
    /// # Returns
    /// * A string containing the appropriate number of leading spaces for formatting.
    fn get_day_padding(day: u32, has_event: bool) -> String {
        if day <= 9 {
            if has_event { " ".to_string() } else { "  ".to_string() } // " 1*" vs "  1 "
        } else { // day is 10-31
            if has_event { "".to_string() } else { " ".to_string() } // "15*" vs " 15 "
        }
    }

    /// Creates a string with a specified number of spaces
    /// 
    /// # Arguments
    /// * `n` - The number of spaces to create
    /// 
    /// # Returns
    /// * A string containing n spaces
    fn spaces(n: usize) -> String {
        let mut s = String::from_str("").unwrap();
        for _ in 0..n {
            s += " ";
        }
        s
    }

    /// Calculates the starting day of a given line in the calendar.
    ///
    /// # Arguments
    /// * `line_no` - The line number (0-based) for which the starting day is calculated.
    ///
    /// # Returns
    /// * The starting day of the line as an `i32`.
    fn calculate_line_start(&self, line_no: u32) -> i32 {
        let month_base = (self.get_month_base_day() % 7) as i32;
        let mut line_no = line_no;
        if month_base == 6 {
            line_no += 1;
        }
        (line_no * 7) as i32 - month_base
    }

    /// Prints a single day in the calendar.
    ///
    /// # Arguments
    /// * `day` - The day of the month to print.
    /// * `today` - A tuple representing today's date `(day, month, year)`.
    /// * `j` - The position of the day in the week (0-based).
    /// * `current_y` - The y-coordinate in the ncurses window.
    /// * `start_x` - The starting x-coordinate for this day's printing.
    /// * `selected_day_val`, `selected_month_0_idx`, `selected_year_val` - The currently selected date.
    /// * `event_dates` - A slice of NaiveDate for days that have events.
    fn print_day(&self, win: &Window, day: i32, today: (u32, u8, u16), j: usize, current_y: i32, start_x: i32, selected_day_val: u32, selected_month_0_idx: u8, selected_year_val: u16, event_dates: &[NaiveDate]) {
        if day <= 0 || day > self.get_total_days_in_month() as i32 {
            win.mvaddstr(current_y, start_x + (j * 4) as i32, "    ");
        } else {
            let current_day_naive_date = NaiveDate::from_ymd_opt(self.year as i32, (self.month + 1) as u32, day as u32);
            let has_event = if let Some(d) = current_day_naive_date {
                event_dates.contains(&d)
            } else {
                false
            };

            if j % 7 == 0 { // Sunday
                self.print_week_start_day(win, day, today, current_y, start_x + (j * 4) as i32, selected_day_val, selected_month_0_idx, selected_year_val, has_event);
            } else { // Other days
                self.print_regular_day(win, day, today, current_y, start_x + (j * 4) as i32, selected_day_val, selected_month_0_idx, selected_year_val, has_event);
            }
        }
    }

    /// Prints a day that starts a week (e.g., Sunday).
    ///
    /// # Arguments
    /// * `day` - The day of the month to print.
    /// * `today` - A tuple representing today's date `(day, month, year)`.
    /// * `y`, `x` - Coordinates for ncurses window.
    /// * `selected_day_val`, `selected_month_0_idx`, `selected_year_val` - The currently selected date.
    /// * `has_event` - Boolean indicating if the current day has an event.
    fn print_week_start_day(&self, win: &Window, day: i32, today: (u32, u8, u16), y: i32, x: i32, selected_day_val: u32, selected_month_0_idx: u8, selected_year_val: u16, has_event: bool) {
        let day_num_str = day.to_string();
        let marker = if has_event { "*" } else { "" };
        let combined_day_display = format!("{}{}", day_num_str, marker);
        // Ensure the display string is right-aligned within 3 characters, followed by a space, for a total of 4 characters.
        let day_str = format!("{:>3} ", combined_day_display); 
        
        let is_selected = day as u32 == selected_day_val && self.month == selected_month_0_idx && self.year == selected_year_val;
        // let is_today = day == today.0 as i32 && self.month == today.1 && self.year == today.2;

        if is_selected {
            win.attron(Attribute::A_REVERSE);
        }
        // TODO: Add specific color for "today" if it's not selected, or combine attributes.
        // TODO: Add specific color for "Sunday" if desired.
        win.mvaddstr(y, x, &day_str);
        if is_selected {
            win.attroff(Attribute::A_REVERSE);
        }
    }

    /// Prints a regular day (not the start of a week).
    ///
    /// # Arguments
    /// * `day` - The day of the month to print.
    /// * `today` - A tuple representing today's date `(day, month, year)`.
    /// * `y`, `x` - Coordinates for ncurses window.
    /// * `selected_day_val`, `selected_month_0_idx`, `selected_year_val` - The currently selected date.
    /// * `has_event` - Boolean indicating if the current day has an event.
    fn print_regular_day(&self, win: &Window, day: i32, today: (u32, u8, u16), y: i32, x: i32, selected_day_val: u32, selected_month_0_idx: u8, selected_year_val: u16, has_event: bool) {
        let day_num_str = day.to_string();
        let marker = if has_event { "*" } else { "" };
        let combined_day_display = format!("{}{}", day_num_str, marker);
        // Ensure the display string is right-aligned within 3 characters, followed by a space.
        let day_str = format!("{:>3} ", combined_day_display);

        let is_selected = day as u32 == selected_day_val && self.month == selected_month_0_idx && self.year == selected_year_val;
        // let is_today = day == today.0 as i32 && self.month == today.1 && self.year == today.2;

        if is_selected {
            win.attron(Attribute::A_REVERSE);
        }
        // TODO: Add specific color for "today" if it's not selected, or combine attributes.
        win.mvaddstr(y, x, &day_str);
        if is_selected {
            win.attroff(Attribute::A_REVERSE);
        }
    }

    /// Calculates the total number of days in the current month.
    ///
    /// # Returns
    /// * The total number of days in the month as a `u32`.
    fn get_total_days_in_month(&self) -> u32 {
        let month_days: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut total_days = month_days[self.month as usize];
        if self.is_leap_year() && self.month == 1 {
            total_days += 1;
        }
        total_days
    }

    /// Prints a calendar row starting from the given line number
    /// 
    /// # Arguments
    /// * `line_no` - The row number (0-5) to print
    /// * `start_y` - The base y-coordinate for calendar rows.
    /// * `selected_day_val`, `selected_month_0_idx`, `selected_year_val` - The currently selected date.
    /// * `event_dates` - A slice of NaiveDate for days that have events.
    fn print_line(&self, win: &Window, line_no: u32, start_y: i32, start_x: i32, selected_day_val: u32, selected_month_0_idx: u8, selected_year_val: u16, event_dates: &[NaiveDate]) {
        let today = Self::get_today();
        let line_start = self.calculate_line_start(line_no);
        let current_y = start_y + line_no as i32;
        for (j, day) in (line_start..line_start + 7).enumerate() {
            self.print_day(win, day, today, j, current_y, start_x, selected_day_val, selected_month_0_idx, selected_year_val, event_dates);
        }
    }

    /// Prints the day names header (Sun Mon Tue etc.)
    fn print_day_names(&self, win: &Window, y: i32, x: i32) {
        // TODO: Add ncurses colors
        win.mvaddstr(y, x, " Sun Mon Tue Wed Thu Fri Sat");
    }

    fn print_heading_month(&self, win: &Window, y: i32, x: i32) {
        let month_names = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];

        let name_length = month_names[self.month as usize].len();
        let total_length = name_length;
        // Approximate centering for a 28 char width (7 days * 4 chars)
        let available_width = 28;
        let empty_space_left = (available_width - total_length) / 2;
        // let empty_space_right = available_width - total_length - empty_space_left;

        let heading = format!(
            "{}{}",
            Self::spaces(empty_space_left),
            month_names[self.month as usize]
            // Self::spaces(empty_space_right) // Not strictly needed with mvaddstr
        );
        win.mvaddstr(y, x, &heading);
    }

    /// Prints three months side by side in the calendar.
    /// (This function will need significant rework for ncurses, placeholder for now)
    ///
    /// # Arguments
    /// * `win` - The ncurses window to draw in.
    /// * `cal1` - The first calendar to print.
    /// * `cal2` - The second calendar to print.
    /// * `cal3` - The third calendar to print.
    pub fn print_three_calendars(_win: &Window, _cal1: Calendar, _cal2: Calendar, _cal3: Calendar) {
        // cal1.print_heading_month(win);
        // cal2.print_heading_month(win);
        // cal3.print_heading_month(win);
        // win.mvaddstr(next_line, x, ""); // Placeholder for println

        // cal1.print_day_names(win);
        // cal2.print_day_names(win);
        // cal3.print_day_names(win);
        // win.mvaddstr(next_line, x, ""); // Placeholder for println

        // for i in 0..6 {
            // cal1.print_line(win, i);
            // cal2.print_line(win, i);
            // cal3.print_line(win, i);
            // win.mvaddstr(next_line, x, ""); // Placeholder for println
        // }
        // For now, just indicate it's not implemented
        // _win.mvaddstr(0, 0, "Three-month view not implemented for ncurses yet.");
    }

    /// Prints a single month in the ncurses window.
    ///
    /// # Arguments
    /// * `win` - The ncurses window to draw in.
    /// * `cal` - The calendar to print.
    /// * `selected_day_val`, `selected_month_0_idx`, `selected_year_val` - The currently selected date to highlight.
    /// * `event_dates` - A slice of NaiveDate for days that have events.
    pub fn print_one_month(win: &Window, cal: Calendar, selected_day_val: u32, selected_month_0_idx: u8, selected_year_val: u16, event_dates: &[NaiveDate]) {
        let start_y = 1; // Starting row for the calendar in the window
        let start_x = 1; // Starting col for the calendar in the window

        win.clear(); // Clear the window before drawing

        cal.print_heading_month(win, start_y, start_x);
        cal.print_day_names(win, start_y + 1, start_x); // y increased by 1
        for i in 0..6 {
            // y increased by 2 to account for heading and day names
            cal.print_line(win, i, start_y + 2, start_x, selected_day_val, selected_month_0_idx, selected_year_val, event_dates); 
        }
        win.refresh(); // Refresh the window to show changes
    }

    /// Prints the entire year as a calendar.
    /// (This function will need significant rework for ncurses, placeholder for now)
    ///
    /// # Arguments
    /// * `win` - The ncurses window to draw in.
    /// * `year` - The year to print.
    pub fn print_entire_year(_win: &Window, year: u16) {
        // Self::print_year_heading(win, year); // Needs adaptation
        // for i in 0..4 {
            // let cal1 = Calendar { year, month: i * 3 };
            // let cal2 = Calendar {
            //     year,
            //     month: i * 3 + 1,
            // };
            // let cal3 = Calendar {
            //     year,
            //     month: i * 3 + 2,
            // };
            // Self::print_three_calendars(win, cal1, cal2, cal3); // Needs adaptation
            // win.mvaddstr(next_line, x, ""); // Placeholder
        // }
        // For now, just indicate it's not implemented
        // _win.mvaddstr(0,0, &format!("Year view for {} not implemented for ncurses yet.", year));
    }

    /// Gets the calendar for the previous month
    /// Handles year boundaries (e.g., January to previous December)
    fn prev_month(&self) -> Calendar {
        if self.month == 0 {
            Calendar {
                year: self.year - 1,
                month: 11,
            }
        } else {
            Calendar {
                year: self.year,
                month: self.month - 1,
            }
        }
    }

    /// Gets the calendar for the next month
    /// Handles year boundaries (e.g., December to next January)
    fn next_month(&self) -> Calendar {
        if self.month == 11 {
            Calendar {
                year: self.year + 1,
                month: 0,
            }
        } else {
            Calendar {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    fn print_year_heading(_win: &Window, year: u16) {
        // let space_on_each_side = 42; // This needs to be window-relative
        // _win.mvaddstr(y, x, &Self::spaces(space_on_each_side));
        // _win.addstr(&year.to_string()); // TODO: Add attributes
        // _win.addstr(&Self::spaces(space_on_each_side));
        // _win.mvaddstr(next_line, x, ""); // Placeholder
        // _win.mvaddstr(next_line, x, ""); // Placeholder
         _win.mvaddstr(0,0, &format!("Year Heading for {}", year)); // Basic placeholder
    }

    /// Prints the calendar for the current month, along with the previous and next months.
    /// (This function will need significant rework for ncurses, placeholder for now)
    pub fn print(self, win: &Window) {
        let prev_month = self.prev_month();
        let next_month = self.next_month();
        // Self::print_year_heading(win, self.year); // Needs adaptation
        Self::print_three_calendars(win, prev_month, self, next_month); // Needs adaptation
        win.mvaddstr(0, 0, "Three-month (default) view not implemented for ncurses yet.");

    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn check_first_year() {
        let calendar = Calendar { year: 1, month: 1 };
        assert_eq!(calendar.get_year_base_day(), 0);
        assert_eq!(calendar.get_month_base_day(), 31);
    }

    #[test]
    fn check_leap_year() {
        let calendar = Calendar { year: 4, month: 1 };
        assert_eq!(calendar.get_year_base_day(), 365 * 3);
        assert_eq!(calendar.get_month_base_day(), 365 * 3 + 31);
    }

    #[test]
    fn check_leap_year_high_month() {
        let calendar = Calendar { year: 4, month: 3 };
        assert_eq!(calendar.get_year_base_day(), 365 * 3);
        assert_eq!(calendar.get_month_base_day(), 365 * 3 + 31 + 29 + 31);
    }

    #[test]
    fn check_day_of_week() {
        let calendar = Calendar {
            year: 2022,
            month: 6,
        };
        assert_eq!(calendar.get_day_of_week(3), DayOfWeek::Sun);
    }

    #[test]
    fn check_day_of_week_2() {
        let calendar = Calendar {
            year: 2022,
            month: 5,
        };
        assert_eq!(calendar.get_day_of_week(27), DayOfWeek::Mon);
    }

    #[test]
    fn check_day_of_week_leap() {
        let calendar = Calendar {
            year: 2020,
            month: 5,
        };
        assert_eq!(calendar.get_day_of_week(9), DayOfWeek::Tue);
    }

    #[test]
    fn check_day_of_week_leap_2() {
        let calendar = Calendar {
            year: 2020,
            month: 0,
        };
        assert_eq!(calendar.get_day_of_week(15), DayOfWeek::Wed);
    }
}
