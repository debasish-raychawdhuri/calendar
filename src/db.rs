use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_postgres::{Client, Error as PgError, NoTls};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database connection error: {0}")]
    ConnectionError(#[from] PgError),
    
    #[error("Event not found")]
    EventNotFound,
    
    #[error("Invalid date format")]
    InvalidDate,
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
    client: Client,
}

impl Database {
    pub async fn connect(connection_string: &str) -> Result<Self, DbError> {
        let (client, connection) = tokio_postgres::connect(connection_string, NoTls).await?;
        
        // Spawn the connection task to run in the background
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Database connection error: {}", e);
            }
        });
        
        // Create tables if they don't exist
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS events (
                    id SERIAL PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    date DATE NOT NULL,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                )",
                &[],
            )
            .await?;
            
        Ok(Database { client })
    }
    
    pub async fn add_event(&self, event: &Event) -> Result<i32, DbError> {
        let row = self
            .client
            .query_one(
                "INSERT INTO events (title, description, date) VALUES ($1, $2, $3) RETURNING id",
                &[&event.title, &event.description, &event.date],
            )
            .await?;
            
        Ok(row.get(0))
    }
    
    pub async fn update_event(&self, event: &Event) -> Result<(), DbError> {
        let id = event.id.ok_or(DbError::EventNotFound)?;
        
        let rows_affected = self
            .client
            .execute(
                "UPDATE events SET title = $1, description = $2, date = $3 WHERE id = $4",
                &[&event.title, &event.description, &event.date, &id],
            )
            .await?;
            
        if rows_affected == 0 {
            return Err(DbError::EventNotFound);
        }
        
        Ok(())
    }
    
    pub async fn delete_event(&self, id: i32) -> Result<(), DbError> {
        let rows_affected = self
            .client
            .execute("DELETE FROM events WHERE id = $1", &[&id])
            .await?;
            
        if rows_affected == 0 {
            return Err(DbError::EventNotFound);
        }
        
        Ok(())
    }
    
    pub async fn get_event(&self, id: i32) -> Result<Event, DbError> {
        let row = self
            .client
            .query_opt("SELECT id, title, description, date, created_at FROM events WHERE id = $1", &[&id])
            .await?;
            
        if let Some(row) = row {
            Ok(Event {
                id: Some(row.get(0)),
                title: row.get(1),
                description: row.get(2),
                date: row.get(3),
                created_at: Some(row.get(4)),
            })
        } else {
            Err(DbError::EventNotFound)
        }
    }
    
    pub async fn get_events_for_date(&self, date: NaiveDate) -> Result<Vec<Event>, DbError> {
        let rows = self
            .client
            .query(
                "SELECT id, title, description, date, created_at FROM events WHERE date = $1",
                &[&date],
            )
            .await?;
            
        let events = rows
            .into_iter()
            .map(|row| Event {
                id: Some(row.get(0)),
                title: row.get(1),
                description: row.get(2),
                date: row.get(3),
                created_at: Some(row.get(4)),
            })
            .collect();
            
        Ok(events)
    }
    
    pub async fn get_events_for_month(&self, year: i32, month: i32) -> Result<Vec<Event>, DbError> {
        let rows = self
            .client
            .query(
                "SELECT id, title, description, date, created_at FROM events 
                 WHERE EXTRACT(YEAR FROM date) = $1 AND EXTRACT(MONTH FROM date) = $2",
                &[&year, &month],
            )
            .await?;
            
        let events = rows
            .into_iter()
            .map(|row| Event {
                id: Some(row.get(0)),
                title: row.get(1),
                description: row.get(2),
                date: row.get(3),
                created_at: Some(row.get(4)),
            })
            .collect();
            
        Ok(events)
    }
}
