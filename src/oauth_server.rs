use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;

// Simple HTTP server to handle OAuth callback
pub async fn start_oauth_server(
    cancellation_token: CancellationToken,
    code_receiver: Arc<Mutex<Option<String>>>,
) -> Result<(), String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to address: {}", e))?;

    println!("Listening for OAuth callback on http://localhost:8080");

    loop {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                return Ok(());
            }
            result = listener.accept() => {
                match result {
                    Ok((mut stream, _)) => {
                        let code_receiver = Arc::clone(&code_receiver);
                        
                        tokio::spawn(async move {
                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                            
                            let mut buffer = [0; 1024];
                            if let Ok(n) = stream.read(&mut buffer).await {
                                let request = String::from_utf8_lossy(&buffer[..n]);
                                println!("Received callback request: {}", request.lines().next().unwrap_or(""));
                                
                                // Extract the authorization code from the request
                                if let Some(code) = extract_code_from_request(&request) {
                                    // Store the code
                                    let mut code_guard = code_receiver.lock().await;
                                    *code_guard = Some(code);
                                    
                                    // Send success response
                                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                        <html><body><h1>Authentication Successful</h1>\
                                        <p>You can now close this window and return to the application.</p>\
                                        </body></html>";
                                    
                                    let _ = stream.write_all(response.as_bytes()).await;
                                } else {
                                    // Send error response
                                    let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                                        <html><body><h1>Authentication Failed</h1>\
                                        <p>No authorization code found in the request.</p>\
                                        </body></html>";
                                    
                                    let _ = stream.write_all(response.as_bytes()).await;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        }
    }
}

fn extract_code_from_request(request: &str) -> Option<String> {
    // Extract the query string from the request
    let request_line = request.lines().next()?;
    println!("Request line: {}", request_line);
    
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    
    let path = parts[1];
    if !path.starts_with('?') && !path.contains('?') {
        return None;
    }
    
    // Parse the URL and extract the code parameter
    let url = Url::parse(&format!("http://localhost{}", path)).ok()?;
    let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    
    let code = params.get("code").cloned();
    println!("Extracted code: {:?}", code);
    
    code
}
