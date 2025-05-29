use postgres::{Client, NoTls, Error};
use chrono::NaiveDate;

const DB_URL: &str = "postgresql://cal_user:cal_pass@localhost:5432/calendar_db";

pub fn connect_db() -> Result<Client, Error> {
    let mut client = Client::connect(DB_URL, NoTls)?;

    client.batch_execute("
        CREATE TABLE IF NOT EXISTS events (
            id SERIAL PRIMARY KEY,
            event_date DATE NOT NULL,
            event_description TEXT NOT NULL
        )
    ")?;

    Ok(client)
}

pub fn save_event(client: &mut Client, date: NaiveDate, description: &str) -> Result<(), Error> {
    client.execute(
        "INSERT INTO events (event_date, event_description) VALUES ($1, $2)",
        &[&date, &description],
    )?;
    Ok(())
}

pub fn get_events_for_month(client: &mut Client, year: i32, month: u32) -> Result<Vec<NaiveDate>, Error> {
    let rows = client.query(
        "SELECT DISTINCT event_date FROM events WHERE EXTRACT(YEAR FROM event_date) = $1 AND EXTRACT(MONTH FROM event_date) = $2 ORDER BY event_date",
        &[&year, &(month as i32)], // PostgreSQL month is 1-indexed, and EXTRACT returns double precision, but $2 is i32
    )?;

    let mut event_dates = Vec::new();
    for row in rows {
        let date: NaiveDate = row.get(0);
        event_dates.push(date);
    }
    Ok(event_dates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // Helper to get a client and ensure the events table is clean for some tests.
    // For tests that add data, they should clean up their specific data.
    // Using a shared test DB means tests should be careful not to interfere if run in parallel (not an issue here).
    fn setup_test_client() -> Client {
        let mut client = Client::connect(DB_URL, NoTls)
            .expect("Failed to connect to test database");
        
        // Ensure table exists (connect_db would do this, but calling it directly here for clarity)
        client.batch_execute("
            CREATE TABLE IF NOT EXISTS events (
                id SERIAL PRIMARY KEY,
                event_date DATE NOT NULL,
                event_description TEXT NOT NULL
            )
        ").expect("Failed to ensure events table exists");
        
        client
    }
    
    fn clear_event(client: &mut Client, date: NaiveDate, description: &str) {
        client.execute(
            "DELETE FROM events WHERE event_date = $1 AND event_description = $2",
            &[&date, &description],
        ).expect("Failed to delete test event");
    }

    #[test]
    fn test_connect_db_and_table_creation() {
        match connect_db() {
            Ok(mut client) => {
                // Check if table exists by trying to select from it (count will be 0 if empty)
                let result = client.query_one("SELECT COUNT(*) FROM events", &[]);
                assert!(result.is_ok(), "Querying events table failed, it might not exist or schema is wrong.");
            }
            Err(e) => {
                panic!("test_connect_db_and_table_creation failed: {}", e);
            }
        }
    }

    #[test]
    fn test_save_and_get_event() {
        let mut client = setup_test_client();
        let test_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let test_desc = "Unique test event for save_and_get";

        // Save the event
        match save_event(&mut client, test_date, test_desc) {
            Ok(_) => (),
            Err(e) => panic!("save_event failed: {}", e),
        }

        // Try to retrieve it
        let row_opt = client.query_opt(
            "SELECT event_description FROM events WHERE event_date = $1 AND event_description = $2",
            &[&test_date, &test_desc],
        ).expect("Querying for saved event failed");

        assert!(row_opt.is_some(), "Event was not saved or cannot be retrieved.");
        if let Some(row) = row_opt {
            let retrieved_desc: String = row.get(0);
            assert_eq!(retrieved_desc, test_desc);
        }
        
        // Cleanup
        clear_event(&mut client, test_date, test_desc);
    }
    
    #[test]
    fn test_get_events_for_month_no_events() {
        let mut client = setup_test_client();
        // Clear any existing events for a known month to ensure test isolation for "no events"
        client.execute("DELETE FROM events WHERE EXTRACT(YEAR FROM event_date) = 2099 AND EXTRACT(MONTH FROM event_date) = 1", &[])
              .expect("Failed to clear events for test_get_events_for_month_no_events");

        match get_events_for_month(&mut client, 2099, 1) { // Year 2099, Month 1 (Jan)
            Ok(dates) => {
                assert!(dates.is_empty(), "Expected no events for a month that should be empty, got: {:?}", dates);
            }
            Err(e) => panic!("get_events_for_month failed for empty month: {}", e),
        }
    }

    #[test]
    fn test_get_events_for_month_with_events() {
        let mut client = setup_test_client();
        let year = 2025;
        let month = 3; // March

        let date1 = NaiveDate::from_ymd_opt(year, month, 10).unwrap();
        let desc1 = "Event 1 for March 2025";
        let date2 = NaiveDate::from_ymd_opt(year, month, 15).unwrap();
        let desc2 = "Event 2 for March 2025";
        let date3_dup = NaiveDate::from_ymd_opt(year, month, 10).unwrap(); // Same date as date1
        let desc3_dup = "Event 3 (duplicate date) for March 2025";
        let date_other_month = NaiveDate::from_ymd_opt(year, month + 1, 5).unwrap();
        let desc_other_month = "Event in April 2025";

        // Save events
        save_event(&mut client, date1, desc1).unwrap();
        save_event(&mut client, date2, desc2).unwrap();
        save_event(&mut client, date3_dup, desc3_dup).unwrap();
        save_event(&mut client, date_other_month, desc_other_month).unwrap();

        match get_events_for_month(&mut client, year, month) {
            Ok(event_dates) => {
                assert_eq!(event_dates.len(), 2, "Expected 2 distinct event dates, got {:?}", event_dates);
                assert!(event_dates.contains(&date1), "Event dates should contain {}", date1);
                assert!(event_dates.contains(&date2), "Event dates should contain {}", date2);
                assert!(!event_dates.contains(&date_other_month), "Event dates should not contain event from another month");
            }
            Err(e) => panic!("get_events_for_month failed: {}", e),
        }

        // Cleanup
        clear_event(&mut client, date1, desc1);
        clear_event(&mut client, date2, desc2);
        clear_event(&mut client, date3_dup, desc3_dup);
        clear_event(&mut client, date_other_month, desc_other_month);
    }
}
