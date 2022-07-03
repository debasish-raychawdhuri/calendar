
pub struct Calendar{
    month:u8, //month starts from 0
    year:u16,
}


#[derive(Debug, PartialEq)]
pub enum DayOfWeek{
    SUN,MON,TUE,WED,THU,FRI,SAT,
}

impl DayOfWeek{
    fn from_day_number(day:u32) -> Self{
        match day % 7{
            0=>DayOfWeek::SUN,
            1=>DayOfWeek::MON,
            2=>DayOfWeek::TUE,
            3=>DayOfWeek::WED,
            4=>DayOfWeek::THU,
            5=>DayOfWeek::FRI,
            6=>DayOfWeek::SAT,
            _=>DayOfWeek::FRI,
        }
    }
}

impl Calendar{
    pub fn get_year_base_day(&self) -> u32 {
        let year = (self.year-1) as u32; // the point being that the current year's days are still not added.
        let base_days_for_year = year*365;
        let leap_days_for_year = year/4;
        let leap_misses_for_century = year/100;
        let leap_hits_for_century = year/400;
        base_days_for_year +leap_days_for_year-leap_misses_for_century+leap_hits_for_century 
    }

    pub fn is_leap_year(&self) -> bool {
        if self.year % 100 == 0 {
            self.year % 400 == 0
        }else{
            self.year % 4 == 0 
        }
    }

    pub fn get_month_base_day(&self) -> u32 {
        let year_first_day = self.get_year_base_day();
        let month_days :[u32;12]= [31,28,31,30,31,30,31,31,30,31,30,31];
        let month = (self.month) as usize;
        let month_days:u32 = month_days.into_iter().take(month).sum();
        if self.is_leap_year() && month > 1 {
            year_first_day + month_days+1
        }else{
            year_first_day+month_days
        }
        
    }

    pub fn get_day_of_week(&self, day:u32) -> DayOfWeek{
        DayOfWeek::from_day_number(self.get_month_base_day()+day)
    }
}
#[cfg(test)]
mod test{
    use super::*;
    #[test]
    fn check_first_year(){
        let calendar = Calendar{
            year:1,
            month:1,
        };
        assert_eq!(calendar.get_year_base_day(),0);
        assert_eq!(calendar.get_month_base_day(),31);
    }

    #[test]
    fn check_leap_year(){
        let calendar = Calendar{
            year:4,
            month:1,
        };
        assert_eq!(calendar.get_year_base_day(),365*3);
        assert_eq!(calendar.get_month_base_day(),365*3 + 31);
    }

    #[test]
    fn check_leap_year_high_month(){
        let calendar = Calendar{
            year:4,
            month:3,
        };
        assert_eq!(calendar.get_year_base_day(),365*3);
        assert_eq!(calendar.get_month_base_day(),365*3 + 31+29+31);
    }

    #[test]
    fn check_day_of_week(){
        let calendar = Calendar{
            year:2022,
            month:6,
        };
        assert_eq!(calendar.get_day_of_week(3), DayOfWeek::SUN);
    }

    #[test]
    fn check_day_of_week_2(){
        let calendar = Calendar{
            year:2022,
            month:5,
        };
        assert_eq!(calendar.get_day_of_week(27), DayOfWeek::MON);
    }

    #[test]
    fn check_day_of_week_leap(){
        let calendar = Calendar{
            year:2020,
            month:5,
        };
        assert_eq!(calendar.get_day_of_week(9), DayOfWeek::TUE);
    }

    #[test]
    fn check_day_of_week_leap_2(){
        let calendar = Calendar{
            year:2020,
            month:0,
        };
        assert_eq!(calendar.get_day_of_week(15), DayOfWeek::WED);
    }
}