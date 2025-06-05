use chrono::{DateTime, NaiveDate, Utc};
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
    pub created_at: Option<DateTime<Utc>>,
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
        
        // Open or create the database file
        let conn = Connection::open(&db_path)
            .map_err(DbError::DatabaseError)?;
        
        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                date TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        ).map_err(DbError::DatabaseError)?;
        
        Ok(Database { conn })
    }
    
    pub async fn add_event(&self, event: &Event) -> Result<i32, DbError> {
        let now = Utc::now();
        let created_at = event.created_at.unwrap_or(now);
        
        self.conn.execute(
            "INSERT INTO events (title, description, date, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                event.title,
                event.description,
                event.date.to_string(),
                created_at.to_rfc3339()
            ],
        ).map_err(DbError::DatabaseError)?;
        
        let id = self.conn.last_insert_rowid() as i32;
        Ok(id)
    }
    
    pub async fn update_event(&self, event: &Event) -> Result<(), DbError> {
        let id = event.id.ok_or(DbError::EventNotFound)?;
        
        let rows_affected = self.conn.execute(
            "UPDATE events SET title = ?1, description = ?2, date = ?3 WHERE id = ?4",
            params![
                event.title,
                event.description,
                event.date.to_string(),
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
            "SELECT id, title, description, date, created_at FROM events WHERE id = ?1"
        ).map_err(DbError::DatabaseError)?;
        
        let event = stmt.query_row(params![id], |row| {
            let date_str: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid date format".to_string()))?;
            
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid datetime format".to_string()))?;
            
            Ok(Event {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                date,
                created_at: Some(created_at),
            })
        });
        
        match event {
            Ok(event) => Ok(event),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(DbError::EventNotFound),
            Err(e) => Err(DbError::DatabaseError(e)),
        }
    }
    
    pub async fn get_events_for_date(&self, date: NaiveDate) -> Result<Vec<Event>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, date, created_at FROM events WHERE date = ?1"
        ).map_err(DbError::DatabaseError)?;
        
        let date_str = date.to_string();
        let events_iter = stmt.query_map(params![date_str], |row| {
            let date_str: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid date format".to_string()))?;
            
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid datetime format".to_string()))?;
            
            Ok(Event {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                date,
                created_at: Some(created_at),
            })
        }).map_err(DbError::DatabaseError)?;
        
        let mut events = Vec::new();
        for event in events_iter {
            events.push(event.map_err(DbError::DatabaseError)?);
        }
        
        Ok(events)
    }
    
    pub async fn get_events_for_month(&self, year: i32, month: i32) -> Result<Vec<Event>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, date, created_at FROM events 
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
            
            Ok(Event {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                date,
                created_at: Some(created_at),
            })
        }).map_err(DbError::DatabaseError)?;
        
        let mut events = Vec::new();
        for event in events_iter {
            events.push(event.map_err(DbError::DatabaseError)?);
        }
        
        Ok(events)
    }
}
