use std::{collections::VecDeque, process::Command};

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
}

#[derive(Debug, Default, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub owner: Owner,
}

#[derive(Debug, Default, Deserialize)]
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
            Event::Action(dataset) => self.update_github_data(dataset),
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
            },
        }
        Ok(())
    }
    fn change_column_index(&mut self, delta: isize) {
        let new_index = (self.app_state.column_index as isize + delta) as usize;
        if new_index < 3 {
            self.app_state.column_index = new_index;
        } else if new_index < 0 {
            self.app_state.column_index = 2;
        } else {
            self.app_state.column_index = 0;
        }
    }
    fn change_row_index(&mut self, delta: isize) {
        self.app_state.row_index = (self.app_state.row_index as isize + delta) as usize;
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
    pub fn update_github_data(&mut self, data: Option<crate::event::WorkflowData>) {
        if let Some(workflow_data) = data {
            // Clear existing jobs and add new ones, maintaining the desired limit
            // Or you could append new jobs and prune older ones if you want a scrolling history
            self.job_details.clear();
            for job in workflow_data.jobs {
                if self.job_details.len() >= MAX_DISPLAYED_JOBS {
                    self.job_details.pop_front(); // Remove oldest if exceeding limit
                }
                self.job_details.push_back(job);
            }
            // Ensure current_job_index is valid after update
            self.current_job_index = self
                .current_job_index
                .min(self.job_details.len().saturating_sub(1));
        }
        // If data is None (due to API error), we just keep the old data or clear it.
        // For now, we keep it, but clearing might also be an option.
    }
}
