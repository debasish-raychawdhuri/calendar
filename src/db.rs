use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use directories::ProjectDirs;
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    
    #[error("Event not found")]
    EventNotFound,
    
    #[error("Invalid date format")]
    InvalidDate,
    
    #[error("Failed to create database directory: {0}")]
    DirectoryCreationError(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Event {
    pub id: Option<i32>,
    pub title: String,
    pub description: Option<String>,
    pub date: NaiveDate,
    pub start_time: Option<NaiveTime>,  // Start time of the event
    pub duration_minutes: Option<i32>,  // Duration in minutes
    pub created_at: Option<DateTime<Utc>>,
    pub google_id: Option<String>,      // Google Calendar event ID for deduplication
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub async fn connect(db_path: Option<&str>) -> Result<Self, DbError> {
        let db_path = match db_path {
            Some(path) => PathBuf::from(path),
            None => {
                // Get the default data directory for the application
                let proj_dirs = ProjectDirs::from("com", "calendar", "calendar-app")
                    .ok_or_else(|| DbError::DirectoryCreationError("Failed to determine project directory".to_string()))?;
                
                let data_dir = proj_dirs.data_dir();
                fs::create_dir_all(data_dir)
                    .map_err(|e| DbError::DirectoryCreationError(e.to_string()))?;
                
                data_dir.join("calendar.db")
            }
        };
        
        let conn = Connection::open(&db_path)
            .map_err(DbError::DatabaseError)?;
        
        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                date TEXT NOT NULL,
                start_time TEXT,
                duration_minutes INTEGER,
                created_at TEXT NOT NULL,
                google_id TEXT
            )",
            [],
        ).map_err(DbError::DatabaseError)?;
        
        Ok(Database { conn })
    }
    
    pub async fn migrate_database(&self) -> Result<(), DbError> {
        println!("Running database migrations...");
        
        // Check if google_id column exists
        let columns = self.conn.prepare("PRAGMA table_info(events)")
            .map_err(DbError::DatabaseError)?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })
            .map_err(DbError::DatabaseError)?
            .collect::<Result<Vec<String>, _>>()
            .map_err(DbError::DatabaseError)?;
        
        // Add google_id column if it doesn't exist
        if !columns.contains(&"google_id".to_string()) {
            println!("Adding google_id column to events table");
            self.conn.execute(
                "ALTER TABLE events ADD COLUMN google_id TEXT;",
                [],
            ).map_err(DbError::DatabaseError)?;
        } else {
            println!("google_id column already exists");
        }
        
        println!("Migrations completed successfully.");
        Ok(())
    }
    
    // Delete all events that were imported from Google Calendar
    pub async fn delete_all_google_events(&self) -> Result<usize, DbError> {
        let query = "DELETE FROM events WHERE google_id IS NOT NULL";
        
        let rows_affected = self.conn.execute(query, [])
            .map_err(DbError::DatabaseError)?;
        
        Ok(rows_affected)
    }
    
    pub async fn add_event(&self, event: &Event) -> Result<i32, DbError> {
        let now = Utc::now();
        let created_at = event.created_at.unwrap_or(now);
        
        // Store time in UTC format
        self.conn.execute(
            "INSERT INTO events (title, description, date, start_time, duration_minutes, created_at, google_id) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.title,
                event.description,
                event.date.to_string(),
                event.start_time.map(|t| t.format("%H:%M:%S").to_string()),
                event.duration_minutes,
                created_at.to_rfc3339(),
                event.google_id
            ],
        ).map_err(DbError::DatabaseError)?;
        
        let id = self.conn.last_insert_rowid() as i32;
        Ok(id)
    }
    
    pub async fn update_event(&self, event: &Event) -> Result<(), DbError> {
        let id = event.id.ok_or(DbError::EventNotFound)?;
        
        let rows_affected = self.conn.execute(
            "UPDATE events SET title = ?1, description = ?2, date = ?3, start_time = ?4, duration_minutes = ?5, google_id = ?6 WHERE id = ?7",
            params![
                event.title,
                event.description,
                event.date.to_string(),
                event.start_time.map(|t| t.format("%H:%M:%S").to_string()),
                event.duration_minutes,
                event.google_id,
                id
            ],
        ).map_err(DbError::DatabaseError)?;
        
        if rows_affected == 0 {
            return Err(DbError::EventNotFound);
        }
        
        Ok(())
    }
    
    pub async fn delete_event(&self, id: i32) -> Result<(), DbError> {
        let rows_affected = self.conn.execute(
            "DELETE FROM events WHERE id = ?1",
            params![id],
        ).map_err(DbError::DatabaseError)?;
        
        if rows_affected == 0 {
            return Err(DbError::EventNotFound);
        }
        
        Ok(())
    }
    
    pub async fn get_event(&self, id: i32) -> Result<Event, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, date, created_at, start_time, duration_minutes, google_id FROM events WHERE id = ?1"
        ).map_err(DbError::DatabaseError)?;
        
        let event = stmt.query_row(params![id], |row| {
            let date_str: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid date format".to_string()))?;
            
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid datetime format".to_string()))?;
            
            let start_time_str: Option<String> = row.get(5)?;
            let start_time = start_time_str.and_then(|s| NaiveTime::parse_from_str(&s, "%H:%M:%S").ok());
            
            let duration_minutes: Option<i32> = row.get(6)?;
            let google_id: Option<String> = row.get(7)?;
            
            Ok(Event {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                date,
                start_time,
                duration_minutes,
                created_at: Some(created_at),
                google_id,
            })
        });
        
        match event {
            Ok(event) => Ok(event),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(DbError::EventNotFound),
            Err(e) => Err(DbError::DatabaseError(e)),
        }
    }
    
    pub async fn get_events_for_month(&self, year: i32, month: i32) -> Result<Vec<Event>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, date, created_at, start_time, duration_minutes, google_id FROM events 
             WHERE strftime('%Y', date) = ?1 AND strftime('%m', date) = ?2"
        ).map_err(DbError::DatabaseError)?;
        
        let year_str = year.to_string();
        let month_str = format!("{:02}", month);
        
        let events_iter = stmt.query_map(params![year_str, month_str], |row| {
            let date_str: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid date format".to_string()))?;
            
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid datetime format".to_string()))?;
            
            let start_time_str: Option<String> = row.get(5)?;
            let start_time = start_time_str.and_then(|s| NaiveTime::parse_from_str(&s, "%H:%M:%S").ok());
            
            let duration_minutes: Option<i32> = row.get(6)?;
            let google_id: Option<String> = row.get(7)?;
            
            Ok(Event {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                date,
                start_time,
                duration_minutes,
                created_at: Some(created_at),
                google_id,
            })
        }).map_err(DbError::DatabaseError)?;
        
        let mut events = Vec::new();
        for event in events_iter {
            events.push(event.map_err(DbError::DatabaseError)?);
        }
        
        Ok(events)
    }
    
    // Find an event by Google ID
    pub async fn find_event_by_google_id(&self, google_id: &str) -> Result<Option<Event>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, date, created_at, start_time, duration_minutes, google_id FROM events WHERE google_id = ?1"
        ).map_err(DbError::DatabaseError)?;
        
        let event_result = stmt.query_row(params![google_id], |row| {
            let date_str: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid date format".to_string()))?;
            
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid datetime format".to_string()))?;
            
            let start_time_str: Option<String> = row.get(5)?;
            let start_time = start_time_str.and_then(|s| NaiveTime::parse_from_str(&s, "%H:%M:%S").ok());
            
            let duration_minutes: Option<i32> = row.get(6)?;
            let google_id: Option<String> = row.get(7)?;
            
            Ok(Event {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                date,
                start_time,
                duration_minutes,
                created_at: Some(created_at),
                google_id,
            })
        });
        
        match event_result {
            Ok(event) => Ok(Some(event)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::DatabaseError(e)),
        }
    }
    
    // Delete all events with Google IDs that are not in the provided list
    pub async fn delete_missing_google_events(&self, google_ids: &[String]) -> Result<usize, DbError> {
        let placeholders = google_ids.iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for id in google_ids {
            params.push(id);
        }
        
        let query = if !google_ids.is_empty() {
            format!("DELETE FROM events WHERE google_id IS NOT NULL AND google_id NOT IN ({})", placeholders)
        } else {
            "DELETE FROM events WHERE google_id IS NOT NULL".to_string()
        };
        
        let rows_affected = self.conn.execute(&query, rusqlite::params_from_iter(params))
            .map_err(DbError::DatabaseError)?;
        
        Ok(rows_affected)
    }
}
