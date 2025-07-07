use core::prelude::v1;
use std::collections::{BTreeMap, VecDeque};

use crate::{
    event::{AppEvent, Event, EventHandler},
    gh_cli::{self, GithubJob},
};
use clap::Parser;
use ratatui::{
    DefaultTerminal,
    crossterm::{
        self,
        event::{KeyCode, KeyEvent, KeyModifiers},
    },
};
const MAX_DISPLAYED_JOBS: usize = 300;

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub job_details: VecDeque<GithubJob>,
    pub current_job_index: usize,
    pub events: EventHandler,
    pub app_state: AppState,
    pub gh_cli: crate::gh_cli::GhCli,
    pub args: crate::Args,
}

#[derive(Debug)]
pub struct AppState {
    pub column_index: usize,
    pub row_index: usize,
    pub show_details: bool,
    pub in_progress_jobs: BTreeMap<String, Vec<usize>>,
    pub success_jobs: BTreeMap<String, Vec<usize>>,
    pub failure_jobs: BTreeMap<String, Vec<usize>>,
    pub loading_status: String,
    pub scroll_offset: usize,
    pub selected_job: Option<GithubJob>
}

impl Default for App {
    fn default() -> Self {
        let args_obj = crate::Args::parse();
        let gh_cli_instance = gh_cli::GhCli::new(args_obj.branch, args_obj.user, args_obj.latest);
        Self {
            running: true,
            job_details: VecDeque::new(),
            current_job_index: 0,
            gh_cli: gh_cli_instance.clone(),
            events: EventHandler::new(gh_cli_instance),
            app_state: AppState {
                column_index: 0,
                row_index: 0,
                show_details: false,
                in_progress_jobs: BTreeMap::new(),
                success_jobs: BTreeMap::new(),
                failure_jobs: BTreeMap::new(),
                loading_status: "Initializing...".to_string(),
                scroll_offset: 0,
                selected_job: None,
            },
            args: args_obj,
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
                AppEvent::PageDown => self.change_scroll_offset(25),
                AppEvent::PageUp => self.change_scroll_offset(-25),
                AppEvent::OpenGitHub => self.open_github(),
            },
        }
        Ok(())
    }
    fn change_column_index(&mut self, delta: isize) {
        if self.app_state.show_details {
            return;
        }
        let num_columns = 3;
        let new_index = (self.app_state.column_index as isize + delta) as usize;

        self.app_state.column_index = new_index % num_columns;

        self.app_state.row_index = 0;
        self.app_state.scroll_offset = 0;

        self.update_current_job_index_from_state();
    }
    fn open_github(&mut self) {
        if let Some(job) = self.job_details.get(self.current_job_index) {
            let url = job.html_url.clone();
            if let Err(e) = open::that(url) {
                eprintln!("Error opening URL: {}", e);
            }
        }
    }

    fn change_row_index(&mut self, delta: isize) {
        if self.app_state.show_details {
            return;
        }
        let current_column_jobs = self.get_jobs_for_current_column();
        if current_column_jobs.is_empty() {
            self.app_state.row_index = 0;
            self.current_job_index = 0;
            return;
        }

        let mut new_row_index = self.app_state.row_index as isize + delta;

        // Ensure the row index stays within bounds
        if new_row_index < 0 {
            new_row_index = 0;
        }
        self.app_state.row_index =
            (new_row_index as usize).min(current_column_jobs.values().flatten().count().saturating_sub(1));

        // Update current_job_index based on the new row and column
        self.update_current_job_index_from_state();
    }
    fn change_scroll_offset(&mut self, delta: isize) {
        let new_offset = self.app_state.scroll_offset as isize + delta;
        if new_offset < 0 {
            self.app_state.scroll_offset = 0;
        } else {
            self.app_state.scroll_offset = new_offset as usize;
        }
    }

    fn update_current_job_index_from_state(&mut self) {
        let current_column_jobs_indices = self.get_jobs_for_current_column();
        let indices: Vec<usize> = current_column_jobs_indices
            .values()
            .flatten()
            .copied()
            .collect();
        if let Some(original_index) = indices.get(self.app_state.row_index) {
            self.current_job_index = *original_index;
        } else {
            // No job selected, default to first available or 0
            self.current_job_index = indices.first().copied().unwrap_or(0);
        }
    }

    fn get_jobs_for_current_column(&self) -> &BTreeMap<String, Vec<usize>> {
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
            KeyCode::PageDown => self.events.send(AppEvent::PageDown),
            KeyCode::PageUp => self.events.send(AppEvent::PageUp),
            KeyCode::Backspace => self.events.send(AppEvent::OpenGitHub),
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
    pub fn update_github_data(&mut self, workflow_data: crate::gh_cli::WorkflowData) {
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
        let mut sorted_jobs: Vec<(usize, &crate::gh_cli::GithubJob)> =
            self.job_details.iter().enumerate().collect();

        sorted_jobs.sort_by(|(_, a), (_, b)| {
            b.started_at.cmp(&a.started_at) // Sort descending
        });


        for (original_index, job) in sorted_jobs {
            let tool = self.parse_job_name_for_tool(&job.name);
            match job.status.as_str() {
                "completed" => {
                    if let Some(conclusion) = &job.conclusion {
                        match conclusion.as_str() {
                            "success" => self.app_state.success_jobs.entry(tool).or_default().push(original_index),
                            "failure" => self.app_state.failure_jobs.entry(tool).or_default().push(original_index),
                            _ => { /* Ignore cancelled, skipped, etc. as per request */ }
                        }
                    }
                }
                "in_progress" | "queued" | "waiting" => {
                    self.app_state.in_progress_jobs.entry(tool).or_default().push(original_index)
                }
                _ => { /* Ignore other statuses if any */ }
            }
        }

        // Ensure current_job_index is valid after update and re-filtering
        self.update_current_job_index_from_state();
    }
    pub fn parse_job_name_for_tool(&self, job_name: &str) -> String {
        let parts: Vec<&str> = job_name.split(" / ").collect();
        parts.get(0).unwrap_or(&"Other").to_string()
    }
}
