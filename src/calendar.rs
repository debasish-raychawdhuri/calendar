use chrono::{Datelike, Local};
use colored::*;
use std::{fmt::Display, print, str::FromStr};

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
    /// * `v` - The number to pad
    /// 
    /// # Returns
    /// * A string containing the appropriate number of spaces
    fn pad(v: u32) -> String {
        if v <= 9 {
            format!("   ")
        } else if v <= 99 {
            format!("  ")
        } else {
            format!(" ")
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
    fn print_day(&self, day: i32, today: (u32, u8, u16), j: usize) {
        if day <= 0 || day > self.get_total_days_in_month() as i32 {
            print!("    ");
        } else if j % 7 == 0 {
            self.print_week_start_day(day, today);
        } else {
            self.print_regular_day(day, today);
        }
    }

    /// Prints a day that starts a week (e.g., Sunday).
    ///
    /// # Arguments
    /// * `day` - The day of the month to print.
    /// * `today` - A tuple representing today's date `(day, month, year)`.
    fn print_week_start_day(&self, day: i32, today: (u32, u8, u16)) {
        if day == today.0 as i32 && self.month == today.1 && self.year == today.2 {
            print!(
                "{}{}",
                Self::pad(day as u32),
                format!("{}", day).bold().black().on_magenta()
            );
        } else {
            print!("{}{}", Self::pad(day as u32), format!("{}", day).magenta());
        }
    }

    /// Prints a regular day (not the start of a week).
    ///
    /// # Arguments
    /// * `day` - The day of the month to print.
    /// * `today` - A tuple representing today's date `(day, month, year)`.
    fn print_regular_day(&self, day: i32, today: (u32, u8, u16)) {
        if day == today.0 as i32 && self.month == today.1 && self.year == today.2 {
            print!(
                "{}{}",
                Self::pad(day as u32),
                format!("{}", day).bold().black().on_cyan()
            );
        } else {
            print!("{}{}", Self::pad(day as u32), format!("{}", day).cyan());
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
    fn print_line(&self, line_no: u32) {
        let today = Self::get_today();
        let line_start = self.calculate_line_start(line_no);
        for (j, day) in (line_start..line_start + 7).enumerate() {
            self.print_day(day, today, j);
        }
    }

    /// Prints the day names header (Sun Mon Tue etc.)
    /// Uses different colors for Sunday and other days
    fn print_day_names(&self) {
        print!(
            "{} {}",
            " Sun".red().bold(),
            "Mon Tue Wed Thu Fri Sat".green().bold()
        );
    }

    fn print_heading_month(&self) {
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
        let empty_space_left = (28 - total_length) / 2 + 1;
        let empty_space_right = 28 - total_length - empty_space_left;
        print!(
            "{}{}{}",
            Self::spaces(empty_space_left),
            month_names[self.month as usize].yellow(),
            Self::spaces(empty_space_right),
        );
    }

    /// Prints three months side by side in the calendar.
    ///
    /// # Arguments
    /// * `cal1` - The first calendar to print.
    /// * `cal2` - The second calendar to print.
    /// * `cal3` - The third calendar to print.
    pub fn print_three_calendars(cal1: Calendar, cal2: Calendar, cal3: Calendar) {
        cal1.print_heading_month();
        print!("  ");
        cal2.print_heading_month();
        print!("  ");
        cal3.print_heading_month();
        println!();

        cal1.print_day_names();
        print!("  ");
        cal2.print_day_names();
        print!("  ");
        cal3.print_day_names();
        println!();

        for i in 0..6 {
            cal1.print_line(i);
            print!("  ");
            cal2.print_line(i);
            print!("  ");
            cal3.print_line(i);
            println!();
        }
    }

    /// Prints a single month in the calendar.
    ///
    /// # Arguments
    /// * `cal` - The calendar to print.
    pub fn print_one_month(cal: Calendar) {
        cal.print_heading_month();
        println!();
        cal.print_day_names();
        println!();
        for i in 0..6 {
            cal.print_line(i);
            println!();
        }
    }

    /// Prints the entire year as a calendar.
    ///
    /// # Arguments
    /// * `year` - The year to print.
    pub fn print_entire_year(year: u16) {
        Self::print_year_heading(year);
        for i in 0..4 {
            let cal1 = Calendar { year, month: i * 3 };
            let cal2 = Calendar {
                year,
                month: i * 3 + 1,
            };
            let cal3 = Calendar {
                year,
                month: i * 3 + 2,
            };
            Self::print_three_calendars(cal1, cal2, cal3);
            println!();
        }
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

    fn print_year_heading(year: u16) {
        let space_on_each_side = 42;
        print!("{}", Self::spaces(space_on_each_side));
        print!("{}", year.to_string().bold().bright_yellow());
        print!("{}", Self::spaces(space_on_each_side));
        println!();
        println!();
    }

    /// Prints the calendar for the current month, along with the previous and next months.
    pub fn print(self) {
        let prev_month = self.prev_month();
        let next_month = self.next_month();
        Self::print_year_heading(self.year);
        Self::print_three_calendars(prev_month, self, next_month);
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
