use crate::calendar::{Calendar, DayOfWeek};
use crate::db::{Database, DbError, Event};
use crate::google_calendar::{GoogleCalendarClient, GoogleCredentials};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeZone, Utc};
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
const COLOR_SELECTED_EVENT: i16 = 7;
const COLOR_SELECTED_TODAY: i16 = 8;
const COLOR_HEADER: i16 = 9;

/// View modes for the calendar UI
#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Calendar,  // Main calendar view
    EventList, // Event list view
}

pub struct CalendarUI {
    db: Arc<Mutex<Database>>,
    current_year: u16,
    current_month: u8,
    selected_day: u32,
    events_cache: Vec<Event>,
    view_mode: ViewMode,
    selected_event_index: usize,
    google_client: Option<GoogleCalendarClient>,
}
impl CalendarUI {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        let today = Calendar::get_today();
        
        // Try to load Google credentials and create client
        let google_client = GoogleCredentials::load().map(|creds| {
            GoogleCalendarClient::new(&creds.client_id, &creds.client_secret)
        });
        
        CalendarUI {
            db,
            current_year: today.2,
            current_month: today.1,
            selected_day: today.0,
            events_cache: Vec::new(),
            view_mode: ViewMode::Calendar,
            selected_event_index: 0,
            google_client,
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
        timeout(100); // Set getch timeout for non-blocking input

        // Initialize color pairs
        init_pair(COLOR_DEFAULT, COLOR_WHITE, COLOR_BLACK);
        init_pair(COLOR_HIGHLIGHT, COLOR_RED, COLOR_BLACK);
        init_pair(COLOR_TODAY, COLOR_GREEN, COLOR_BLACK);
        init_pair(COLOR_EVENT, COLOR_CYAN, COLOR_BLACK);
        init_pair(COLOR_SELECTED, COLOR_BLACK, COLOR_WHITE);
        init_pair(COLOR_DIALOG, COLOR_BLACK, COLOR_CYAN);
        init_pair(COLOR_SELECTED_EVENT, COLOR_BLACK, COLOR_CYAN);
        init_pair(COLOR_SELECTED_TODAY, COLOR_BLACK, COLOR_GREEN);
        init_pair(COLOR_HEADER, COLOR_YELLOW, COLOR_BLACK);

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

        // Draw border around the entire screen
        box_(stdscr(), 0, 0);

        // Calculate calendars for previous, current, and next month
        let prev_cal = cal.prev_month();
        let next_cal = cal.next_month();
        
        // Calculate positions for the three calendars with minimal spacing
        let cal_width = 28; // Width for each calendar
        let gap = 3;        // Gap between calendars (3 characters)
        
        // Calculate positions to distribute calendars evenly with minimal gaps
        let total_width = 3 * cal_width + 2 * gap;
        let start_x = (COLS() - total_width) / 2;
        
        let left_x = start_x;
        let center_x = left_x + cal_width + gap;
        let right_x = center_x + cal_width + gap;
        
        // Draw all three calendars side by side with minimal spacing
        self.draw_month_calendar(&prev_cal, left_x, false, false);
        self.draw_month_calendar(&cal, center_x, is_current_month, true);
        self.draw_month_calendar(&next_cal, right_x, false, false);

        // Print navigation help
        attron(A_BOLD());
        mvprintw(
            LINES() - 2,
            2,
            "Arrow keys: Navigate | Enter: Add | Tab: Events | G: Google | q: Quit",
        );
        attroff(A_BOLD());

        // Display events for selected day
        self.draw_events_panel();

        refresh();
    }
    
    fn draw_month_calendar(&self, cal: &Calendar, start_x: i32, is_current_month: bool, is_selected_month: bool) {
        let today = Calendar::get_today();
        let is_today_month = cal.year == today.2 && cal.month == today.1;
        
        // Calculate width for each month - use fixed width
        let width = 28; // Fixed width for consistent layout
        
        // Print month and year
        let month_name = cal.get_month_name();
        let title = format!("{} {}", month_name, cal.year);
        
        // Calculate center position for the title within this month's area
        let title_x = start_x + (width - title.len() as i32) / 2;
        
        // Use different color for selected month
        if is_selected_month {
            attron(COLOR_PAIR(COLOR_HEADER) | A_BOLD());
        } else {
            attron(COLOR_PAIR(COLOR_DEFAULT));
        }
        
        // Clear the entire title area first to ensure clean display
        for i in 0..width {
            mvprintw(1, start_x + i, " ");
        }
        
        // Print the title centered in the cleared area
        mvprintw(1, title_x, &title);
        
        if is_selected_month {
            attroff(COLOR_PAIR(COLOR_HEADER) | A_BOLD());
        } else {
            attroff(COLOR_PAIR(COLOR_DEFAULT));
        }

        // Print day names
        let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for (i, day) in day_names.iter().enumerate() {
            if i == 0 {
                attron(COLOR_PAIR(COLOR_HIGHLIGHT) | A_BOLD());
            } else {
                attron(COLOR_PAIR(COLOR_DEFAULT) | A_BOLD());
            }
            mvprintw(3, start_x + i as i32 * 4, day);
            attroff(if i == 0 { COLOR_PAIR(COLOR_HIGHLIGHT) } else { COLOR_PAIR(COLOR_DEFAULT) } | A_BOLD());
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
                let x = start_x + weekday * 4;
                let y = 5 + week;

                if week == 0 && weekday < first_day_offset || day_counter > total_days {
                    // Empty cell
                    mvprintw(y, x, "   ");
                } else {
                    // Determine cell color
                    let is_today = is_today_month && day_counter == today.0;
                    let is_selected = is_selected_month && day_counter == self.selected_day;
                    let has_event = is_selected_month && self.has_event(day_counter);

                    let color = if is_selected && is_today {
                        COLOR_SELECTED_TODAY
                    } else if is_selected {
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

                    let attrs = if is_selected || is_today || has_event {
                        A_BOLD()
                    } else {
                        0
                    };

                    attron(COLOR_PAIR(color) | attrs);
                    mvprintw(y, x, &format!("{:2}", day_counter));
                    attroff(COLOR_PAIR(color) | attrs);

                    day_counter += 1;
                }
            }
        }
    }

    fn draw_events_panel(&self) {
        let events = self.get_events_for_day(self.selected_day);
        let panel_width = 40;
        let panel_x = COLS() - panel_width - 2;
        let panel_height = LINES() - 6;

        // Draw panel border
        for y in 3..LINES() - 3 {
            mvaddch(y, panel_x - 1, ACS_VLINE());
        }

        // Panel title
        attron(COLOR_PAIR(COLOR_HEADER) | A_BOLD());
        mvprintw(
            3,
            panel_x,
            &format!(" Events for {}/{}/{} ", self.selected_day, self.current_month + 1, self.current_year),
        );
        attroff(COLOR_PAIR(COLOR_HEADER) | A_BOLD());

        // List events
        if events.is_empty() {
            mvprintw(5, panel_x + 2, "No events for this day");
        } else {
            for (i, event) in events.iter().enumerate() {
                if i >= ((panel_height - 2) / 2) as usize {
                    // Limit display based on panel height
                    mvprintw(5 + i as i32 * 2, panel_x + 2, "... more events");
                    break;
                }
                
                let is_selected = self.view_mode == ViewMode::EventList && i == self.selected_event_index;
                
                if is_selected {
                    attron(COLOR_PAIR(COLOR_SELECTED_EVENT) | A_BOLD());
                } else {
                    attron(A_BOLD());
                }
                
                // Format event title with time if available
                let display_title = if let Some(start_time) = &event.start_time {
                    // Convert UTC time to local time for display
                    let naive_datetime = chrono::NaiveDateTime::new(event.date, *start_time);
                    let utc_datetime = Utc.from_utc_datetime(&naive_datetime);
                    let local_datetime = utc_datetime.with_timezone(&Local);
                    
                    // Format the time in local timezone
                    let time_str = local_datetime.format("%H:%M").to_string();
                    
                    // Add duration if available
                    let duration_str = if let Some(duration) = event.duration_minutes {
                        let end_time = utc_datetime + chrono::Duration::minutes(duration as i64);
                        let local_end_time = end_time.with_timezone(&Local);
                        format!(" - {}", local_end_time.format("%H:%M"))
                    } else {
                        String::new()
                    };
                    
                    format!("{}{}: {}", time_str, duration_str, event.title)
                } else {
                    event.title.clone()
                };
                
                // Truncate title if too long for panel
                let title_display = if display_title.len() > (panel_width - 4) as usize {
                    format!("{}...", &display_title[0..(panel_width - 7) as usize])
                } else {
                    display_title
                };
                
                mvprintw(5 + i as i32 * 2, panel_x + 2, &title_display);
                
                if is_selected {
                    attroff(COLOR_PAIR(COLOR_SELECTED_EVENT) | A_BOLD());
                } else {
                    attroff(A_BOLD());
                }
                
                if let Some(desc) = &event.description {
                    let desc_short = if desc.len() > panel_width as usize - 4 {
                        format!("{}...", &desc[0..panel_width as usize - 7])
                    } else {
                        desc.clone()
                    };
                    
                    if is_selected {
                        attron(COLOR_PAIR(COLOR_SELECTED_EVENT));
                    }
                    
                    mvprintw(6 + i as i32 * 2, panel_x + 4, &desc_short);
                    
                    if is_selected {
                        attroff(COLOR_PAIR(COLOR_SELECTED_EVENT));
                    }
                }
            }
        }
    }

    async fn show_event_dialog(&self) -> Result<Option<Event>, DbError> {
        // Create event date
        let event_date = match NaiveDate::from_ymd_opt(
            self.current_year as i32,
            (self.current_month + 1) as u32,
            self.selected_day,
        ) {
            Some(date) => date,
            None => return Err(DbError::InvalidDate),
        };
        
        // Use the shared dialog function from edit_event module
        crate::edit_event::show_event_dialog(&self.db, event_date, None).await
    }

    pub async fn run(&mut self) -> Result<(), DbError> {
        self.draw_calendar();

        loop {
            let ch = getch();
            if ch == ERR {
                // No input, continue loop
                continue;
            }
            
            // Check for quit command in any mode
            if ch == 113 || ch == 81 { // 'q' or 'Q'
                return Ok(());
            }
            
            match self.view_mode {
                ViewMode::Calendar => self.handle_calendar_input(ch).await?,
                ViewMode::EventList => self.handle_event_list_input(ch).await?,
            }
            
            self.draw_calendar();
        }
    }
    
    async fn handle_calendar_input(&mut self, ch: i32) -> Result<(), DbError> {
        match ch {
            KEY_LEFT => {
                if self.selected_day > 1 {
                    self.selected_day -= 1;
                } else {
                    // Move to previous month
                    let prev_cal = Calendar {
                        year: self.current_year,
                        month: self.current_month,
                    }
                    .prev_month();
                    
                    self.current_year = prev_cal.year;
                    self.current_month = prev_cal.month;
                    self.selected_day = prev_cal.get_total_days_in_month();
                    
                    self.load_events().await?;
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
                } else {
                    // Move to next month
                    let next_cal = Calendar {
                        year: self.current_year,
                        month: self.current_month,
                    }
                    .next_month();
                    
                    self.current_year = next_cal.year;
                    self.current_month = next_cal.month;
                    self.selected_day = 1;
                    
                    self.load_events().await?;
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
                    let day_offset = 7 - self.selected_day;
                    if total_days >= day_offset {
                        self.selected_day = total_days - day_offset + 1;
                    } else {
                        self.selected_day = total_days;
                    }
                    
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
                    if self.selected_day > next_cal.get_total_days_in_month() {
                        self.selected_day = next_cal.get_total_days_in_month();
                    }
                    
                    self.load_events().await?;
                }
            }
            KEY_ENTER | 10 => {
                // Show dialog to add/edit event
                if let Some(event) = self.show_event_dialog().await? {
                    let db = self.db.lock().await;
                    db.add_event(&event).await?;
                    drop(db);
                    self.load_events().await?;
                }
            }
            9 => { // Tab key
                let events = self.get_events_for_day(self.selected_day);
                if !events.is_empty() {
                    self.view_mode = ViewMode::EventList;
                    self.selected_event_index = 0;
                }
            }
            KEY_HOME => {
                // Go to first day of month
                self.selected_day = 1;
            }
            KEY_END => {
                // Go to last day of month
                self.selected_day = Calendar {
                    year: self.current_year,
                    month: self.current_month,
                }
                .get_total_days_in_month();
            }
            KEY_PPAGE => {
                // Previous month
                let prev_cal = Calendar {
                    year: self.current_year,
                    month: self.current_month,
                }
                .prev_month();
                
                self.current_year = prev_cal.year;
                self.current_month = prev_cal.month;
                
                let total_days = prev_cal.get_total_days_in_month();
                if self.selected_day > total_days {
                    self.selected_day = total_days;
                }
                
                self.load_events().await?;
            }
            KEY_NPAGE => {
                // Next month
                let next_cal = Calendar {
                    year: self.current_year,
                    month: self.current_month,
                }
                .next_month();
                
                self.current_year = next_cal.year;
                self.current_month = next_cal.month;
                
                let total_days = next_cal.get_total_days_in_month();
                if self.selected_day > total_days {
                    self.selected_day = total_days;
                }
                
                self.load_events().await?;
            }
            103 | 71 => { // 'g' or 'G' for Google Calendar
                self.handle_google_calendar().await?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    async fn handle_event_list_input(&mut self, ch: i32) -> Result<(), DbError> {
        let events = self.get_events_for_day(self.selected_day);
        if events.is_empty() {
            self.view_mode = ViewMode::Calendar;
            return Ok(());
        }
        
        match ch {
            KEY_UP => {
                if self.selected_event_index > 0 {
                    self.selected_event_index -= 1;
                }
            },
            KEY_DOWN => {
                if self.selected_event_index < events.len() - 1 {
                    self.selected_event_index += 1;
                }
            },
            9 => { // Tab key
                self.view_mode = ViewMode::Calendar;
            },
            KEY_ENTER | 10 => {
                if let Some(event_id) = events[self.selected_event_index].id {
                    // Show event details with edit/delete options
                    self.show_event_details(event_id).await?;
                }
            },
            KEY_DC => { // Delete key
                if let Some(event_id) = events[self.selected_event_index].id {
                    if crate::edit_event::confirm_delete_event() {
                        let db = self.db.lock().await;
                        let _ = db.delete_event(event_id).await;
                        drop(db);
                        self.load_events().await?;
                        
                        if self.selected_event_index >= self.get_events_for_day(self.selected_day).len() && self.selected_event_index > 0 {
                            self.selected_event_index -= 1;
                        }
                    }
                }
            },
            101 | 69 => { // 'e' or 'E' for Edit
                if let Some(event_id) = events[self.selected_event_index].id {
                    // Edit the selected event
                    crate::edit_event::edit_event(&self.db, event_id).await?;
                    self.load_events().await?;
                }
            },
            _ => {}
        }
        
        Ok(())
    }
    
    async fn show_event_details(&mut self, event_id: i32) -> Result<(), DbError> {
        let db = self.db.lock().await;
        let event = db.get_event(event_id).await?;
        drop(db);
        
        // Create a panel to cover the entire screen
        let background = newwin(LINES(), COLS(), 0, 0);
        wbkgd(background, COLOR_PAIR(COLOR_DEFAULT));
        wrefresh(background);
        
        // Create dialog window
        let height = 18;
        let width = 70;
        let starty = (LINES() - height) / 2;
        let startx = (COLS() - width) / 2;
        
        let dialog = newwin(height, width, starty, startx);
        box_(dialog, 0, 0);
        wbkgd(dialog, COLOR_PAIR(COLOR_DIALOG));
        
        // Dialog title
        mvwprintw(dialog, 1, 2, "Event Details");
        mvwprintw(dialog, 3, 2, &format!("Date: {}", event.date));
        
        // Display time information if available
        let mut time_info_y = 4;
        if let Some(start_time) = event.start_time {
            // Convert UTC time to local time for display
            let naive_datetime = chrono::NaiveDateTime::new(event.date, start_time);
            let utc_datetime = Utc.from_utc_datetime(&naive_datetime);
            let local_datetime = utc_datetime.with_timezone(&Local);
            
            // Format the time in local timezone
            let time_str = local_datetime.format("%H:%M").to_string();
            
            // Add duration if available
            let time_display = if let Some(duration) = event.duration_minutes {
                let end_time = utc_datetime + chrono::Duration::minutes(duration as i64);
                let local_end_time = end_time.with_timezone(&Local);
                format!("Time: {} - {} ({}m)", time_str, local_end_time.format("%H:%M"), duration)
            } else {
                format!("Time: {}", time_str)
            };
            
            mvwprintw(dialog, time_info_y, 2, &time_display);
            time_info_y += 1;
        }
        
        // Function to wrap text to fit within width
        let wrap_text = |text: &str, max_width: usize| -> Vec<String> {
            let mut lines = Vec::new();
            let mut current_line = String::new();
            
            for word in text.split_whitespace() {
                if current_line.len() + word.len() + 1 > max_width {
                    lines.push(current_line);
                    current_line = word.to_string();
                } else {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(word);
                }
            }
            
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            
            // Handle empty text
            if lines.is_empty() {
                lines.push(String::new());
            }
            
            lines
        };
        
        // Wrap title if needed
        let title_max_width = width - 10; // "Title: " + padding
        let title_wrapped = wrap_text(&event.title, title_max_width as usize);
        
        // Display title (potentially multi-line)
        mvwprintw(dialog, time_info_y, 2, "Title:");
        for (i, line) in title_wrapped.iter().enumerate() {
            mvwprintw(dialog, time_info_y + i as i32, 9, line);
        }
        
        // Adjust starting position for description based on title height
        let desc_start_y = time_info_y + title_wrapped.len() as i32 + 1;
        
        // Action buttons at the bottom
        mvwprintw(dialog, height - 3, 2, "[E]dit | [D]elete | Any other key: Close");
        
        if let Some(desc) = &event.description {
            mvwprintw(dialog, desc_start_y, 2, "Description:");
            
            // Calculate available space for description
            let desc_width = width - 8; // Leave padding for borders
            let desc_area_height = height - desc_start_y - 5; // Leave room for buttons and borders
            
            // Wrap description text to fit within dialog
            let mut wrapped_lines = Vec::new();
            
            // First split by explicit newlines
            for paragraph in desc.split('\n') {
                if paragraph.is_empty() {
                    wrapped_lines.push(String::new());
                } else {
                    // Then wrap each paragraph
                    wrapped_lines.extend(wrap_text(paragraph, desc_width as usize - 2));
                }
            }
            
            // Display lines with scrolling if needed
            let visible_lines = desc_area_height as usize;
            let mut scroll_pos: usize = 0;
            let max_scroll = wrapped_lines.len().saturating_sub(visible_lines).max(0);
            let mut redraw = true;
            
            while redraw {
                if redraw {
                    // Clear the description area
                    for y in 0..desc_area_height {
                        for x in 0..desc_width-2 {
                            mvwaddch(dialog, desc_start_y + 1 + y, 4 + x, ' ' as u32);
                        }
                    }
                    
                    // Display visible lines with proper padding
                    for (i, line) in wrapped_lines.iter().enumerate().skip(scroll_pos).take(visible_lines) {
                        mvwprintw(dialog, desc_start_y + 1 + (i - scroll_pos) as i32, 4, line);
                    }
                    
                    // Show scroll indicators if needed
                    if scroll_pos > 0 {
                        mvwprintw(dialog, desc_start_y + 1, width - 5, "↑");
                    }
                    if scroll_pos < max_scroll {
                        mvwprintw(dialog, desc_start_y + desc_area_height, width - 5, "↓");
                    }
                    
                    wrefresh(dialog);
                    redraw = false;
                }
                
                // Handle scrolling and actions
                let ch = wgetch(dialog);
                match ch {
                    KEY_UP => {
                        if scroll_pos > 0 {
                            scroll_pos -= 1;
                            redraw = true;
                        }
                    },
                    KEY_DOWN => {
                        if scroll_pos < max_scroll {
                            scroll_pos += 1;
                            redraw = true;
                        }
                    },
                    101 | 69 => { // 'e' or 'E' for Edit
                        delwin(dialog);
                        delwin(background);
                        crate::edit_event::edit_event(&self.db, event_id).await?;
                        self.load_events().await?;
                        return Ok(());
                    },
                    100 | 68 => { // 'd' or 'D' for Delete
                        if crate::edit_event::confirm_delete_event() {
                            let db = self.db.lock().await;
                            let _ = db.delete_event(event_id).await;
                            drop(db);
                            self.load_events().await?;
                            delwin(dialog);
                            delwin(background);
                            return Ok(());
                        } else {
                            redraw = true;
                        }
                    },
                    _ => {
                        // Any other key closes the dialog
                        break;
                    }
                }
            }
        } else {
            mvwprintw(dialog, desc_start_y + 1, 4, "No description available");
            
            // Wait for key press
            let ch = wgetch(dialog);
            match ch {
                101 | 69 => { // 'e' or 'E' for Edit
                    delwin(dialog);
                    delwin(background);
                    crate::edit_event::edit_event(&self.db, event_id).await?;
                    self.load_events().await?;
                    return Ok(());
                },
                100 | 68 => { // 'd' or 'D' for Delete
                    if crate::edit_event::confirm_delete_event() {
                        let db = self.db.lock().await;
                        let _ = db.delete_event(event_id).await;
                        drop(db);
                        self.load_events().await?;
                        delwin(dialog);
                        delwin(background);
                        return Ok(());
                    }
                },
                _ => {}
            }
        }
        
        delwin(dialog);
        delwin(background);
        
        Ok(())
    }
    
    async fn handle_google_calendar(&mut self) -> Result<(), DbError> {
        // Create a clone of the necessary data to avoid borrow checker issues
        let google_client = &mut self.google_client;
        let db = Arc::clone(&self.db);
        let year = self.current_year;
        let month = self.current_month;
        
        // Call the Google Calendar handler with the cloned data
        let result = crate::ui_google::handle_google_calendar(
            google_client,
            &db,
            year,
            month,
        ).await;
        
        // Reload events after Google Calendar operations
        if result.is_ok() {
            self.load_events().await?;
        }
        
        result
    }
}
