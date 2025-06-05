use crate::calendar::Calendar;
use crate::db::DbError;
use crate::google_calendar::{GoogleCalendarClient, GoogleCredentials};
use crate::oauth_server;
use chrono::{NaiveDate, Utc};
use ncurses::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// Google Calendar integration methods for CalendarUI
pub async fn handle_google_calendar(
    google_client: &mut Option<GoogleCalendarClient>,
    db: &Arc<Mutex<crate::db::Database>>,
    current_year: u16,
    current_month: u8,
) -> Result<(), DbError> {
    // Temporarily exit ncurses mode to interact with the terminal
    def_prog_mode();
    endwin();
    
    println!("\n=== Google Calendar Integration ===\n");
    
    if google_client.is_none() {
        println!("Google Calendar is not configured.");
        println!("Would you like to set it up now? (y/n)");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
        
        if input.trim().to_lowercase() == "y" {
            setup_google_calendar(google_client).await?;
        }
    } else if !google_client.as_ref().unwrap().is_authenticated() {
        println!("Google Calendar is configured but not authenticated.");
        println!("Would you like to authenticate now? (y/n)");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
        
        if input.trim().to_lowercase() == "y" {
            authenticate_google_calendar(google_client).await?;
        }
    } else {
        println!("Google Calendar is configured and authenticated.");
        println!("What would you like to do?");
        println!("1. Import events for current month");
        println!("2. Import events for a specific date range");
        println!("3. Log out");
        println!("4. Return to calendar");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
        
        match input.trim() {
            "1" => {
                // Import events for current month
                let start_date = NaiveDate::from_ymd_opt(
                    current_year as i32,
                    current_month as u32 + 1,
                    1,
                ).unwrap_or_else(|| Utc::now().naive_utc().date());
                
                let days_in_month = Calendar {
                    year: current_year,
                    month: current_month,
                }.get_total_days_in_month();
                
                let end_date = NaiveDate::from_ymd_opt(
                    current_year as i32,
                    current_month as u32 + 1,
                    days_in_month,
                ).unwrap_or_else(|| start_date);
                
                println!("Importing events from {} to {}...", start_date, end_date);
                
                let count = google_client.as_mut().unwrap()
                    .import_events_to_db(db, start_date, end_date)
                    .await
                    .map_err(|e| DbError::Other(e))?;
                
                println!("Successfully imported {} events.", count);
                println!("Press Enter to continue...");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap_or_default();
                
                // Reload events to show the imported ones
                // We'll handle this manually in the UI
            },
            "2" => {
                // Import events for a specific date range
                println!("Enter start date (YYYY-MM-DD):");
                let mut start_input = String::new();
                std::io::stdin().read_line(&mut start_input).unwrap_or_default();
                
                println!("Enter end date (YYYY-MM-DD):");
                let mut end_input = String::new();
                std::io::stdin().read_line(&mut end_input).unwrap_or_default();
                
                // Parse dates
                let start_date = match NaiveDate::parse_from_str(start_input.trim(), "%Y-%m-%d") {
                    Ok(date) => date,
                    Err(_) => {
                        println!("Invalid start date format. Using today's date.");
                        Utc::now().naive_utc().date()
                    }
                };
                
                let end_date = match NaiveDate::parse_from_str(end_input.trim(), "%Y-%m-%d") {
                    Ok(date) => date,
                    Err(_) => {
                        println!("Invalid end date format. Using start date + 30 days.");
                        start_date + chrono::Duration::days(30)
                    }
                };
                
                println!("Importing events from {} to {}...", start_date, end_date);
                
                let count = google_client.as_mut().unwrap()
                    .import_events_to_db(db, start_date, end_date)
                    .await
                    .map_err(|e| DbError::Other(e))?;
                
                println!("Successfully imported {} events.", count);
                println!("Press Enter to continue...");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap_or_default();
                
                // Reload events to show the imported ones
                // We'll handle this manually in the UI
            },
            "3" => {
                // Log out (remove token file)
                let token_path = std::path::PathBuf::from(dirs::home_dir().unwrap_or_default())
                    .join(".calendar_google_token.json");
                
                if token_path.exists() {
                    std::fs::remove_file(token_path).unwrap_or_default();
                    println!("Logged out successfully.");
                }
                
                // Reset the client
                if let Some(creds) = GoogleCredentials::load() {
                    *google_client = Some(GoogleCalendarClient::new(&creds.client_id, &creds.client_secret));
                }
                
                println!("Press Enter to continue...");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap_or_default();
            },
            _ => {}
        }
    }
    
    // Return to ncurses mode
    reset_prog_mode();
    refresh();
    
    Ok(())
}

async fn setup_google_calendar(google_client: &mut Option<GoogleCalendarClient>) -> Result<(), DbError> {
    println!("\nTo set up Google Calendar integration, you need to create OAuth credentials in Google Cloud Console.");
    println!("Follow these steps:");
    println!("1. Go to https://console.cloud.google.com/");
    println!("2. Create a new project");
    println!("3. Enable the Google Calendar API");
    println!("4. Create OAuth 2.0 credentials (Web application type)");
    println!("5. Add http://localhost:8080 as an authorized redirect URI");
    println!("6. Copy the Client ID and Client Secret");
    println!("\nEnter your Client ID:");
    
    let mut client_id = String::new();
    std::io::stdin().read_line(&mut client_id).unwrap_or_default();
    
    println!("Enter your Client Secret:");
    let mut client_secret = String::new();
    std::io::stdin().read_line(&mut client_secret).unwrap_or_default();
    
    // Save credentials
    let credentials = GoogleCredentials {
        client_id: client_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
    };
    
    if let Err(e) = credentials.save() {
        println!("Failed to save credentials: {}", e);
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
        return Ok(());
    }
    
    // Create Google client
    *google_client = Some(GoogleCalendarClient::new(
        &credentials.client_id,
        &credentials.client_secret,
    ));
    
    println!("\nCredentials saved successfully!");
    println!("Would you like to authenticate now? (y/n)");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or_default();
    
    if input.trim().to_lowercase() == "y" {
        authenticate_google_calendar(google_client).await?;
    } else {
        println!("You can authenticate later by pressing 'G' in the calendar view.");
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
    }
    
    Ok(())
}

async fn authenticate_google_calendar(google_client: &mut Option<GoogleCalendarClient>) -> Result<(), DbError> {
    if google_client.is_none() {
        println!("Google Calendar is not configured.");
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
        return Ok(());
    }
    
    let google_client_ref = google_client.as_ref().unwrap();
    
    // Start the OAuth flow
    let (auth_url, _csrf_token, pkce_challenge) = google_client_ref.start_auth_flow();
    
    println!("\nA browser window should open automatically.");
    println!("If it doesn't, please open this URL manually:");
    println!("{}", auth_url);
    println!("\nWaiting for authentication...");
    
    // Start a local server to handle the OAuth callback
    let cancellation_token = CancellationToken::new();
    let code_receiver = Arc::new(Mutex::new(None));
    
    // Spawn the server in a separate task
    let server_token = cancellation_token.clone();
    let server_code_receiver = Arc::clone(&code_receiver);
    
    let server_handle = tokio::spawn(async move {
        oauth_server::start_oauth_server(server_token, server_code_receiver).await
    });
    
    // Wait for the code or timeout
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(300)); // 5 minutes
    tokio::pin!(timeout);
    
    let mut auth_code = None;
    
    loop {
        tokio::select! {
            _ = &mut timeout => {
                println!("Authentication timed out.");
                cancellation_token.cancel();
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                let code_guard = code_receiver.lock().await;
                if let Some(code) = code_guard.as_ref() {
                    auth_code = Some(code.clone());
                    break;
                }
            }
        }
    }
    
    // Cancel the server and wait for it to finish
    cancellation_token.cancel();
    let _ = server_handle.await;
    
    // Complete the OAuth flow if we got a code
    if let Some(code) = auth_code {
        println!("Received authorization code. Completing authentication...");
        
        // Get a mutable reference to the client
        let google_client_mut = google_client.as_mut().unwrap();
        
        match google_client_mut.complete_auth_flow(&code, pkce_challenge).await {
            Ok(_) => {
                println!("Authentication successful!");
            },
            Err(e) => {
                println!("Authentication failed: {}", e);
            }
        }
    }
    
    println!("Press Enter to continue...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or_default();
    
    Ok(())
}
