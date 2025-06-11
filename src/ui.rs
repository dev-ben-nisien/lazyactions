use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};

use crate::app::App;

impl Widget for &App {
    /// Renders the user interface widgets.
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Define the main layout to split the screen vertically
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // Fixed height for header/instructions
                Constraint::Min(0), // Remaining space for job details columns or columns + details
            ])
            .split(area);

        // --- Render the header/instructions paragraph ---
        let header_block = Block::bordered()
            .title("lazyactions")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta));

        let header_text = format!(
            "Showing jobs for: {} | Fetch Status: {}\n\
                Press `Esc`, `Ctrl-C` or `q` to stop running. \n\
                Use `Left`/`Right` to navigate columns, `Up`/`Down` for rows.\n\
                Press `Enter` to toggle job details. Auto-refresh every 5 seconds.",
            self.job_details
                .front()
                .map_or("N/A", |job| job.repo.as_str()),
            self.app_state.loading_status
        );

        let header_paragraph = Paragraph::new(header_text)
            .block(header_block)
            .fg(Color::Cyan)
            .bg(Color::Black)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        header_paragraph.render(main_chunks[0], buf);

        // --- Render the job columns and potentially the details panel ---
        if self.app_state.show_details {
            // Split the bottom chunk for columns and details panel
            let app_body_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(70), // 70% for job columns
                    Constraint::Percentage(30), // 30% for details panel
                ])
                .split(main_chunks[1]);

            self.render_job_columns(app_body_chunks[0], buf);
            self.render_job_details_panel(app_body_chunks[1], buf);
        } else {
            // Just render columns if details are not shown
            self.render_job_columns(main_chunks[1], buf);
        }
    }
}

impl App {
    // This new function manages the three-column layout
    fn render_job_columns(&self, area: Rect, buf: &mut Buffer) {
        // Define the horizontal layout for the three columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33), // In Progress
                Constraint::Percentage(34), // Concluded Success
                Constraint::Percentage(33), // Concluded Failure
            ])
            .split(area);

        // Render each column using the helper function
        self.render_job_list_column(
            columns[0],
            buf,
            "In Progress",
            &self.app_state.in_progress_jobs,
            Color::Yellow, // Color for in progress
            0,             // Column index for 'In Progress'
        );

        self.render_job_list_column(
            columns[1],
            buf,
            "Concluded Success",
            &self.app_state.success_jobs,
            Color::Green, // Color for success
            1,            // Column index for 'Concluded Success'
        );

        self.render_job_list_column(
            columns[2],
            buf,
            "Concluded Failure",
            &self.app_state.failure_jobs,
            Color::Red, // Color for failure
            2,          // Column index for 'Concluded Failure'
        );
    }

    // Reusable function to render a single column of job summaries
    fn render_job_list_column(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        job_indices: &[usize], // Now takes a slice of original indices
        border_color: Color,
        column_idx: usize, // New parameter for column index
    ) {
        let is_selected_column = self.app_state.column_index == column_idx;

        let block =
            Block::default()
                .title(format!("{} ({})", title, job_indices.len()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color).add_modifier(
                    if is_selected_column {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                ));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if job_indices.is_empty() {
            let no_data_text = Text::styled(
                "No jobs in this category.",
                Style::default().fg(Color::DarkGray),
            );
            let paragraph = Paragraph::new(no_data_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
            return;
        }

        let lines_per_job_summary = 3;
        let separator_lines = 1;
        let total_lines_per_job_entry = lines_per_job_summary + separator_lines;

        let available_height = inner_area.height as usize;
        let max_jobs_by_height = if available_height >= lines_per_job_summary {
            let full_entries =
                (available_height - lines_per_job_summary) / total_lines_per_job_entry;
            full_entries + 1
        } else {
            0
        };

        let mut all_summary_lines: Vec<Line> = Vec::new();

        let num_jobs_in_category = job_indices.len();
        let actual_jobs_to_display = max_jobs_by_height.min(num_jobs_in_category);

        for i in 0..actual_jobs_to_display {
            let original_job_idx = job_indices[i];
            let job = &self.job_details[original_job_idx]; // Get the actual job data

            let is_selected_row = is_selected_column && self.app_state.row_index == i;

            let status_style = match job.status.as_str() {
                "completed" => Style::default().fg(Color::Green),
                "in_progress" => Style::default().fg(Color::Yellow),
                "queued" | "waiting" => Style::default().fg(Color::DarkGray),
                _ => Style::default().fg(Color::White),
            };

            let conclusion_span = if let Some(conclusion) = &job.conclusion {
                let conclusion_style = match conclusion.as_str() {
                    "success" => Style::default().fg(Color::LightGreen),
                    "failure" => Style::default().fg(Color::Red),
                    "cancelled" => Style::default().fg(Color::DarkGray),
                    "skipped" => Style::default().fg(Color::Blue),
                    _ => Style::default().fg(Color::White),
                };
                Span::styled(format!(" ({})", conclusion), conclusion_style)
            } else {
                Span::raw("")
            };

            let base_style = if is_selected_row {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(Color::White)
            };

            all_summary_lines.push(Line::from(vec![
                Span::styled(
                    format!("{}. ", i + 1),
                    base_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", job.name),
                    base_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", status_style),
                Span::styled(job.status.clone(), status_style),
                conclusion_span,
                Span::styled("]", status_style),
            ]));
            all_summary_lines.push(Line::from(vec![Span::styled(
                format!("  {} by {}", job.head_branch, job.actor_login),
                base_style.fg(Color::Gray).add_modifier(Modifier::ITALIC),
            )]));

            if i < actual_jobs_to_display.saturating_sub(1) {
                all_summary_lines.push(Line::from(Span::styled(
                    "---",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        let paragraph = Paragraph::new(all_summary_lines).wrap(Wrap { trim: false });

        paragraph.render(inner_area, buf);
    }

    fn render_job_details_panel(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("Job Details")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let selected_job = self.job_details.get(self.current_job_index);

        if let Some(job) = selected_job {
            let mut details_text = Vec::new();

            details_text.push(Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.name.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Repo: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.repo.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Run ID: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.run_id.to_string()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::LightBlue)),
                Span::styled(
                    job.status.clone(),
                    match job.status.as_str() {
                        "completed" => Style::default().fg(Color::Green),
                        "in_progress" => Style::default().fg(Color::Yellow),
                        "queued" | "waiting" => Style::default().fg(Color::DarkGray),
                        _ => Style::default().fg(Color::White),
                    },
                ),
            ]));
            if let Some(conclusion) = &job.conclusion {
                details_text.push(Line::from(vec![
                    Span::styled("Conclusion: ", Style::default().fg(Color::LightBlue)),
                    Span::styled(
                        conclusion.clone(),
                        match conclusion.as_str() {
                            "success" => Style::default().fg(Color::LightGreen),
                            "failure" => Style::default().fg(Color::Red),
                            "cancelled" => Style::default().fg(Color::DarkGray),
                            "skipped" => Style::default().fg(Color::Blue),
                            _ => Style::default().fg(Color::White),
                        },
                    ),
                ]));
            }
            details_text.push(Line::from(vec![
                Span::styled("Branch: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.head_branch.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Actor: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.actor_login.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("URL: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.html_url.clone()).add_modifier(Modifier::UNDERLINED),
            ]));

            let paragraph = Paragraph::new(details_text).wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
        } else {
            let no_job_selected_text = Text::styled(
                "No job selected. Use navigation keys to select a job.",
                Style::default().fg(Color::DarkGray),
            );
            let paragraph = Paragraph::new(no_job_selected_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
        }
    }
}
