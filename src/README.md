# Terminal Calendar Application

**Calendar** is an interactive terminal-based calendar application with event scheduling capabilities, built using Rust. It utilizes an ncurses interface for display and interaction, and PostgreSQL for event storage.

## Features

*   **Interactive Ncurses Interface**: Provides a responsive and terminal-friendly user experience.
*   **Monthly Calendar View**: Displays a full month, allowing users to see dates and days of the week.
*   **Date Navigation**: Use arrow keys (Up, Down, Left, Right) to easily navigate between dates. The calendar view updates to the new month/year as you navigate.
*   **Event Scheduling**: Add textual events to any selected date.
*   **PostgreSQL Backend**: Events are persistently stored in a PostgreSQL database.
*   **Event Indicators**: Dates with one or more events are visually marked with an asterisk (`*`) in the calendar view.
*   **Command-Line Options**: Specify a year or month to view directly upon startup.

## Dependencies

This project relies on several Rust crates, including:
*   `pancurses` for the ncurses terminal interface.
*   `postgres` for PostgreSQL database interaction.
*   `chrono` for date and time operations.
*   `clap` for command-line argument parsing.

## Database Setup

A PostgreSQL database is required to store events.

1.  **Install PostgreSQL**: Ensure you have PostgreSQL installed and running.
2.  **Database and User**:
    *   The application expects a database named `calendar_db`.
    *   It uses the connection string: `postgresql://cal_user:cal_pass@localhost:5432/calendar_db`.
    *   You need to create the user `cal_user` with the password `cal_pass` and grant it permissions to connect to `calendar_db` and create tables/read/write data.
    *   Example PSQL commands:
        ```sql
        CREATE DATABASE calendar_db;
        CREATE USER cal_user WITH PASSWORD 'cal_pass';
        GRANT ALL PRIVILEGES ON DATABASE calendar_db TO cal_user;
        -- Connect to calendar_db as a superuser or the database owner to grant table permissions if needed,
        -- though the application attempts to create the table which might require cal_user to have those rights.
        -- Alternatively, after first run (which creates the table), grant specific rights on the table:
        -- \c calendar_db
        -- GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE events TO cal_user;
        ```
3.  **Table Creation**: The application will automatically attempt to create the necessary `events` table if it doesn't exist upon its first connection.
4.  **Custom Connection**: Currently, the database connection string is hardcoded in `src/db.rs` (constant `DB_URL`). If you need to use a different database, user, password, host, or port, you will need to modify this constant and recompile the application.

## Usage

### Building the Application

1.  Ensure you have Rust and Cargo installed.
2.  Ensure PostgreSQL development libraries are installed on your system.
    *   On Debian/Ubuntu: `sudo apt-get install libpq-dev`
    *   On Fedora/RHEL: `sudo dnf install postgresql-devel`
3.  Navigate to the project root directory.
4.  Build the application:
    ```bash
    cargo build --release
    ```
    The executable will be located at `target/release/calendar`.

### Running the Application

Execute the compiled binary:

```bash
./target/release/calendar [OPTIONS] [YEAR_OR_MONTH] [MONTH]
```

**Command-Line Options:**

*   `-y, --year`: When followed by a year (e.g., `calendar -y 2024`), it's intended to show the whole year. (Note: Full year view in ncurses is not yet implemented; this will show a message).
*   `-s, --single-month`: This flag is implicitly handled as the ncurses view currently shows one month.
*   `[YEAR_OR_MONTH]`: Specify a year (e.g., `2024`) to view the current month of that year, or a month number (1-12) to view that month of the current year.
*   `[YEAR_OR_MONTH] [MONTH]`: Specify a year and a month (1-12) to view (e.g., `calendar 2024 12` for December 2024).

If no arguments are provided, the calendar will open to the current month with the current day selected.

**Key Bindings:**

*   **Arrow Keys (Up, Down, Left, Right)**: Navigate between dates. The calendar view will update if you navigate to a different month or year.
*   **Enter**: On a selected date, press Enter to open the "Add Event" dialog.
    *   In the Event Dialog: Type your event description.
    *   In the Event Dialog: Press **Enter** to save the event to the database.
*   **'q'**: Quit the application from the main calendar view.

## Development

The project is structured with modules for calendar logic (`calendar.rs`), database interaction (`db.rs`), and the main application setup and UI (`main.rs`).
The `pancurses` library is used for all ncurses interactions. Event data is stored and retrieved via the `postgres` crate.
