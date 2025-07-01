use color_eyre::eyre::WrapErr; // `eyre` might not be strictly needed here anymore, but keeping for safety.
use ratatui::crossterm::event::{self, Event as CrosstermEvent};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Import the necessary components from the new gh_cli module
use crate::gh_cli::{GhCli, WorkflowData};

/// The frequency at which tick events are emitted.
const TICK_FPS: f64 = 0.15;

/// Representation of all possible events.
#[derive(Clone, Debug)]
pub enum Event {
    /// An event that is emitted on a regular schedule.
    Action, // This will now trigger a *background* fetch, not carry data directly
    /// Event carrying the result of the background GitHub data fetch.
    GitHubDataFetched(Result<WorkflowData, String>), // Carries result or error
    /// Crossterm events.
    Crossterm(CrosstermEvent),
    /// Application events.
    App(AppEvent),
}

/// Application events.
#[derive(Clone, Debug)]
pub enum AppEvent {
    NavigateLeft,
    NavigateRight,
    NavigateUp,
    NavigateDown,
    Quit,
    ToggleDetails,
    PageUp,
    PageDown,
    OpenGitHub,
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new(gh_cli: GhCli) -> Self {
        let (sender, receiver) = mpsc::channel();
        let actor = EventThread::new(sender.clone(), gh_cli);
        thread::spawn(|| actor.run());
        Self { sender, receiver }
    }

    /// Receives an event from the sender.
    pub fn next(&self) -> color_eyre::Result<Event> {
        Ok(self.receiver.recv()?)
    }

    /// Queue an app event to be sent to the event receiver.
    pub fn send(&mut self, app_event: AppEvent) {
        let _ = self.sender.send(Event::App(app_event));
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
struct EventThread {
    sender: mpsc::Sender<Event>,
    gh_cli: GhCli, // Use the new GhCli struct
}

impl EventThread {
    /// Constructs a new instance of [`EventThread`].
    fn new(sender: mpsc::Sender<Event>, gh_cli: GhCli) -> Self {
        Self { sender, gh_cli }
    }

    /// Runs the event thread.
    fn run(self) -> color_eyre::Result<()> {
        let tick_interval = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut last_tick = Instant::now();
        let mut first = true; // Flag to ensure an immediate first fetch

        loop {
            let timeout = tick_interval.saturating_sub(last_tick.elapsed());

            // If it's time for a tick or it's the very first run, trigger an action
            if timeout == Duration::ZERO || first {
                last_tick = Instant::now();
                first = false; // Reset first run flag after the initial tick

                // Send an `Action` event to trigger the fetch
                self.send(Event::Action);

                // Spawn a new thread for the potentially blocking network call
                let sender_clone = self.sender.clone();
                let gh_cli_clone = self.gh_cli.clone(); // Clone GhCli for the new thread
                thread::spawn(move || {
                    match gh_cli_clone.fetch_github_workflow_data() {
                        // Call method on GhCli instance
                        Ok(data) => sender_clone.send(Event::GitHubDataFetched(Ok(data))),
                        Err(e) => sender_clone.send(Event::GitHubDataFetched(Err(format!(
                            "Error fetching GitHub data via gh CLI: {:?}",
                            e
                        )))),
                    }
                });
            }

            // Poll for crossterm events
            if event::poll(timeout).wrap_err("failed to poll for crossterm events")? {
                let event = event::read().wrap_err("failed to read crossterm event")?;
                self.send(Event::Crossterm(event));
            }
        }
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}
