use crate::db::{Database, DbError, Event};
use chrono::{DateTime, NaiveDate, Utc};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use reqwest::Client;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

const CREDENTIALS_FILE: &str = ".calendar_google_credentials.json";
const TOKEN_FILE: &str = ".calendar_google_token.json";

pub struct GoogleCalendarClient {
    oauth_client: BasicClient,
    http_client: Client,
    token: Option<oauth2::AccessToken>,
    refresh_token: Option<RefreshToken>,
}

impl GoogleCalendarClient {
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        let oauth_client = BasicClient::new(
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/auth".to_string()).unwrap(),
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
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

        GoogleCalendarClient {
            oauth_client,
            http_client,
            token,
            refresh_token,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }

    pub fn start_auth_flow(&self) -> (Url, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        
        let (auth_url, csrf_token) = self
            .oauth_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/calendar.readonly".to_string(),
            ))
            .set_pkce_challenge(pkce_challenge)
            // Add access_type=offline to get a refresh token that persists
            .add_extra_param("access_type", "offline")
            // Force approval prompt to ensure we always get a refresh token
            .add_extra_param("prompt", "consent")
            .url();

        println!("Open this URL in your browser: {}", auth_url);
        
        // Try to open the URL in the browser
        if let Err(e) = webbrowser::open(auth_url.as_str()) {
            println!("Failed to open URL automatically: {}", e);
            println!("Please open the URL manually in your browser.");
        }

        (auth_url, csrf_token, pkce_verifier)
    }

    pub async fn complete_auth_flow(
        &mut self,
        code: &str,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<(), String> {
        println!("Sending token request to Google with PKCE verifier...");
        
        let token_result = match self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await {
                Ok(token) => token,
                Err(e) => {
                    // Extract detailed error information
                    let error_details = match &e {
                        oauth2::RequestTokenError::ServerResponse(response) => {
                            format!(
                                "Server Error: {}, Description: {}",
                                response.error(),
                                response.error_description().map_or("No description", |s| s.as_str())
                            )
                        },
                        oauth2::RequestTokenError::Request(req_err) => {
                            format!("Request error: {}", req_err)
                        },
                        _ => format!("Other error: {:?}", e),
                    };
                    
                    return Err(format!("Failed to exchange code: {}", error_details));
                }
            };

        self.token = Some(token_result.access_token().clone());
        
        if let Some(refresh_token) = token_result.refresh_token() {
            self.refresh_token = Some(refresh_token.clone());
        }
        
        // Save token to file
        self.save_token()?;
        
        Ok(())
    }

    pub async fn refresh_access_token(&mut self) -> Result<(), String> {
        if let Some(refresh_token) = &self.refresh_token {
            println!("Refreshing access token...");
            
            let token_result = match self
                .oauth_client
                .exchange_refresh_token(refresh_token)
                .request_async(oauth2::reqwest::async_http_client)
                .await {
                    Ok(token) => token,
                    Err(e) => {
                        // Extract detailed error information
                        let error_details = match &e {
                            oauth2::RequestTokenError::ServerResponse(response) => {
                                format!(
                                    "Server Error: {}, Description: {}",
                                    response.error(),
                                    response.error_description().map_or("No description", |s| s.as_str())
                                )
                            },
                            oauth2::RequestTokenError::Request(req_err) => {
                                format!("Request error: {}", req_err)
                            },
                            _ => format!("Other error: {:?}", e),
                        };
                        
                        println!("Failed to refresh token: {}", error_details);
                        println!("You'll need to re-authenticate with Google Calendar.");
                        return Err(format!("Failed to refresh token: {}", error_details));
                    }
                };

            self.token = Some(token_result.access_token().clone());
            
            // Save the updated token
            self.save_token()?;
            
            Ok(())
        } else {
            Err("No refresh token available".to_string())
        }
    }

    fn get_token_path() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(TOKEN_FILE);
        path
    }

    fn load_token() -> Option<TokenData> {
        let path = Self::get_token_path();
        
        if !path.exists() {
            return None;
        }
        
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return None,
        };
        
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return None;
        }
        
        serde_json::from_str(&contents).ok()
    }

    fn save_token(&self) -> Result<(), String> {
        let token_data = TokenData {
            access_token: self.token.as_ref().map_or("".to_string(), |t| t.secret().clone()),
            refresh_token: self.refresh_token.as_ref().map(|t| t.secret().clone()),
            expiry: None, // We don't track expiry currently
        };
        
        let path = Self::get_token_path();
        let serialized = serde_json::to_string(&token_data)
            .map_err(|e| format!("Failed to serialize token: {}", e))?;
        
        let mut file = File::create(&path)
            .map_err(|e| format!("Failed to create token file: {}", e))?;
        
        file.write_all(serialized.as_bytes())
            .map_err(|e| format!("Failed to write token file: {}", e))?;
        
        Ok(())
    }

    pub async fn fetch_events(&mut self, start_date: NaiveDate, end_date: NaiveDate) 
        -> Result<Vec<Event>, String> {
        if self.token.is_none() {
            return Err("Not authenticated".to_string());
        }

        // Format dates for Google Calendar API
        let start_datetime = format!("{}T00:00:00Z", start_date);
        let end_datetime = format!("{}T23:59:59Z", end_date);
        
        // Build the URL with query parameters
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime",
            start_datetime, end_datetime
        );

        // Make the API request
        let response = match self.http_client
            .get(&url)
            .bearer_auth(self.token.as_ref().unwrap().secret())
            .send()
            .await {
                Ok(resp) => resp,
                Err(e) => {
                    // If we get an authentication error, try refreshing the token
                    if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
                        self.refresh_access_token().await?;
                        
                        // Retry with new token
                        self.http_client
                            .get(&url)
                            .bearer_auth(self.token.as_ref().unwrap().secret())
                            .send()
                            .await
                            .map_err(|e| format!("Failed to fetch events: {}", e))?
                    } else {
                        return Err(format!("Failed to fetch events: {}", e));
                    }
                }
            };

        // Get status before consuming the response
        let status = response.status();
        println!("Response status: {}", status);
            
        // Parse the response
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        // Check if there's an error in the response
        if response_body.get("error").is_some() {
            let error_msg = response_body["error"]["message"].as_str()
                .unwrap_or("Unknown error from Google Calendar API");
            return Err(format!("Google Calendar API error: {}", error_msg));
        }
        
        // Extract events - handle case where items might not exist
        let events = match response_body.get("items") {
            Some(items) => {
                match items.as_array() {
                    Some(array) => array,
                    None => {
                        println!("Items is not an array: {:?}", items);
                        return Err("Items field is not an array".to_string());
                    }
                }
            },
            None => {
                println!("Response doesn't contain items field: {:?}", response_body);
                // Return empty list instead of error if no events
                return Ok(Vec::new());
            }
        };

        // Convert Google Calendar events to our Event format
        let mut result = Vec::new();
        for event in events {
            // Skip events without a start date/time
            if !event.get("start").and_then(|s| s.as_object()).is_some() {
                println!("Skipping event without start object: {:?}", event);
                continue;
            }

            // Get the start date (either dateTime or date field)
            let start_date_str = if event.get("start").and_then(|s| s.get("dateTime")).and_then(|dt| dt.as_str()).is_some() {
                event["start"]["dateTime"].as_str().unwrap()
            } else if event.get("start").and_then(|s| s.get("date")).and_then(|d| d.as_str()).is_some() {
                event["start"]["date"].as_str().unwrap()
            } else {
                // Skip events with invalid start date format
                println!("Skipping event with invalid start date format: {:?}", event);
                continue;
            };

            // Parse the date
            let event_date = if start_date_str.contains('T') {
                // It's a datetime string
                match DateTime::parse_from_rfc3339(start_date_str) {
                    Ok(dt) => dt.naive_utc().date(),
                    Err(e) => {
                        println!("Failed to parse datetime '{}': {}", start_date_str, e);
                        continue;
                    }
                }
            } else {
                // It's a date string
                match NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d") {
                    Ok(date) => date,
                    Err(e) => {
                        println!("Failed to parse date '{}': {}", start_date_str, e);
                        continue;
                    }
                }
            };
            
            // Extract start time and duration
            let (start_time, duration_minutes) = if event.get("start").and_then(|s| s.get("dateTime")).is_some() && 
                                                   event.get("end").and_then(|e| e.get("dateTime")).is_some() {
                let start_datetime_str = event["start"]["dateTime"].as_str().unwrap();
                let end_datetime_str = event["end"]["dateTime"].as_str().unwrap();
                
                let start_datetime = match DateTime::parse_from_rfc3339(start_datetime_str) {
                    Ok(dt) => dt.with_timezone(&Utc),
                    Err(e) => {
                        println!("Failed to parse start datetime '{}': {}", start_datetime_str, e);
                        continue;
                    }
                };
                
                let end_datetime = match DateTime::parse_from_rfc3339(end_datetime_str) {
                    Ok(dt) => dt.with_timezone(&Utc),
                    Err(e) => {
                        println!("Failed to parse end datetime '{}': {}", end_datetime_str, e);
                        continue;
                    }
                };
                
                let duration = end_datetime.signed_duration_since(start_datetime);
                let duration_minutes = duration.num_minutes() as i32;
                
                // Store the time in UTC
                (Some(start_datetime.time()), Some(duration_minutes))
            } else {
                // All-day event
                (None, None)
            };

            // Create our Event object
            let calendar_event = Event {
                id: None, // This will be assigned when saved to the database
                title: event.get("summary")
                    .and_then(|s| s.as_str())
                    .unwrap_or("Untitled Event")
                    .to_string(),
                description: event.get("description").and_then(|s| s.as_str()).map(|s| s.to_string()),
                date: event_date,
                start_time,
                duration_minutes,
                created_at: None,
                google_id: event.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()), // Store Google's event ID
            };

            result.push(calendar_event);
        }

        Ok(result)
    }

    pub async fn import_events_to_db(
        &mut self,
        db: &Arc<Mutex<Database>>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<usize, String> {
        // Fetch events from Google Calendar
        let events = self.fetch_events(start_date, end_date).await?;
        
        // Save events to the database
        let db_lock = db.lock().await;
        let mut count = 0;
        let mut imported_google_ids = Vec::new();
        
        for event in events {
            // Skip events without Google ID (shouldn't happen, but just in case)
            let google_id = match &event.google_id {
                Some(id) => {
                    imported_google_ids.push(id.clone());
                    id
                },
                None => continue,
            };
            
            // Check if this event already exists in our database
            match db_lock.find_event_by_google_id(google_id).await {
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
        
        // Delete events that were previously imported from Google but are no longer present
        match db_lock.delete_missing_google_events(&imported_google_ids).await {
            Ok(deleted) => {
                println!("Removed {} events that were deleted from Google Calendar", deleted);
            },
            Err(e) => eprintln!("Failed to clean up deleted events: {:?}", e),
        }
        
        Ok(count)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TokenData {
    access_token: String,
    refresh_token: Option<String>,
    expiry: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl GoogleCredentials {
    pub fn load() -> Option<Self> {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(CREDENTIALS_FILE);
        
        if !path.exists() {
            return None;
        }
        
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return None,
        };
        
        serde_json::from_str(&contents).ok()
    }
    
    pub fn save(&self) -> Result<(), String> {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(CREDENTIALS_FILE);
        
        let serialized = serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize credentials: {}", e))?;
        
        fs::write(&path, serialized)
            .map_err(|e| format!("Failed to write credentials file: {}", e))?;
        
        Ok(())
    }
}

// Function to handle the OAuth callback
pub async fn handle_oauth_callback(
    code: &str,
    client: &mut GoogleCalendarClient,
    pkce_verifier: PkceCodeVerifier,
) -> Result<(), String> {
    client.complete_auth_flow(code, pkce_verifier).await
}
