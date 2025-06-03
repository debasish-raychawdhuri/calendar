use crate::db::{Database, DbError, Event};
use chrono::NaiveDate;
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
    let mut created_at = None;
    
    if let Some(id) = event_id {
        let db_lock = db.lock().await;
        let event = db_lock.get_event(id).await?;
        drop(db_lock);
        
        title = event.title;
        description = event.description.unwrap_or_default();
        created_at = event.created_at;
    }
    
    // Create a panel to cover the entire screen (prevents text from showing through)
    let background = newwin(LINES(), COLS(), 0, 0);
    wbkgd(background, COLOR_PAIR(1)); // COLOR_DEFAULT
    wrefresh(background);

    // Create dialog window
    let height = 14;
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
    let desc_visible_lines = 4; // Increased visible lines
    
    let mut current_field = 0; // 0 = title, 1 = description
    let mut desc_scroll: usize = 0;   // Scroll position for description
    
    // Cursor positions for editing
    let mut title_cursor_pos = title.len();
    let mut desc_cursor_pos = description.len();
    
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
    
    loop {
        // Clear and redraw the input fields
        if current_field == 0 {
            // Title field active
            wattron(dialog, A_BOLD());
            
            // Clear title field
            for i in 0..title_max_width {
                mvwaddch(dialog, title_y, title_x + i, ' ' as u32);
            }
            
            // Display title (single line only for title)
            let display_len = title.len().min(title_max_width as usize);
            mvwprintw(dialog, title_y, title_x, &title[0..display_len]);
            
            wattroff(dialog, A_BOLD());
            
            // Position cursor at the current position
            let cursor_x = title_cursor_pos.min(title_max_width as usize);
            wmove(dialog, title_y, title_x + cursor_x as i32);
        } else {
            // Description field active
            wattron(dialog, A_BOLD());
            
            // Clear description field area
            for y in 0..desc_visible_lines {
                for x in 0..desc_max_width {
                    mvwaddch(dialog, desc_y + y, desc_x + x, ' ' as u32);
                }
            }
            
            // Split description into wrapped lines
            let mut all_lines = Vec::new();
            let mut line_breaks = Vec::new();
            let mut char_count = 0;
            
            for line in description.split('\n') {
                if !line.is_empty() {
                    let wrapped = wrap_text(line, desc_max_width as usize);
                    all_lines.extend(wrapped);
                    
                    // Track character positions for cursor placement
                    for c in line.chars() {
                        char_count += 1;
                        if char_count >= desc_cursor_pos {
                            break;
                        }
                    }
                    char_count += 1; // For the newline
                } else {
                    all_lines.push(String::new());
                    char_count += 1; // For the newline
                }
                line_breaks.push(char_count);
            }
            
            // Display description with scrolling
            let desc_visible_lines_usize = desc_visible_lines as usize;
            
            for (i, line) in all_lines.iter().enumerate().skip(desc_scroll).take(desc_visible_lines_usize) {
                mvwprintw(dialog, desc_y + (i - desc_scroll) as i32, desc_x, line);
            }
            
            // Show scroll indicators if needed
            if desc_scroll > 0 {
                mvwprintw(dialog, desc_y, width - 4, "↑");
            }
            if all_lines.len() > desc_scroll + desc_visible_lines_usize {
                mvwprintw(dialog, desc_y + desc_visible_lines - 1, width - 4, "↓");
            }
            
            wattroff(dialog, A_BOLD());
            
            // Calculate cursor position in the wrapped text
            let (cursor_line, cursor_col) = find_cursor_position(&description, desc_cursor_pos, desc_max_width as usize);
            
            // Ensure cursor is visible (adjust scroll if needed)
            if cursor_line < desc_scroll {
                desc_scroll = cursor_line;
            } else if cursor_line >= desc_scroll + desc_visible_lines_usize {
                desc_scroll = cursor_line - desc_visible_lines_usize + 1;
            }
            
            // Position cursor
            let visible_line_idx = cursor_line - desc_scroll;
            wmove(dialog, desc_y + visible_line_idx as i32, desc_x + cursor_col as i32);
        }
        
        wrefresh(dialog);
        
        let ch = wgetch(dialog);
        match ch {
            KEY_ENTER | 10 => break, // Enter key - save and exit
            27 => {
                // Escape key - cancel
                delwin(dialog);
                delwin(background);
                curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
                return Ok(None);
            },
            9 => {
                // Tab key - switch fields
                current_field = 1 - current_field;
            },
            KEY_BACKSPACE | 127 => {
                if current_field == 0 && !title.is_empty() && title_cursor_pos > 0 {
                    // Remove character before cursor in title
                    title_cursor_pos -= 1;
                    title.remove(title_cursor_pos);
                } else if current_field == 1 && !description.is_empty() && desc_cursor_pos > 0 {
                    // Remove character before cursor in description
                    desc_cursor_pos -= 1;
                    description.remove(desc_cursor_pos);
                }
            },
            KEY_DC => { // Delete key
                if current_field == 0 && !title.is_empty() && title_cursor_pos < title.len() {
                    // Remove character at cursor in title
                    title.remove(title_cursor_pos);
                } else if current_field == 1 && !description.is_empty() && desc_cursor_pos < description.len() {
                    // Remove character at cursor in description
                    description.remove(desc_cursor_pos);
                }
            },
            KEY_LEFT => {
                if current_field == 0 && title_cursor_pos > 0 {
                    title_cursor_pos -= 1;
                } else if current_field == 1 && desc_cursor_pos > 0 {
                    desc_cursor_pos -= 1;
                }
            },
            KEY_RIGHT => {
                if current_field == 0 && title_cursor_pos < title.len() {
                    title_cursor_pos += 1;
                } else if current_field == 1 && desc_cursor_pos < description.len() {
                    desc_cursor_pos += 1;
                }
            },
            KEY_UP => {
                if current_field == 1 {
                    // Find the previous line's equivalent position
                    let mut prev_line_start = 0;
                    let mut current_line_start = 0;
                    let mut found = false;
                    
                    for (i, c) in description.char_indices() {
                        if i >= desc_cursor_pos {
                            break;
                        }
                        if c == '\n' {
                            prev_line_start = current_line_start;
                            current_line_start = i + 1;
                            found = true;
                        }
                    }
                    
                    if found {
                        // Calculate position in previous line
                        let current_offset = desc_cursor_pos - current_line_start;
                        let prev_line_length = current_line_start - prev_line_start - 1;
                        desc_cursor_pos = prev_line_start + current_offset.min(prev_line_length);
                    } else if desc_scroll > 0 {
                        desc_scroll -= 1;
                    }
                }
            },
            KEY_DOWN => {
                if current_field == 1 {
                    // Find the next line's equivalent position
                    let mut current_line_start = 0;
                    let mut next_line_start = description.len();
                    let mut found = false;
                    
                    for (i, c) in description.char_indices() {
                        if i < desc_cursor_pos && c == '\n' {
                            current_line_start = i + 1;
                        } else if i >= desc_cursor_pos && c == '\n' {
                            next_line_start = i + 1;
                            found = true;
                            break;
                        }
                    }
                    
                    if found {
                        // Calculate position in next line
                        let current_offset = desc_cursor_pos - current_line_start;
                        let next_line_length = if description[next_line_start..].contains('\n') {
                            description[next_line_start..].find('\n').unwrap()
                        } else {
                            description.len() - next_line_start
                        };
                        desc_cursor_pos = next_line_start + current_offset.min(next_line_length);
                    } else {
                        // Count total lines after wrapping
                        let mut line_count = 0;
                        for line in description.split('\n') {
                            line_count += wrap_text(line, desc_max_width as usize).len();
                            if !line.is_empty() && line != description.split('\n').last().unwrap() {
                                line_count += 1; // For explicit newlines
                            }
                        }
                        
                        let desc_visible_lines_usize = desc_visible_lines as usize;
                        if line_count > desc_scroll + desc_visible_lines_usize {
                            desc_scroll += 1;
                        }
                    }
                }
            },
            13 => { // Enter key for newline in description
                if current_field == 1 {
                    description.insert(desc_cursor_pos, '\n');
                    desc_cursor_pos += 1;
                }
            },
            _ => {
                if ch >= 32 && ch <= 126 {
                    if current_field == 0 && title.len() < 100 { // Reasonable title length limit
                        title.insert(title_cursor_pos, ch as u8 as char);
                        title_cursor_pos += 1;
                    } else if current_field == 1 && description.len() < 1000 { // Increased description length limit
                        description.insert(desc_cursor_pos, ch as u8 as char);
                        desc_cursor_pos += 1;
                    }
                }
            }
        }
    }
    
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
    
    // Create or update the event
    let event = Event {
        id: event_id,
        title,
        description: if description.is_empty() { None } else { Some(description) },
        date,
        created_at,
    };
    
    delwin(dialog);
    delwin(background);
    
    Ok(Some(event))
}

// Function to edit an existing event
pub async fn edit_event(
    db: &Arc<Mutex<Database>>,
    event_id: i32,
) -> Result<(), DbError> {
    // Get the existing event
    let db_lock = db.lock().await;
    let event = db_lock.get_event(event_id).await?;
    let date = event.date;
    drop(db_lock);
    
    // Show dialog to edit the event
    if let Some(updated_event) = show_event_dialog(db, date, Some(event_id)).await? {
        // Save the updated event
        let db_lock = db.lock().await;
        db_lock.update_event(&updated_event).await?;
    }
    
    Ok(())
}

// Function to confirm deletion of an event
pub fn confirm_delete_event() -> bool {
    // Create a panel to cover the entire screen
    let background = newwin(LINES(), COLS(), 0, 0);
    wbkgd(background, COLOR_PAIR(1)); // COLOR_DEFAULT
    wrefresh(background);
    
    // Create dialog window
    let height = 6;
    let width = 50;
    let starty = (LINES() - height) / 2;
    let startx = (COLS() - width) / 2;
    
    let dialog = newwin(height, width, starty, startx);
    box_(dialog, 0, 0);
    wbkgd(dialog, COLOR_PAIR(6)); // COLOR_DIALOG
    
    // Dialog content
    mvwprintw(dialog, 1, 2, "Confirm Delete");
    mvwprintw(dialog, 2, 2, "Are you sure you want to delete this event?");
    mvwprintw(dialog, 4, 2, "Press Y to confirm, any other key to cancel");
    
    wrefresh(dialog);
    
    // Wait for key press
    let ch = wgetch(dialog);
    
    delwin(dialog);
    delwin(background);
    
    ch == 'y' as i32 || ch == 'Y' as i32
}
