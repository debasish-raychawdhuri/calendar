mod calendar;
use colored::*;
use calendar::*;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let year:u16 = args[1].parse().unwrap();
    let month:u8 = args[2].parse().unwrap();
    let cal = Calendar{
        year:year,
        month: month-1,
    };
    cal.print();
}
