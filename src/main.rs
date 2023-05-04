mod calendar;
use calendar::*;
use std::time::SystemTime;
use std::{env, process::exit};

fn main() {
    let args: Vec<String> = env::args().collect();

    let now = SystemTime::now();

    if args.len() == 1 {
        let cal = Calendar::from_time_millis(now);
        cal.print();
        exit(0);
    }

    if args.len() != 3 {
        println!("Usage: calendar <year> <month>");
        exit(1);
    }
    let year: u16 = match args[1].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("The year must be an integer");
            exit(1);
        }
    };
    let month: u8 = match args[2].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("The month must be an integer");
            exit(1);
        }
    };

    if year < 1759 || (1..12).contains(&month) {
        println!("Invalid range");
        exit(1);
    }
    let cal = Calendar {
        year,
        month: month - 1,
    };
    cal.print();
}
