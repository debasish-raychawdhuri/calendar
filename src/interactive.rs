use std::error::Error;

use crossterm::{
    event::{self, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame, Terminal,
};

use crate::calendar::Calendar;
use chrono::Datelike;

pub struct InteractiveCalendar {
    current_month: u8,
    current_year: u16,
}

impl InteractiveCalendar {
    pub fn new() -> Self {
        let now = chrono::Local::now();
        let date = now.date_naive();
        Self {
            current_month: date.month0() as u8,
            current_year: date.year() as u16,
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Set up the terminal for tui-rs
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the application
        let result = self.run_app(&mut terminal);

        // Restore the terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let event::Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Left => self.prev_month(),
                    KeyCode::Right => self.next_month(),
                    KeyCode::Up => self.prev_year(),
                    KeyCode::Down => self.next_year(),
                    _ => {}
                }
            }
        }
    }

    fn ui<B: Backend>(&self, f: &mut Frame<B>) {
        // Create layout with just the calendar area
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10), // For calendar grid
            ])
            .margin(2) // Add margin to shift everything to the right
            .split(f.size());

        // Split the calendar area into three parts for three months
        let calendar_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(chunks[0]);

        // Get calendar data for previous, current, and next months
        let prev_month = self.get_calendar_for_offset(-1);
        let current_month = self.get_calendar_for_offset(0);
        let next_month = self.get_calendar_for_offset(1);

        // Render the actual calendar content with correct month names
        self.render_month(
            f,
            &prev_month,
            calendar_chunks[0],
            self.get_month_name_for_offset(-1),
        );
        self.render_month(f, &current_month, calendar_chunks[1], self.get_month_name());
        self.render_month(
            f,
            &next_month,
            calendar_chunks[2],
            self.get_month_name_for_offset(1),
        );
    }

    fn render_month<B: Backend>(
        &self,
        f: &mut Frame<B>,
        calendar: &Calendar,
        area: tui::layout::Rect,
        month_name: String,
    ) {
        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let headers = days.iter().map(|d| {
            let style = if *d == "Sun" {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };
            Cell::from(*d).style(style)
        });

        let mut rows = Vec::new();

        for week in 0..6 {
            let mut cells = Vec::new();
            let start_day = calendar.calculate_line_start(week);

            for day_offset in 0..7 {
                let day = start_day + day_offset as i32;
                let today = Calendar::get_today();
                let is_today =
                    day == today.0 as i32 && calendar.month == today.1 && calendar.year == today.2;

                let cell = if day < 1 || day > calendar.get_total_days_in_month() as i32 {
                    Cell::from("    ")
                } else {
                    let day_str = format!("{:>2}", day);
                    let style = if is_today {
                        if day_offset == 0 {
                            Style::default().fg(Color::Black).bg(Color::Magenta)
                        } else {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        }
                    } else if day_offset == 0 {
                        Style::default().fg(Color::Magenta)
                    } else {
                        Style::default().fg(Color::Cyan)
                    };
                    Cell::from(day_str).style(style)
                };
                cells.push(cell);
            }

            let row = Row::new(cells);
            rows.push(row);
        }

        let title = format!("{} {}", month_name, calendar.year);

        // Create the table with all the calendar data
        let table_widget = Table::new(rows)
            .header(Row::new(headers).style(Style::default().add_modifier(Modifier::BOLD)))
            .block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Center),
            )
            .widths(&[
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .column_spacing(1);

        // Calculate the width of the table content
        let table_width = 3 * 7 + 1 * 6; // remove borders width

        // Create a centered layout to hold the table
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(table_width),
                Constraint::Percentage(50),
            ])
            .split(area);

        // Render the table in the centered area
        f.render_widget(table_widget, horizontal_chunks[1]);
    }

    fn get_month_name(&self) -> String {
        let month_names = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        month_names[self.current_month as usize].to_string()
    }

    fn get_month_name_for_offset(&self, offset: i8) -> String {
        let month_names = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let mut month = self.current_month as i16 + offset as i16;

        if month < 0 {
            month = 11;
        } else if month > 11 {
            month = 0;
        }

        month_names[month as usize].to_string()
    }

    fn get_calendar_for_offset(&self, offset: i8) -> Calendar {
        let mut month = self.current_month as i16 + offset as i16;
        let mut year = self.current_year as i16;

        if month < 0 {
            month = 11;
            year -= 1;
        } else if month > 11 {
            month = 0;
            year += 1;
        }

        Calendar {
            month: month as u8,
            year: year as u16,
        }
    }

    fn prev_month(&mut self) {
        if self.current_month == 0 {
            self.current_month = 11;
            self.current_year -= 1;
        } else {
            self.current_month -= 1;
        }
    }

    fn next_month(&mut self) {
        if self.current_month == 11 {
            self.current_month = 0;
            self.current_year += 1;
        } else {
            self.current_month += 1;
        }
    }

    fn prev_year(&mut self) {
        if self.current_year > 1583 {
            self.current_year -= 1;
        }
    }

    fn next_year(&mut self) {
        self.current_year += 1;
    }
}
