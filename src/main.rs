mod calendar;
use colored::*;
use calendar::*;
use std::{env, process::exit};
use std::time::SystemTime;

fn main() {
    let args: Vec<String> = env::args().collect();

    let now = SystemTime::now();

    if args.len() == 1{
        let cal = Calendar::from_time_millis(now);
        cal.print();
        exit(0);
    }

    if args.len() != 3 {
        println!("Usage: calendar <year> <month>");
        exit(1);
    }
    let year:u16 = match args[1].parse() {
        Ok(v)=>v,
        Err(_) => {
            println!("The year must be an integer");
            exit(1);
        }
    };
    let month:u8 = match args[2].parse(){
        Ok(v) => v,
        Err(_) => {
            println!("The month must be an integer");
            exit(1);
        }
    };

    if year<1759 || month<1 || month>12 {
        println!("Invalid range");
        exit(1);
    }
    let cal = Calendar{
        year:year,
        month: month-1,
    };
    cal.print();
}
