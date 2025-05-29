use crate::calendar::{Calendar, DayOfWeek};
use crate::db::{Database, DbError, Event};
use chrono::{Datelike, Local, NaiveDate};
use ncurses::*;
use std::sync::Arc;
use tokio::sync::Mutex;

// Color pairs for ncurses
const COLOR_DEFAULT: i16 = 1;
const COLOR_HIGHLIGHT: i16 = 2;
const COLOR_TODAY: i16 = 3;
const COLOR_EVENT: i16 = 4;
const COLOR_SELECTED: i16 = 5;
const COLOR_DIALOG: i16 = 6;

pub struct CalendarUI {
    db: Arc<Mutex<Database>>,
    current_year: u16,
    current_month: u8,
    selected_day: u32,
    events_cache: Vec<Event>,
}

impl CalendarUI {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        let today = Calendar::get_today();
        CalendarUI {
            db,
            current_year: today.2,
            current_month: today.1,
            selected_day: today.0,
            events_cache: Vec::new(),
        }
    }

    pub async fn init(&mut self) -> Result<(), DbError> {
        // Initialize ncurses
        initscr();
        start_color();
        cbreak();
        noecho();
        keypad(stdscr(), true);
        curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

        // Initialize color pairs
        init_pair(COLOR_DEFAULT, COLOR_WHITE, COLOR_BLACK);
        init_pair(COLOR_HIGHLIGHT, COLOR_RED, COLOR_BLACK);
        init_pair(COLOR_TODAY, COLOR_GREEN, COLOR_BLACK);
        init_pair(COLOR_EVENT, COLOR_CYAN, COLOR_BLACK);
        init_pair(COLOR_SELECTED, COLOR_BLACK, COLOR_WHITE);
        init_pair(COLOR_DIALOG, COLOR_BLACK, COLOR_CYAN);

        // Load events for the current month
        self.load_events().await?;

        Ok(())
    }

    pub fn cleanup(&self) {
        endwin();
    }

    async fn load_events(&mut self) -> Result<(), DbError> {
        let db = self.db.lock().await;
        self.events_cache = db
            .get_events_for_month(self.current_year as i32, (self.current_month + 1) as i32)
            .await?;
        Ok(())
    }

    fn has_event(&self, day: u32) -> bool {
        let target_date = match NaiveDate::from_ymd_opt(
            self.current_year as i32,
            (self.current_month + 1) as u32,
            day,
        ) {
            Some(date) => date,
            None => return false,
        };

        self.events_cache
            .iter()
            .any(|event| event.date == target_date)
    }

    fn get_events_for_day(&self, day: u32) -> Vec<Event> {
        let target_date = match NaiveDate::from_ymd_opt(
            self.current_year as i32,
            (self.current_month + 1) as u32,
            day,
        ) {
            Some(date) => date,
            None => return Vec::new(),
        };

        self.events_cache
            .iter()
            .filter(|event| event.date == target_date)
            .cloned()
            .collect()
    }

    fn draw_calendar(&self) {
        clear();

        let cal = Calendar {
            year: self.current_year,
            month: self.current_month,
        };

        let today = Calendar::get_today();
        let is_current_month = cal.year == today.2 && cal.month == today.1;

        // Print month and year
        let month_name = cal.get_month_name();
        let title = format!("{} {}", month_name, cal.year);
        mvprintw(1, (COLS() - title.len() as i32) / 2, &title);

        // Print day names
        let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for (i, day) in day_names.iter().enumerate() {
            if i == 0 {
                attron(COLOR_PAIR(COLOR_HIGHLIGHT));
            } else {
                attron(COLOR_PAIR(COLOR_DEFAULT));
            }
            mvprintw(3, 4 + i as i32 * 4, day);
            attroff(COLOR_PAIR(if i == 0 { COLOR_HIGHLIGHT } else { COLOR_DEFAULT }));
        }

        // Calculate first day of month
        let first_day_of_week = cal.get_day_of_week(1);
        let first_day_offset = match first_day_of_week {
            DayOfWeek::Sun => 0,
            DayOfWeek::Mon => 1,
            DayOfWeek::Tue => 2,
            DayOfWeek::Wed => 3,
            DayOfWeek::Thu => 4,
            DayOfWeek::Fri => 5,
            DayOfWeek::Sat => 6,
        };

        // Print days
        let total_days = cal.get_total_days_in_month();
        let mut day_counter = 1;
        for week in 0..6 {
            for weekday in 0..7 {
                let x = 4 + weekday * 4;
                let y = 5 + week;

                if week == 0 && weekday < first_day_offset || day_counter > total_days {
                    // Empty cell
                    mvprintw(y, x, "   ");
                } else {
                    // Determine cell color
                    let is_today = is_current_month && day_counter == today.0;
                    let is_selected = day_counter == self.selected_day;
                    let has_event = self.has_event(day_counter);

                    let color = if is_selected {
                        COLOR_SELECTED
                    } else if is_today {
                        COLOR_TODAY
                    } else if has_event {
                        COLOR_EVENT
                    } else if weekday == 0 {
                        COLOR_HIGHLIGHT
                    } else {
                        COLOR_DEFAULT
                    };

                    attron(COLOR_PAIR(color));
                    mvprintw(y, x, &format!("{:2}", day_counter));
                    attroff(COLOR_PAIR(color));

                    day_counter += 1;
                }
            }
        }

        // Print navigation help
        mvprintw(
            LINES() - 2,
            2,
            "Arrow keys: Navigate | Enter: View/Add Event | q: Quit",
        );

        // Display events for selected day
        self.draw_events_panel();

        refresh();
    }

    fn draw_events_panel(&self) {
        let events = self.get_events_for_day(self.selected_day);
        let panel_width = 40;
        let panel_x = COLS() - panel_width - 2;

        // Draw panel border
        for y in 3..LINES() - 3 {
            mvaddch(y, panel_x - 1, ACS_VLINE());
        }

        // Panel title
        attron(A_BOLD());
        mvprintw(
            3,
            panel_x,
            &format!(" Events for {}/{}/{} ", self.selected_day, self.current_month + 1, self.current_year),
        );
        attroff(A_BOLD());

        // List events
        if events.is_empty() {
            mvprintw(5, panel_x + 2, "No events for this day");
        } else {
            for (i, event) in events.iter().enumerate() {
                if i >= 10 {
                    // Limit display to 10 events
                    mvprintw(5 + i as i32, panel_x + 2, "... more events");
                    break;
                }
                
                attron(A_BOLD());
                mvprintw(5 + i as i32 * 2, panel_x + 2, &event.title);
                attroff(A_BOLD());
                
                if let Some(desc) = &event.description {
                    let desc_short = if desc.len() > panel_width as usize - 4 {
                        format!("{}...", &desc[0..panel_width as usize - 7])
                    } else {
                        desc.clone()
                    };
                    mvprintw(6 + i as i32 * 2, panel_x + 4, &desc_short);
                }
            }
        }
    }

    async fn show_event_dialog(&self) -> Result<Option<Event>, DbError> {
        // Save current screen
        let win = newwin(0, 0, 0, 0);
        wrefresh(win);

        // Create dialog window
        let height = 10;
        let width = 60;
        let starty = (LINES() - height) / 2;
        let startx = (COLS() - width) / 2;
        
        let dialog = newwin(height, width, starty, startx);
        box_(dialog, 0, 0);
        wbkgd(dialog, COLOR_PAIR(COLOR_DIALOG));
        
        // Dialog title
        mvwprintw(dialog, 1, 2, &format!("Event for {}/{}/{}", self.selected_day, self.current_month + 1, self.current_year));
        mvwprintw(dialog, 3, 2, "Title: ");
        mvwprintw(dialog, 5, 2, "Description (optional): ");
        mvwprintw(dialog, 8, 2, "Press Enter to save, Esc to cancel");
        
        wrefresh(dialog);
        
        // Create input fields
        echo();
        curs_set(CURSOR_VISIBILITY::CURSOR_VISIBLE);
        
        // Title input
        wmove(dialog, 3, 9);
        wrefresh(dialog);
        let mut title = String::new();
        let mut ch = getch();
        while ch != KEY_ENTER && ch != 10 && ch != 27 {
            if ch == KEY_BACKSPACE || ch == 127 {
                if !title.is_empty() {
                    title.pop();
                    mvwprintw(dialog, 3, 9, &format!("{:<30}", title));
                    wmove(dialog, 3, 9 + title.len() as i32);
                }
            } else if ch >= 32 && ch <= 126 && title.len() < 30 {
                title.push(ch as u8 as char);
                mvwaddch(dialog, 3, 9 + title.len() as i32 - 1, ch as u32);
            }
            wrefresh(dialog);
            ch = getch();
        }
        
        if ch == 27 {
            // Escape key pressed
            delwin(dialog);
            delwin(win);
            noecho();
            curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
            return Ok(None);
        }
        
        // Description input
        wmove(dialog, 5, 25);
        wrefresh(dialog);
        let mut description = String::new();
        ch = getch();
        while ch != KEY_ENTER && ch != 10 && ch != 27 {
            if ch == KEY_BACKSPACE || ch == 127 {
                if !description.is_empty() {
                    description.pop();
                    mvwprintw(dialog, 5, 25, &format!("{:<30}", description));
                    wmove(dialog, 5, 25 + description.len() as i32);
                }
            } else if ch >= 32 && ch <= 126 && description.len() < 30 {
                description.push(ch as u8 as char);
                mvwaddch(dialog, 5, 25 + description.len() as i32 - 1, ch as u32);
            }
            wrefresh(dialog);
            ch = getch();
        }
        
        noecho();
        curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
        
        if ch == 27 {
            // Escape key pressed
            delwin(dialog);
            delwin(win);
            return Ok(None);
        }
        
        // Create event
        let event_date = match NaiveDate::from_ymd_opt(
            self.current_year as i32,
            (self.current_month + 1) as u32,
            self.selected_day,
        ) {
            Some(date) => date,
            None => {
                delwin(dialog);
                delwin(win);
                return Err(DbError::InvalidDate);
            }
        };
        
        let event = Event {
            id: None,
            title,
            description: if description.is_empty() { None } else { Some(description) },
            date: event_date,
            created_at: None,
        };
        
        delwin(dialog);
        delwin(win);
        
        Ok(Some(event))
    }

    pub async fn run(&mut self) -> Result<(), DbError> {
        self.draw_calendar();

        loop {
            let ch = getch();
            match ch {
                KEY_LEFT => {
                    if self.selected_day > 1 {
                        self.selected_day -= 1;
                    }
                }
                KEY_RIGHT => {
                    let total_days = Calendar {
                        year: self.current_year,
                        month: self.current_month,
                    }
                    .get_total_days_in_month();
                    
                    if self.selected_day < total_days {
                        self.selected_day += 1;
                    }
                }
                KEY_UP => {
                    if self.selected_day > 7 {
                        self.selected_day -= 7;
                    } else {
                        // Move to previous month
                        let prev_cal = Calendar {
                            year: self.current_year,
                            month: self.current_month,
                        }
                        .prev_month();
                        
                        self.current_year = prev_cal.year;
                        self.current_month = prev_cal.month;
                        
                        let total_days = prev_cal.get_total_days_in_month();
                        self.selected_day = total_days - (7 - self.selected_day);
                        
                        self.load_events().await?;
                    }
                }
                KEY_DOWN => {
                    let total_days = Calendar {
                        year: self.current_year,
                        month: self.current_month,
                    }
                    .get_total_days_in_month();
                    
                    if self.selected_day + 7 <= total_days {
                        self.selected_day += 7;
                    } else {
                        // Move to next month
                        let next_cal = Calendar {
                            year: self.current_year,
                            month: self.current_month,
                        }
                        .next_month();
                        
                        self.current_year = next_cal.year;
                        self.current_month = next_cal.month;
                        self.selected_day = self.selected_day + 7 - total_days;
                        
                        self.load_events().await?;
                    }
                }
                KEY_ENTER | 10 => {
                    // Show dialog to add/edit event
                    if let Some(event) = self.show_event_dialog().await? {
                        let mut db = self.db.lock().await;
                        db.add_event(&event).await?;
                        drop(db);
                        self.load_events().await?;
                    }
                }
                113 | 81 => {
                    // 'q' or 'Q' to quit
                    break;
                }
                _ => {}
            }
            
            self.draw_calendar();
        }

        Ok(())
    }
}
