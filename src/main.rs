mod calendar;
mod db;
mod ui;
mod edit_event;
mod google_calendar;
mod oauth_server;
mod ui_google;

use calendar::Calendar;
use chrono::{Datelike, Local};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Command line arguments for the calendar application
#[derive(Parser)]
#[command(name = "calendar")]
#[command(about = "Display a calendar for a given year/month")]
struct Args {
    /// Show calendar for the entire year
    #[arg(short = 'y', long = "year")]
    show_year: bool,

    /// Year or month to display (can be either a year > 12 or a month 1-12)
    #[arg(value_name = "YEAR_OR_MONTH")]
    first_arg: Option<String>,

    /// Month to display (1-12) when first argument is a year
    #[arg(value_name = "MONTH")]
    second_arg: Option<String>,

    /// Show only one month instead of the default three
    #[arg(short = 's', long = "single-month")]
    single_month: bool,
    
    /// Run in interactive mode with ncurses UI
    #[arg(short = 'i', long = "interactive", action = clap::ArgAction::SetTrue)]
    interactive: bool,
    
    /// Custom path for SQLite database file (optional)
    #[arg(short = 'd', long = "database")]
    db_path: Option<String>,
    
    /// Start Google Calendar authentication process
    #[arg(long = "google-auth", action = clap::ArgAction::SetTrue)]
    google_auth: bool,
}

/// Entry point of the calendar application
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Handle Google Calendar authentication if requested
    if args.google_auth {
        println!("Starting Google Calendar authentication process...");
        let db = Arc::new(Mutex::new(db::Database::connect(args.db_path.as_deref()).await?));
        return handle_google_auth(db).await;
    }
    
    if args.interactive {
        // Run in interactive mode with ncurses UI
        let db = Arc::new(Mutex::new(db::Database::connect(args.db_path.as_deref()).await?));
        let mut ui = ui::CalendarUI::new(db);
        
        ui.init().await?;
        let result = ui.run().await;
        ui.cleanup();
        
        return result.map_err(|e| e.into());
    }
    let now = Local::now();
    let date = now.date_naive();

    // Process the first argument to determine if it's a year or month
    let has_first_arg = args.first_arg.is_some();
    let (year, month) = if let Some(first_arg) = args.first_arg {
        match first_arg.parse::<u16>() {
            Ok(num) if num > 12 => (
                Some(num),
                args.second_arg.clone().and_then(|m| m.parse().ok()),
            ),
            Ok(num) => (None, Some(num as u8)),
            Err(_) => {
                eprintln!("Error: Invalid number format");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    let single = args.single_month;
    let year = year.unwrap_or(date.year() as u16);

    if year < 1583 {
        eprintln!("Error: Year must be 1583 or later");
        std::process::exit(1);
    }

    if args.show_year || (has_first_arg && args.second_arg.is_none() && year != date.year() as u16)
    {
        Calendar::print_entire_year(year);
    } else if let Some(month) = month {
        if !(1..=12).contains(&month) {
            eprintln!("Error: Month must be between 1 and 12");
            std::process::exit(1);
        }

        let cal = Calendar {
            year,
            month: month - 1,
        };
        if single {
            Calendar::print_single_month(cal);
        } else {
            cal.print();
        }
    } else {
        let cal = Calendar {
            year,
            month: now.month0() as u8,
        };
        if single {
            Calendar::print_single_month(cal);
        } else {
            cal.print();
        }
    }
    
    Ok(())
}
/// Handle Google Calendar authentication in a non-interactive way
async fn handle_google_auth(db: Arc<Mutex<db::Database>>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::google_calendar::{GoogleCalendarClient, GoogleCredentials};
    use tokio_util::sync::CancellationToken;
    use std::sync::Arc as StdArc;
    
    println!("=== Google Calendar Authentication ===");
    
    // Check for existing credentials
    if let Some(creds) = GoogleCredentials::load() {
        println!("Found existing credentials.");
        println!("Client ID: {}", creds.client_id);
        println!("Client Secret: {}", if creds.client_secret.is_empty() { "Not set" } else { "[Set]" });
        
        // Create Google client
        let mut client = GoogleCalendarClient::new(&creds.client_id, &creds.client_secret);
        
        if client.is_authenticated() {
            println!("Already authenticated. Do you want to re-authenticate? (y/n)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            
            if input.trim().to_lowercase() != "y" {
                println!("Authentication skipped. You're already authenticated.");
                return Ok(());
            }
        }
        
        println!("Starting authentication flow...");
        
        // Start the OAuth flow with detailed logging
        let (auth_url, _csrf_token, pkce_verifier) = client.start_auth_flow();
        
        println!("Please open this URL in your browser:");
        println!("{}", auth_url);
        println!("\nWaiting for authentication response...");
        
        // Start a local server to handle the OAuth callback
        let cancellation_token = CancellationToken::new();
        let code_receiver = StdArc::new(tokio::sync::Mutex::new(None));
        
        // Spawn the server in a separate task
        let server_token = cancellation_token.clone();
        let server_code_receiver = StdArc::clone(&code_receiver);
        
        let server_handle = tokio::spawn(async move {
            println!("Starting local server on http://localhost:8080");
            crate::oauth_server::start_oauth_server(server_token, server_code_receiver).await
        });
        
        // Wait for the code or timeout
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(300)); // 5 minutes
        tokio::pin!(timeout);
        
        let mut auth_code = None;
        
        loop {
            tokio::select! {
                _ = &mut timeout => {
                    println!("Authentication timed out after 5 minutes.");
                    cancellation_token.cancel();
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                    let code_guard = code_receiver.lock().await;
                    if let Some(code) = code_guard.as_ref() {
                        println!("Received authorization code: {}", code);
                        auth_code = Some(code.clone());
                        break;
                    }
                }
            }
        }
        
        // Cancel the server and wait for it to finish
        cancellation_token.cancel();
        println!("Stopping local server...");
        let _ = server_handle.await;
        
        // Complete the OAuth flow if we got a code
        if let Some(code) = auth_code {
            println!("Exchanging authorization code for access token...");
            
            match client.complete_auth_flow(&code, pkce_verifier).await {
                Ok(_) => {
                    println!("Authentication successful!");
                    println!("You can now use Google Calendar integration in the application.");
                },
                Err(e) => {
                    println!("Authentication failed: {}", e);
                    return Err(e.into());
                }
            }
        } else {
            println!("No authorization code received. Authentication failed.");
            return Err("Authentication failed: No authorization code received".into());
        }
    } else {
        println!("No Google Calendar credentials found.");
        println!("Please set up credentials first by running the application in interactive mode.");
        println!("Run: cargo run -- -i");
        println!("Then press 'G' to set up Google Calendar integration.");
    }
    
    Ok(())
}
