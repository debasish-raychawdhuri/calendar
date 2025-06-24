//! Outlook Calendar integration module
//! 
//! This module provides functionality to authenticate with Outlook Calendar
//! and import events into the local database.

use crate::db::{Database, Event};
use chrono::{NaiveDate, Utc};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

const CREDENTIALS_FILE: &str = ".calendar_outlook_credentials.json";
const TOKEN_FILE: &str = ".calendar_outlook_token.json";

/// Client for interacting with Outlook Calendar API
pub struct OutlookCalendarClient {
    oauth_client: BasicClient,
    http_client: Client,
    token: Option<oauth2::AccessToken>,
    refresh_token: Option<RefreshToken>,
}

#[derive(Serialize, Deserialize)]
struct TokenData {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

impl OutlookCalendarClient {
    /// Create a new Outlook Calendar client
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        let oauth_client = BasicClient::new(
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
            // Microsoft OAuth endpoints
            AuthUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new("http://localhost:8080".to_string()).unwrap());

        let http_client = Client::new();
        
        // Try to load existing token
        let mut token = None;
        let mut refresh_token = None;
        
        if let Some(saved_token) = Self::load_token() {
            token = Some(oauth2::AccessToken::new(saved_token.access_token));
            if let Some(refresh) = saved_token.refresh_token {
                refresh_token = Some(RefreshToken::new(refresh));
            }
        }

        OutlookCalendarClient {
            oauth_client,
            http_client,
            token,
            refresh_token,
        }
    }
    
    /// Check if the client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
    
    /// Start the OAuth flow
    pub fn start_auth_flow(&self) -> (Url, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        
        let (auth_url, csrf_token) = self
            .oauth_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("https://graph.microsoft.com/Calendars.Read".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();
        
        (auth_url, csrf_token, pkce_verifier)
    }
    
    /// Complete the OAuth flow with the authorization code
    pub async fn complete_auth_flow(
        &mut self,
        code: &str,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<(), String> {
        let token_result = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| format!("Failed to exchange code: {}", e))?;
        
        self.token = Some(token_result.access_token().clone());
        
        if let Some(refresh_token) = token_result.refresh_token() {
            self.refresh_token = Some(refresh_token.clone());
        }
        
        self.save_token()?;
        
        Ok(())
    }
    
    /// Refresh the access token
    pub async fn refresh_token(&mut self) -> Result<(), String> {
        if let Some(refresh_token) = &self.refresh_token {
            let token_result = self
                .oauth_client
                .exchange_refresh_token(refresh_token)
                .request_async(oauth2::reqwest::async_http_client)
                .await
                .map_err(|e| format!("Failed to refresh token: {}", e))?;
            
            self.token = Some(token_result.access_token().clone());
            
            if let Some(new_refresh_token) = token_result.refresh_token() {
                self.refresh_token = Some(new_refresh_token.clone());
            }
            
            self.save_token()?;
            
            Ok(())
        } else {
            Err("No refresh token available".to_string())
        }
    }
    
    /// Load token from file
    fn load_token() -> Option<TokenData> {
        let home_dir = dirs::home_dir()?;
        let token_path = home_dir.join(TOKEN_FILE);
        
        if !token_path.exists() {
            return None;
        }
        
        let mut file = File::open(token_path).ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;
        
        serde_json::from_str(&contents).ok()
    }
    
    /// Save token to file
    fn save_token(&self) -> Result<(), String> {
        if self.token.is_none() {
            return Err("No token to save".to_string());
        }
        
        let token = TokenData {
            access_token: self.token.as_ref().unwrap().secret().to_string(),
            refresh_token: self.refresh_token.as_ref().map(|t| t.secret().to_string()),
            expires_in: None,
        };
        
        let home_dir = dirs::home_dir().ok_or_else(|| "Could not find home directory".to_string())?;
        let token_path = home_dir.join(TOKEN_FILE);
        
        let json = serde_json::to_string_pretty(&token)
            .map_err(|e| format!("Failed to serialize token: {}", e))?;
        
        let mut file = File::create(token_path)
            .map_err(|e| format!("Failed to create token file: {}", e))?;
        
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write token: {}", e))?;
        
        Ok(())
    }
}

/// Credentials for Outlook Calendar API
#[derive(Serialize, Deserialize)]
pub struct OutlookCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl OutlookCredentials {
    /// Load credentials from file
    pub fn load() -> Option<Self> {
        let home_dir = dirs::home_dir()?;
        let creds_path = home_dir.join(CREDENTIALS_FILE);
        
        if !creds_path.exists() {
            return None;
        }
        
        let mut file = File::open(creds_path).ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;
        
        serde_json::from_str(&contents).ok()
    }
    
    /// Save credentials to file
    pub fn save(&self) -> Result<(), String> {
        let home_dir = dirs::home_dir().ok_or_else(|| "Could not find home directory".to_string())?;
        let creds_path = home_dir.join(CREDENTIALS_FILE);
        
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize credentials: {}", e))?;
        
        let mut file = File::create(creds_path)
            .map_err(|e| format!("Failed to create credentials file: {}", e))?;
        
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write credentials: {}", e))?;
        
        Ok(())
    }
}

/// Token response from Outlook API
#[derive(Serialize, Deserialize)]
struct OutlookToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

impl OutlookCalendarClient {
    /// Fetch events from Outlook Calendar
    pub async fn fetch_events(&mut self, start_date: NaiveDate, end_date: NaiveDate) -> Result<Vec<Event>, String> {
        if self.token.is_none() {
            return Err("Not authenticated".to_string());
        }
        
        // Format dates for Outlook API
        let start_str = start_date.format("%Y-%m-%dT00:00:00Z").to_string();
        let end_str = end_date.format("%Y-%m-%dT23:59:59Z").to_string();
        
        // Build the request URL
        let url = format!(
            "https://graph.microsoft.com/v1.0/me/calendarView?startDateTime={}&endDateTime={}",
            start_str, end_str
        );
        
        // Make the request
        let response = match self.http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token.as_ref().unwrap().secret()))
            .send()
            .await {
                Ok(resp) => resp,
                Err(e) => {
                    // If we get an authentication error, try refreshing the token
                    if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
                        self.refresh_token().await?;
                        
                        // Retry with new token
                        self.http_client
                            .get(&url)
                            .header("Authorization", format!("Bearer {}", self.token.as_ref().unwrap().secret()))
                            .send()
                            .await
                            .map_err(|e| format!("Failed to fetch events: {}", e))?
                    } else {
                        return Err(format!("Failed to fetch events: {}", e));
                    }
                }
            };
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API error: {} - {}", status, error_text));
        }
        
        // Parse the response
        let response_text = response.text().await
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        // Extract events
        let events = response_json["value"].as_array()
            .ok_or_else(|| "Invalid response format".to_string())?;
        
        let mut result = Vec::new();
        
        for event_json in events {
            // Extract event details
            let id = event_json["id"].as_str()
                .ok_or_else(|| "Event missing ID".to_string())?;
            
            let subject = event_json["subject"].as_str()
                .unwrap_or("(No title)");
            
            let body = event_json["bodyPreview"].as_str();
            
            // Parse start and end times
            let start_time_str = event_json["start"]["dateTime"].as_str()
                .ok_or_else(|| "Event missing start time".to_string())?;
            
            let end_time_str = event_json["end"]["dateTime"].as_str()
                .ok_or_else(|| "Event missing end time".to_string())?;
            
            // Parse the date/time strings
            let start_datetime = chrono::DateTime::parse_from_rfc3339(start_time_str)
                .map_err(|e| format!("Invalid start time format: {}", e))?;
            
            let end_datetime = chrono::DateTime::parse_from_rfc3339(end_time_str)
                .map_err(|e| format!("Invalid end time format: {}", e))?;
            
            // Calculate duration in minutes
            let duration_minutes = (end_datetime - start_datetime).num_minutes() as i32;
            
            // Create the event
            let event = Event {
                id: None,
                title: subject.to_string(),
                description: body.map(|s| s.to_string()),
                date: start_datetime.date_naive(),
                start_time: Some(start_datetime.time()),
                duration_minutes: Some(duration_minutes),
                created_at: Some(Utc::now()),
                google_id: None,
                outlook_id: Some(id.to_string()),
                source: Some("outlook".to_string()),
            };
            
            result.push(event);
        }
        
        Ok(result)
    }

    /// Import events from Outlook Calendar to local database
    pub async fn import_events_to_db(
        &mut self,
        db: &Arc<Mutex<Database>>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<usize, String> {
        // Fetch events from Outlook Calendar
        let events = self.fetch_events(start_date, end_date).await?;
        
        // Save events to the database
        let db_lock = db.lock().await;
        let mut count = 0;
        let mut imported_outlook_ids = Vec::new();
        
        for event in events {
            // Skip events without Outlook ID (shouldn't happen, but just in case)
            let outlook_id = match &event.outlook_id {
                Some(id) => {
                    imported_outlook_ids.push(id.clone());
                    id
                },
                None => continue,
            };
            
            // Check if this event already exists in our database
            match db_lock.find_event_by_outlook_id(outlook_id).await {
                Ok(Some(existing_event)) => {
                    // Update existing event
                    let mut updated_event = event.clone();
                    updated_event.id = existing_event.id;
                    
                    match db_lock.update_event(&updated_event).await {
                        Ok(_) => count += 1,
                        Err(e) => eprintln!("Failed to update event: {:?}", e),
                    }
                },
                Ok(None) => {
                    // Add new event
                    match db_lock.add_event(&event).await {
                        Ok(_) => count += 1,
                        Err(e) => eprintln!("Failed to add event: {:?}", e),
                    }
                },
                Err(e) => eprintln!("Error checking for existing event: {:?}", e),
            }
        }
        
        // Delete events that were previously imported from Outlook but are no longer present
        match db_lock.delete_missing_outlook_events(&imported_outlook_ids).await {
            Ok(deleted) => {
                println!("Removed {} events that were deleted from Outlook Calendar", deleted);
            },
            Err(e) => eprintln!("Failed to clean up deleted events: {:?}", e),
        }
        
        Ok(count)
    }
}
