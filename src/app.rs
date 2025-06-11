use std::collections::VecDeque;

use crate::event::{AppEvent, Event, EventHandler};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde::Deserialize;
const MAX_DISPLAYED_JOBS: usize = 300;

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub job_details: VecDeque<crate::event::GithubJob>,
    pub current_job_index: usize,
    pub events: EventHandler,
    pub app_state: AppState,
}

#[derive(Debug)]
pub struct AppState {
    pub column_index: usize,
    pub row_index: usize,
    pub show_details: bool,
    // Store the filtered jobs for each column to make navigation easier
    pub in_progress_jobs: Vec<usize>, // Stores original indices from job_details
    pub success_jobs: Vec<usize>,
    pub failure_jobs: Vec<usize>,
    pub loading_status: String, // Added for UI feedback
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct RepoInfo {
    pub name: String,
    pub owner: Owner,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Owner {
    pub login: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            job_details: VecDeque::new(),
            current_job_index: 0,
            events: EventHandler::new(),
            app_state: AppState {
                column_index: 0,
                row_index: 0,
                show_details: false,
                in_progress_jobs: Vec::new(),
                success_jobs: Vec::new(),
                failure_jobs: Vec::new(),
                loading_status: "Initializing...".to_string(), // Initial status
            },
        }
    }
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.running {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            self.handle_events()?;
        }
        Ok(())
    }

    pub fn handle_events(&mut self) -> color_eyre::Result<()> {
        match self.events.next()? {
            Event::Action => {
                // This event now only signals that a data fetch has been *triggered*.
                // You can update a loading status in the UI here.
                self.app_state.loading_status = "Fetching data...".to_string();
            }
            Event::GitHubDataFetched(result) => {
                // This is where the actual data (or error) arrives.
                match result {
                    Ok(workflow_data) => {
                        self.update_github_data(workflow_data);
                        self.app_state.loading_status = "Data updated.".to_string(); // Or clear it
                    }
                    Err(e) => {
                        self.app_state.loading_status = format!("Error: {}", e);
                        eprintln!("Error fetching GitHub data: {}", e);
                    }
                }
            }
            Event::Crossterm(event) => match event {
                crossterm::event::Event::Key(key_event) => self.handle_key_event(key_event)?,
                _ => {}
            },
            Event::App(app_event) => match app_event {
                AppEvent::Quit => self.quit(),
                AppEvent::NavigateRight => self.change_column_index(1),
                AppEvent::NavigateLeft => self.change_column_index(-1),
                AppEvent::NavigateUp => self.change_row_index(-1),
                AppEvent::NavigateDown => self.change_row_index(1),
                AppEvent::ToggleDetails => self.toggle_details_panel(),
            },
        }
        Ok(())
    }
    fn change_column_index(&mut self, delta: isize) {
        let num_columns = 3;
        let new_index = (self.app_state.column_index as isize + delta) as usize;

        // Ensure the new index wraps around
        self.app_state.column_index = new_index % num_columns;

        // Reset row index when changing columns
        self.app_state.row_index = 0;

        // When changing columns, ensure the selected job index is valid
        self.update_current_job_index_from_state();
    }

    fn change_row_index(&mut self, delta: isize) {
        let current_column_jobs = self.get_jobs_for_current_column();
        if current_column_jobs.is_empty() {
            self.app_state.row_index = 0;
            self.current_job_index = 0;
            return;
        }

        let new_row_index = (self.app_state.row_index as isize + delta) as usize;

        // Ensure the row index stays within bounds
        self.app_state.row_index = new_row_index.min(current_column_jobs.len().saturating_sub(1));

        // Update current_job_index based on the new row and column
        self.update_current_job_index_from_state();
    }

    fn update_current_job_index_from_state(&mut self) {
        let current_column_jobs_indices = self.get_jobs_for_current_column();
        if let Some(original_index) = current_column_jobs_indices.get(self.app_state.row_index) {
            self.current_job_index = *original_index;
        } else {
            // No job selected, default to first available or 0
            self.current_job_index = current_column_jobs_indices.first().cloned().unwrap_or(0);
        }
    }

    fn get_jobs_for_current_column(&self) -> &Vec<usize> {
        match self.app_state.column_index {
            0 => &self.app_state.in_progress_jobs,
            1 => &self.app_state.success_jobs,
            2 => &self.app_state.failure_jobs,
            _ => unreachable!(), // Should not happen with 0..2
        }
    }

    fn toggle_details_panel(&mut self) {
        self.app_state.show_details = !self.app_state.show_details;
    }

    /// Handles the key events and updates the state of [`App`].
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Right => self.events.send(AppEvent::NavigateRight),
            KeyCode::Left => self.events.send(AppEvent::NavigateLeft),
            KeyCode::Up => self.events.send(AppEvent::NavigateUp),
            KeyCode::Down => self.events.send(AppEvent::NavigateDown),
            KeyCode::Enter => self.events.send(AppEvent::ToggleDetails),
            _ => {}
        }
        Ok(())
    }

    /// Handles the tick event of the terminal.
    ///
    /// The tick event is where you can update the state of your application with any logic that
    /// needs to be updated at a fixed frame rate. E.g. polling a server, updating an animation.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    // Now accepts `WorkflowData` directly
    pub fn update_github_data(&mut self, workflow_data: crate::event::WorkflowData) {
        self.job_details.clear();
        for job in workflow_data.jobs {
            if self.job_details.len() >= MAX_DISPLAYED_JOBS {
                self.job_details.pop_front();
            }
            self.job_details.push_back(job);
        }

        // After updating job_details, re-filter them into state vectors
        self.app_state.in_progress_jobs.clear();
        self.app_state.success_jobs.clear();
        self.app_state.failure_jobs.clear();

        // Sort by started_at in descending order for better visualization
        // (most recent jobs at the top of the display lists)
        let mut sorted_jobs: Vec<(usize, &crate::event::GithubJob)> =
            self.job_details.iter().enumerate().collect();

        sorted_jobs.sort_by(|(_, a), (_, b)| {
            b.started_at.cmp(&a.started_at) // Sort descending
        });

        for (original_index, job) in sorted_jobs {
            match job.status.as_str() {
                "completed" => {
                    if let Some(conclusion) = &job.conclusion {
                        match conclusion.as_str() {
                            "success" => self.app_state.success_jobs.push(original_index),
                            "failure" => self.app_state.failure_jobs.push(original_index),
                            _ => { /* Ignore cancelled, skipped, etc. as per request */ }
                        }
                    }
                }
                "in_progress" | "queued" | "waiting" => {
                    self.app_state.in_progress_jobs.push(original_index)
                }
                _ => { /* Ignore other statuses if any */ }
            }
        }

        // Ensure current_job_index is valid after update and re-filtering
        self.update_current_job_index_from_state();
    }
}
