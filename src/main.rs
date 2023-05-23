mod calendar;
use std::{env, process::exit};

use calendar::Calendar;
use chrono::{Datelike, Local};

fn main() {
    let args: Vec<String> = env::args().collect();

    let now = Local::now();
    let date = now.date_naive();

    if args.len() == 1 {
        let cal = Calendar {
            year: date.year() as u16,
            month: now.month0() as u8,
        };
        cal.print();
        exit(0);
    }

    if args.len() != 3 && args.len() != 2 {
        println!("Usage: calendar <year> <month>");
        println!("Or: calendar <year>");
        exit(1);
    }
    let year: u16 = match args[1].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("The year must be an integer");
            exit(1);
        }
    };
    if year < 1759 {
        println!("Invalid range");
        exit(1);
    }

    if args.len() == 3 {
        let month: u8 = match args[2].parse() {
            Ok(v) => v,
            Err(_) => {
                println!("The month must be an integer");
                exit(1);
            }
        };
        if !(1..=12).contains(&month) {
            println!("Invalid range");
            exit(1);
        }

        let cal = Calendar {
            year,
            month: month - 1,
        };
        cal.print();
    } else {
        Calendar::print_entire_year(year);
    }
}
