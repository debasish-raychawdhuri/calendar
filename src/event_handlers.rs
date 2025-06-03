use crate::calendar::Calendar;
use crate::db::{Database, DbError, Event};
use crate::ui::ViewMode;
use futures::future::BoxFuture;
use ncurses::*;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

// Color constants
const COLOR_DEFAULT: i16 = 1;
const COLOR_DIALOG: i16 = 6;

pub async fn handle_event_list_input(
    ch: i32,
    db: &Arc<Mutex<Database>>,
    selected_day: u32,
    selected_event_index: &mut usize,
    view_mode: &mut ViewMode,
    get_events_for_day: impl Fn(u32) -> Vec<Event>,
    load_events: impl Fn() -> BoxFuture<'static, Result<(), DbError>>,
) -> Result<(), DbError> {
    let events = get_events_for_day(selected_day);
    if events.is_empty() {
        *view_mode = ViewMode::Calendar;
        return Ok(());
    }
    
    match ch {
        KEY_UP => {
            if *selected_event_index > 0 {
                *selected_event_index -= 1;
            }
        },
        KEY_DOWN => {
            if *selected_event_index < events.len() - 1 {
                *selected_event_index += 1;
            }
        },
        9 => { // Tab key
            *view_mode = ViewMode::Calendar;
        },
        KEY_ENTER | 10 => {
            if let Some(event_id) = events[*selected_event_index].id {
                // Show event details with edit/delete options
                show_event_details(event_id, db, load_events).await?;
            }
        },
        KEY_DC => { // Delete key
            if let Some(event_id) = events[*selected_event_index].id {
                if crate::edit_event::confirm_delete_event() {
                    let db_lock = db.lock().await;
                    let _ = db_lock.delete_event(event_id).await;
                    drop(db_lock);
                    load_events().await?;
                    
                    let updated_events = get_events_for_day(selected_day);
                    if *selected_event_index >= updated_events.len() && *selected_event_index > 0 {
                        *selected_event_index -= 1;
                    }
                }
            }
        },
        101 | 69 => { // 'e' or 'E' for Edit
            if let Some(event_id) = events[*selected_event_index].id {
                // Edit the selected event
                crate::edit_event::edit_event(db, event_id, load_events).await?;
            }
        },
        113 | 81 => {
            // 'q' or 'Q' to quit
            return Ok(());
        },
        _ => {}
    }
    
    Ok(())
}

pub async fn show_event_details(
    event_id: i32,
    db: &Arc<Mutex<Database>>,
    load_events: impl Fn() -> BoxFuture<'static, Result<(), DbError>>,
) -> Result<(), DbError> {
    let db_lock = db.lock().await;
    let event = db_lock.get_event(event_id).await?;
    drop(db_lock);
    
    // Create a panel to cover the entire screen (prevents text from showing through)
    let background = newwin(LINES(), COLS(), 0, 0);
    wbkgd(background, COLOR_PAIR(COLOR_DEFAULT));
    wrefresh(background);
    
    // Create dialog window
    let height = 18; // Increased height for action buttons
    let width = 70;
    let starty = (LINES() - height) / 2;
    let startx = (COLS() - width) / 2;
    
    let dialog = newwin(height, width, starty, startx);
    box_(dialog, 0, 0);
    wbkgd(dialog, COLOR_PAIR(COLOR_DIALOG));
    
    // Dialog title
    mvwprintw(dialog, 1, 2, "Event Details");
    mvwprintw(dialog, 3, 2, &format!("Date: {}", event.date));
    
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
    let title_lines = wrap_text(&event.title, title_max_width as usize);
    
    // Display title (potentially multi-line)
    mvwprintw(dialog, 4, 2, "Title: ");
    for (i, line) in title_lines.iter().enumerate() {
        mvwprintw(dialog, 4 + i as i32, 9, line);
    }
    
    // Adjust starting position for description based on title height
    let desc_start_y = 4 + title_lines.len() as i32 + 1;
    
    // Action buttons at the bottom
    mvwprintw(dialog, height - 3, 2, "[E]dit | [D]elete | Any other key: Close");
    
    let desc_area_height = height - desc_start_y - 5; // Leave room for instructions and buttons
    
    if let Some(desc) = &event.description {
        mvwprintw(dialog, desc_start_y, 2, "Description:");
        
        // Word wrap the description
        let desc_width_usize = (width - 8) as usize; // Leave padding for borders
        let mut wrapped_lines = Vec::new();
        
        // First split by explicit newlines
        for paragraph in desc.split('\n') {
            if paragraph.is_empty() {
                wrapped_lines.push(String::new());
            } else {
                // Then wrap each paragraph
                wrapped_lines.extend(wrap_text(paragraph, desc_width_usize));
            }
        }
        
        // Display lines with scrolling
        let visible_lines = desc_area_height as usize;
        let mut scroll_pos: usize = 0;
        let max_scroll = wrapped_lines.len().saturating_sub(visible_lines).max(0);
        let mut redraw = true;
        
        while redraw {
            if redraw {
                // Clear the description area
                for y in 0..desc_area_height {
                    for x in 0..width-6 {
                        mvwaddch(dialog, desc_start_y + 1 + y, 3 + x, ' ' as u32);
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
                    return crate::edit_event::edit_event(db, event_id, load_events).await;
                },
                100 | 68 => { // 'd' or 'D' for Delete
                    if crate::edit_event::confirm_delete_event() {
                        let db_lock = db.lock().await;
                        let _ = db_lock.delete_event(event_id).await;
                        drop(db_lock);
                        load_events().await?;
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
                return crate::edit_event::edit_event(db, event_id, load_events).await;
            },
            100 | 68 => { // 'd' or 'D' for Delete
                if crate::edit_event::confirm_delete_event() {
                    let db_lock = db.lock().await;
                    let _ = db_lock.delete_event(event_id).await;
                    drop(db_lock);
                    load_events().await?;
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
