use crate::db::{Database, DbError, Event};
use chrono::{Local, NaiveDate, NaiveTime, TimeZone, Utc};
use ncurses::*;
use std::sync::Arc;
use tokio::sync::Mutex;

// Function to show an event dialog (used for both creating and editing events)
pub async fn show_event_dialog(
    db: &Arc<Mutex<Database>>,
    date: NaiveDate,
    event_id: Option<i32>,
) -> Result<Option<Event>, DbError> {
    // If editing an existing event, get its data
    let mut title = String::new();
    let mut description = String::new();
    let mut start_time_str = String::new();
    let mut duration_str = String::new();
    let mut created_at = None;
    let mut start_time = None;
    let mut duration_minutes = None;
    
    if let Some(id) = event_id {
        let db_lock = db.lock().await;
        let event = db_lock.get_event(id).await?;
        drop(db_lock);
        
        title = event.title;
        description = event.description.unwrap_or_default();
        created_at = event.created_at;
        
        // Format existing start time if present (convert from UTC to local for display)
        if let Some(time) = event.start_time {
            // Create a datetime in UTC
            let naive_datetime = chrono::NaiveDateTime::new(event.date, time);
            let utc_datetime = Utc.from_utc_datetime(&naive_datetime);
            
            // Convert to local time for display
            let local_datetime = utc_datetime.with_timezone(&Local);
            start_time_str = local_datetime.format("%H:%M").to_string();
            
            // Keep the original UTC time for storage
            start_time = Some(time);
        }
        
        // Format existing duration if present
        if let Some(mins) = event.duration_minutes {
            duration_str = mins.to_string();
            duration_minutes = Some(mins);
        }
    }
    
    // Create a panel to cover the entire screen (prevents text from showing through)
    let background = newwin(LINES(), COLS(), 0, 0);
    wbkgd(background, COLOR_PAIR(1)); // COLOR_DEFAULT
    wrefresh(background);

    // Create dialog window
    let height = 18; // Increased height to accommodate new fields
    let width = 70;
    let starty = (LINES() - height) / 2;
    let startx = (COLS() - width) / 2;
    
    let dialog = newwin(height, width, starty, startx);
    box_(dialog, 0, 0);
    wbkgd(dialog, COLOR_PAIR(6)); // COLOR_DIALOG
    
    // Dialog title
    let action = if event_id.is_some() { "Edit" } else { "New" };
    mvwprintw(dialog, 1, 2, &format!("{} Event for {}", action, date));
    
    // Labels with clear separation from input areas
    mvwprintw(dialog, 3, 2, "Title:");
    mvwprintw(dialog, 5, 2, "Description (optional):");
    mvwprintw(dialog, 10, 2, "Start Time (HH:MM, optional):");
    mvwprintw(dialog, 12, 2, "Duration (minutes, optional):");
    
    mvwprintw(dialog, height - 2, 2, "Press Enter to save, Esc to cancel, Tab to switch fields");
    
    wrefresh(dialog);
    
    // Create input fields
    noecho(); // Don't echo characters automatically
    curs_set(CURSOR_VISIBILITY::CURSOR_VISIBLE);
    keypad(dialog, true); // Enable special keys in the dialog window
    
    // Define field areas with better spacing and border padding
    let title_x = 9;
    let title_y = 3;
    let title_max_width = width - title_x - 3; // Leave 3 chars for right border padding
    
    let desc_x = 4; // Start at the beginning of the line with padding
    let desc_y = 6; // One line below the label
    let desc_max_width = width - desc_x - 3; // Leave 3 chars for right border padding
    let desc_visible_lines = 3; // Visible lines for description
    
    // Increase the time_x value to prevent overwriting the label
    let time_x = 32; // Increased from 28 to provide more space after the label
    let time_y = 10; // Line for time input
    let time_max_width = 5; // HH:MM format
    
    // Also adjust duration_x for consistency
    let duration_x = 32; // Increased from 28 to match time_x
    let duration_y = 12; // Line for duration input
    let duration_max_width = 6; // Up to 999 minutes
    
    let mut current_field = 0; // 0 = title, 1 = description, 2 = start time, 3 = duration
    let mut desc_scroll: usize = 0;   // Scroll position for description
    
    // Cursor positions for editing
    let mut title_cursor_pos = title.len();
    let mut desc_cursor_pos = description.len();
    let mut time_cursor_pos = start_time_str.len();
    let mut duration_cursor_pos = duration_str.len();
    
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
    
    // Function to find cursor position in wrapped text
    let find_cursor_position = |text: &str, cursor_pos: usize, max_width: usize| -> (usize, usize) {
        if text.is_empty() || cursor_pos == 0 {
            return (0, 0);
        }
        
        let mut line_idx = 0;
        let mut col_idx = 0;
        let mut char_count = 0;
        
        for (i, c) in text.chars().enumerate() {
            if i == cursor_pos {
                break;
            }
            
            if c == '\n' {
                line_idx += 1;
                col_idx = 0;
            } else {
                col_idx += 1;
                if col_idx >= max_width {
                    line_idx += 1;
                    col_idx = 0;
                }
            }
            
            char_count += 1;
        }
        
        (line_idx, col_idx)
    };
    
    // Main input loop
    loop {
        // Clear input areas
        for x in 0..title_max_width {
            mvwaddch(dialog, title_y, title_x + x, ' ' as u32);
        }
        
        for y in 0..desc_visible_lines {
            for x in 0..desc_max_width {
                mvwaddch(dialog, desc_y + y, desc_x + x, ' ' as u32);
            }
        }
        
        for x in 0..time_max_width {
            mvwaddch(dialog, time_y, time_x + x, ' ' as u32);
        }
        
        for x in 0..duration_max_width {
            mvwaddch(dialog, duration_y, duration_x + x, ' ' as u32);
        }
        
        // Clear field indicators
        mvwaddch(dialog, title_y, title_x - 2, ' ' as u32);
        mvwaddch(dialog, desc_y, desc_x - 2, ' ' as u32);
        mvwaddch(dialog, time_y, time_x - 2, ' ' as u32);
        mvwaddch(dialog, duration_y, duration_x - 2, ' ' as u32);
        
        // Show which field is active with a visual indicator
        match current_field {
            0 => { mvwaddch(dialog, title_y, title_x - 2, '>' as u32); },
            1 => { mvwaddch(dialog, desc_y, desc_x - 2, '>' as u32); },
            2 => { mvwaddch(dialog, time_y, time_x - 2, '>' as u32); },
            3 => { mvwaddch(dialog, duration_y, duration_x - 2, '>' as u32); },
            _ => { }
        }
        
        // Display current field values
        if current_field == 0 {
            // Title field active
            wattron(dialog, A_BOLD() | COLOR_PAIR(5)); // Use a distinct color for active field
            mvwprintw(dialog, title_y, title_x, &title[..title.len().min(title_max_width as usize)]);
            wattroff(dialog, A_BOLD() | COLOR_PAIR(5));
            
            // Position cursor at the current position
            let cursor_x = title_cursor_pos.min(title_max_width as usize);
            wmove(dialog, title_y, title_x + cursor_x as i32);
        } else {
            // Title field inactive
            mvwprintw(dialog, title_y, title_x, &title[..title.len().min(title_max_width as usize)]);
        }
        
        if current_field == 1 {
            // Description field active
            wattron(dialog, A_BOLD() | COLOR_PAIR(5));
            
            // Split description into wrapped lines
            let mut all_lines = Vec::new();
            let mut line_breaks = Vec::new();
            let mut char_count = 0;
            
            for line in description.split('\n') {
                let wrapped = wrap_text(line, desc_max_width as usize);
                for wrapped_line in wrapped {
                    let line_len = wrapped_line.len();
                    all_lines.push(wrapped_line);
                    char_count += line_len + 1; // +1 for the newline
                    line_breaks.push(char_count);
                }
            }
            
            // Display visible lines with scrolling
            let visible_lines = all_lines.len().min(desc_visible_lines as usize);
            for i in 0..visible_lines {
                let line_idx = i + desc_scroll;
                if line_idx < all_lines.len() {
                    mvwprintw(dialog, desc_y + i as i32, desc_x, &all_lines[line_idx]);
                }
            }
            
            // Find cursor position in wrapped text
            let (cursor_line, cursor_col) = find_cursor_position(&description, desc_cursor_pos, desc_max_width as usize);
            
            // Adjust scroll position if cursor is outside visible area
            if cursor_line < desc_scroll {
                desc_scroll = cursor_line;
            } else if cursor_line >= desc_scroll + desc_visible_lines as usize {
                desc_scroll = cursor_line - desc_visible_lines as usize + 1;
            }
            
            // Position cursor
            if cursor_line - desc_scroll < desc_visible_lines as usize {
                wmove(dialog, desc_y + (cursor_line - desc_scroll) as i32, desc_x + cursor_col as i32);
            }
            
            wattroff(dialog, A_BOLD() | COLOR_PAIR(5));
        } else {
            // Description field inactive
            // Display description text without highlighting
            let wrapped_lines = wrap_text(&description, desc_max_width as usize);
            let visible_lines = wrapped_lines.len().min(desc_visible_lines as usize);
            
            for i in 0..visible_lines {
                mvwprintw(dialog, desc_y + i as i32, desc_x, &wrapped_lines[i]);
            }
        }
        
        // Display time field
        if current_field == 2 {
            // Time field active
            wattron(dialog, A_BOLD() | COLOR_PAIR(5));
            mvwprintw(dialog, time_y, time_x, &start_time_str);
            wattroff(dialog, A_BOLD() | COLOR_PAIR(5));
            
            // Position cursor
            wmove(dialog, time_y, time_x + time_cursor_pos as i32);
        } else {
            // Time field inactive
            mvwprintw(dialog, time_y, time_x, &start_time_str);
        }
        
        // Display duration field
        if current_field == 3 {
            // Duration field active
            wattron(dialog, A_BOLD() | COLOR_PAIR(5));
            mvwprintw(dialog, duration_y, duration_x, &duration_str);
            wattroff(dialog, A_BOLD() | COLOR_PAIR(5));
            
            // Position cursor
            wmove(dialog, duration_y, duration_x + duration_cursor_pos as i32);
        } else {
            // Duration field inactive
            mvwprintw(dialog, duration_y, duration_x, &duration_str);
        }
        
        // Display field name in status bar
        let field_name = match current_field {
            0 => "Title",
            1 => "Description",
            2 => "Start Time (HH:MM format)",
            3 => "Duration (minutes)",
            _ => "",
        };
        
        // Clear status line
        for x in 0..(width - 4) {
            mvwaddch(dialog, height - 3, 2 + x, ' ' as u32);
        }
        
        // Show current field in status line
        mvwprintw(dialog, height - 3, 2, &format!("Editing: {}", field_name));
        
        wrefresh(dialog);
        
        // Only show cursor for the active field
        if current_field == 0 {
            wmove(dialog, title_y, title_x + title_cursor_pos.min(title_max_width as usize) as i32);
        } else if current_field == 2 {
            wmove(dialog, time_y, time_x + time_cursor_pos as i32);
        } else if current_field == 3 {
            wmove(dialog, duration_y, duration_x + duration_cursor_pos as i32);
        }
        
        // Get user input
        let ch = wgetch(dialog);
        
        match ch {
            KEY_ENTER | 10 | 13 => { // Enter key
                // Save the event and exit
                break;
            },
            27 => { // Escape key
                // Cancel and exit
                delwin(dialog);
                delwin(background);
                return Ok(None);
            },
            9 => { // Tab key
                // Switch to next field
                current_field = (current_field + 1) % 4;
            },
            KEY_BTAB => { // Shift+Tab
                // Switch to previous field
                current_field = (current_field + 3) % 4;
            },
            KEY_BACKSPACE | 127 => { // Backspace key
                if current_field == 0 && !title.is_empty() && title_cursor_pos > 0 {
                    // Remove character before cursor in title
                    title_cursor_pos -= 1;
                    title.remove(title_cursor_pos);
                } else if current_field == 1 && !description.is_empty() && desc_cursor_pos > 0 {
                    // Remove character before cursor in description
                    desc_cursor_pos -= 1;
                    description.remove(desc_cursor_pos);
                } else if current_field == 2 && !start_time_str.is_empty() && time_cursor_pos > 0 {
                    // Remove character before cursor in time
                    time_cursor_pos -= 1;
                    start_time_str.remove(time_cursor_pos);
                } else if current_field == 3 && !duration_str.is_empty() && duration_cursor_pos > 0 {
                    // Remove character before cursor in duration
                    duration_cursor_pos -= 1;
                    duration_str.remove(duration_cursor_pos);
                }
            },
            KEY_DC => { // Delete key
                if current_field == 0 && !title.is_empty() && title_cursor_pos < title.len() {
                    // Remove character at cursor in title
                    title.remove(title_cursor_pos);
                } else if current_field == 1 && !description.is_empty() && desc_cursor_pos < description.len() {
                    // Remove character at cursor in description
                    description.remove(desc_cursor_pos);
                } else if current_field == 2 && !start_time_str.is_empty() && time_cursor_pos < start_time_str.len() {
                    // Remove character at cursor in time
                    start_time_str.remove(time_cursor_pos);
                } else if current_field == 3 && !duration_str.is_empty() && duration_cursor_pos < duration_str.len() {
                    // Remove character at cursor in duration
                    duration_str.remove(duration_cursor_pos);
                }
            },
            KEY_LEFT => {
                if current_field == 0 && title_cursor_pos > 0 {
                    title_cursor_pos -= 1;
                } else if current_field == 1 && desc_cursor_pos > 0 {
                    desc_cursor_pos -= 1;
                } else if current_field == 2 && time_cursor_pos > 0 {
                    time_cursor_pos -= 1;
                } else if current_field == 3 && duration_cursor_pos > 0 {
                    duration_cursor_pos -= 1;
                }
            },
            KEY_RIGHT => {
                if current_field == 0 && title_cursor_pos < title.len() {
                    title_cursor_pos += 1;
                } else if current_field == 1 && desc_cursor_pos < description.len() {
                    desc_cursor_pos += 1;
                } else if current_field == 2 && time_cursor_pos < start_time_str.len() {
                    time_cursor_pos += 1;
                } else if current_field == 3 && duration_cursor_pos < duration_str.len() {
                    duration_cursor_pos += 1;
                }
            },
            KEY_UP => {
                if current_field == 1 {
                    // Find the previous line's equivalent position
                    let mut prev_line_start = 0;
                    let mut current_line_start = 0;
                    let mut current_pos = 0;
                    
                    for (i, c) in description.chars().enumerate() {
                        if i == desc_cursor_pos {
                            break;
                        }
                        
                        if c == '\n' {
                            prev_line_start = current_line_start;
                            current_line_start = i + 1;
                        }
                        
                        current_pos += 1;
                    }
                    
                    if current_line_start > 0 {
                        // Calculate position in previous line
                        let offset = desc_cursor_pos - current_line_start;
                        let prev_line_length = current_line_start - prev_line_start - 1; // -1 for newline
                        
                        desc_cursor_pos = prev_line_start + offset.min(prev_line_length);
                    }
                } else if current_field == 0 {
                    // Move to the last field when pressing up from the first field
                    current_field = 3;
                }
            },
            KEY_DOWN => {
                if current_field == 1 {
                    // Find the next line's equivalent position
                    let mut line_start = 0;
                    let mut found_cursor = false;
                    let mut next_line_start = description.len();
                    
                    for (i, c) in description.chars().enumerate() {
                        if i == desc_cursor_pos {
                            found_cursor = true;
                        }
                        
                        if c == '\n' {
                            if !found_cursor {
                                line_start = i + 1;
                            } else {
                                next_line_start = i + 1;
                                break;
                            }
                        }
                    }
                    
                    if found_cursor && next_line_start < description.len() {
                        // Calculate position in next line
                        let offset = desc_cursor_pos - line_start;
                        let next_line_length = description[next_line_start..].find('\n')
                            .unwrap_or(description.len() - next_line_start);
                        
                        desc_cursor_pos = next_line_start + offset.min(next_line_length);
                    }
                }
            },
            _ => {
                if ch >= 32 && ch <= 126 {
                    // Regular character input
                    if current_field == 0 && title.len() < 100 { // Reasonable title length limit
                        title.insert(title_cursor_pos, ch as u8 as char);
                        title_cursor_pos += 1;
                    } else if current_field == 1 && description.len() < 1000 { // Increased description length limit
                        description.insert(desc_cursor_pos, ch as u8 as char);
                        desc_cursor_pos += 1;
                    } else if current_field == 2 && start_time_str.len() < 5 { // HH:MM format
                        // Only allow digits and colon for time
                        let c = ch as u8 as char;
                        if (c.is_digit(10) || c == ':') && start_time_str.len() < time_max_width as usize {
                            start_time_str.insert(time_cursor_pos, c);
                            time_cursor_pos += 1;
                        }
                    } else if current_field == 3 && duration_str.len() < 6 { // Up to 999 minutes
                        // Only allow digits for duration
                        let c = ch as u8 as char;
                        if c.is_digit(10) && duration_str.len() < duration_max_width as usize {
                            duration_str.insert(duration_cursor_pos, c);
                            duration_cursor_pos += 1;
                        }
                    }
                }
            }
        }
    }
    
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
    
    // Parse time and duration
    if !start_time_str.is_empty() {
        // Try to parse the time string as local time
        if let Ok(local_time) = NaiveTime::parse_from_str(&format!("{}:00", start_time_str), "%H:%M:%S") {
            // Create a datetime in the local timezone
            let local_date = Local::now().date_naive();
            let local_datetime = chrono::NaiveDateTime::new(local_date, local_time);
            let local_dt = Local.from_local_datetime(&local_datetime).unwrap();
            
            // Convert to UTC for storage
            let utc_dt = local_dt.with_timezone(&Utc);
            start_time = Some(utc_dt.time());
        }
    }
    
    if !duration_str.is_empty() {
        // Try to parse the duration string
        duration_minutes = duration_str.parse::<i32>().ok();
    }
    
    // Create or update the event
    let event = Event {
        id: event_id,
        title,
        description: if description.is_empty() { None } else { Some(description) },
        date,
        start_time,
        duration_minutes,
        created_at,
    };
    
    delwin(dialog);
    delwin(background);
    
    // Save the event to the database
    let db_lock = db.lock().await;
    
    if let Some(id) = event_id {
        // Update existing event
        db_lock.update_event(&event).await?;
        Ok(Some(event))
    } else {
        // Create new event
        let id = db_lock.add_event(&event).await?;
        let mut event = event;
        event.id = Some(id);
        Ok(Some(event))
    }
}
// Function to confirm deletion of an event
pub fn confirm_delete_event() -> bool {
    // Create a panel to cover the entire screen
    let background = newwin(LINES(), COLS(), 0, 0);
    wbkgd(background, COLOR_PAIR(1)); // COLOR_DEFAULT
    wrefresh(background);
    
    // Create confirmation dialog
    let height = 7;
    let width = 50;
    let starty = (LINES() - height) / 2;
    let startx = (COLS() - width) / 2;
    
    let dialog = newwin(height, width, starty, startx);
    box_(dialog, 0, 0);
    wbkgd(dialog, COLOR_PAIR(6)); // COLOR_DIALOG
    
    // Dialog content
    mvwprintw(dialog, 1, 2, "Confirm Delete");
    mvwprintw(dialog, 3, 2, "Are you sure you want to delete this event?");
    mvwprintw(dialog, 5, 2, "Press Y to confirm, any other key to cancel");
    
    wrefresh(dialog);
    
    // Get user input
    keypad(dialog, true);
    let ch = wgetch(dialog);
    
    // Clean up
    delwin(dialog);
    delwin(background);
    
    // Return true if user confirmed with 'y' or 'Y'
    ch == 'y' as i32 || ch == 'Y' as i32
}

// Alias for show_event_dialog for backward compatibility
pub async fn edit_event(db: &Arc<Mutex<Database>>, event_id: i32) -> Result<Option<Event>, DbError> {
    let db_lock = db.lock().await;
    let event = db_lock.get_event(event_id).await?;
    drop(db_lock);
    
    show_event_dialog(db, event.date, Some(event_id)).await
}
