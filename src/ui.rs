use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};
use std::collections::BTreeMap; // Using BTreeMap for sorted group keys

use crate::app::App; // Assuming App struct is defined here

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
    // Helper to get the tool from a job name.
    // This is the common parsing logic used for grouping and display.
    fn parse_job_name_for_tool(&self, job_name: &str) -> String {
        let parts: Vec<&str> = job_name.split(" / ").collect();
        parts.get(0).unwrap_or(&"Other").to_string()
    }

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

    // Reusable function to render a single column of job summaries with grouping
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

        // Group jobs by their "tool"
        let mut grouped_jobs: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &original_idx in job_indices {
            let job = &self.job_details[original_idx];
            let tool = self.parse_job_name_for_tool(&job.name);
            grouped_jobs.entry(tool).or_default().push(original_idx);
        }

        // Each job entry takes 4 lines + 1 for separator = 5 lines total.
        // Each group header takes 1 line + 1 for separator = 2 lines total.
        // We'll approximate an average of 5 lines per "displayable unit" for height calculation.
        // This makes `max_jobs_by_height` less accurate if group headers are sparse,
        // but it's a rough estimate for overall scrolling behavior.
        let lines_per_job_summary = 4;
        let separator_lines = 1; // Between jobs/groups
        let total_lines_per_job_entry = lines_per_job_summary + separator_lines;
        let group_header_lines = 1;
        let total_lines_per_group_header = group_header_lines + separator_lines;

        let available_height = inner_area.height as usize;
        let mut current_height = 0;
        let mut all_summary_lines: Vec<Line> = Vec::new();

        let mut rendered_job_count = 0;
        let mut global_rendered_row_index = 0; // Tracks the *visible* row for selection

        // Iterate through grouped jobs (BTreeMap ensures sorted order by key)
        for (tool_name, indices_in_group) in grouped_jobs.iter() {
            // Check if we have enough height for at least the group header
            if current_height + total_lines_per_group_header > available_height {
                break; // Not enough space for another group header
            }

            // Render group header
            all_summary_lines.push(Line::from(vec![
                Span::raw("── "),
                Span::styled(
                    tool_name.clone(),
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::raw(" ──"),
            ]));
            all_summary_lines.push(Line::from(Span::styled(
                "─",
                Style::default().fg(Color::DarkGray),
            ))); // Separator below group header
            current_height += total_lines_per_group_header;

            // Iterate through jobs within this group
            for &original_job_idx in indices_in_group {
                // Check if we have enough height for the next job entry
                if current_height + total_lines_per_job_entry > available_height {
                    // If we're already scrolled past the selected row,
                    // or if the selected row is in a later group that won't be shown,
                    // we need to adjust scrolling if it's currently selecting nothing visible.
                    break;
                }

                let job = &self.job_details[original_job_idx];

                // Determine if this is the currently selected row globally (across all visible jobs in column)
                // This is the new part for selection tracking
                let is_selected_row_globally = is_selected_column
                    && self.app_state.column_index == column_idx
                    && self.app_state.row_index == global_rendered_row_index;

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

                let base_style = if is_selected_row_globally {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };

                // --- Robust Parsing of the job name for display ---
                let (action_part, tool_workflow_service_parts_for_display) =
                    if let Some((before_dash, after_dash)) = job.name.split_once(" - ") {
                        (after_dash, before_dash.split(" / ").collect::<Vec<&str>>())
                    } else {
                        (
                            job.name.as_str(),
                            job.name.split(" / ").collect::<Vec<&str>>(),
                        )
                    };

                // Skip the first part (the tool) as it's handled by the group header
                let workflow_part = tool_workflow_service_parts_for_display
                    .get(1)
                    .unwrap_or(&"");
                let service_or_sub_service_part = tool_workflow_service_parts_for_display
                    .get(2)
                    .unwrap_or(&"");

                // Line 1: Index, Action (or primary name), Status, Conclusion
                all_summary_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}. ", rendered_job_count + 1), // Number relative to column view
                        base_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        action_part.to_string(), // Display the parsed action/primary name
                        base_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" [", status_style),
                    Span::styled(job.status.clone(), status_style),
                    conclusion_span,
                    Span::styled("]", status_style),
                ]));

                // Line 2: Workflow (conditionally displayed)
                if !workflow_part.is_empty() {
                    all_summary_lines.push(Line::from(vec![
                        Span::raw("  "), // Indent for readability
                        Span::styled(
                            format!("{}", workflow_part),
                            base_style.fg(Color::LightYellow),
                        ),
                    ]));
                } else {
                    all_summary_lines.push(Line::from(Span::raw("")));
                }

                // Line 3: Service/Sub-service (conditionally displayed and indented)
                if !service_or_sub_service_part.is_empty() {
                    all_summary_lines.push(Line::from(vec![Span::styled(
                        format!("    {}", service_or_sub_service_part),
                        base_style.fg(Color::White).add_modifier(Modifier::ITALIC),
                    )]));
                } else {
                    all_summary_lines.push(Line::from(Span::raw("")));
                }

                // Line 4: Branch and Actor
                all_summary_lines.push(Line::from(vec![Span::styled(
                    format!("  {} by {}", job.head_branch, job.actor_login),
                    base_style
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )]));

                current_height += total_lines_per_job_entry;
                rendered_job_count += 1;
                global_rendered_row_index += 1;

                // Add separator if it's not the last job overall, and there's space
                if current_height + separator_lines <= available_height {
                    all_summary_lines.push(Line::from(Span::styled(
                        "---",
                        Style::default().fg(Color::DarkGray),
                    )));
                    current_height += separator_lines;
                }
            }
        }

        // Adjust the paragraph content and render it
        let paragraph = Paragraph::new(all_summary_lines).wrap(Wrap { trim: false });
        paragraph.render(inner_area, buf);

        // --- Selection adjustment logic (Simplified for direct rendering context) ---
        // The App's `row_index` now refers to the *absolute* index within the currently
        // active column's `job_indices` list, not a "visible" row.
        // We'd ideally manage the `row_index` in `App::handle_event` methods
        // to move between jobs in the *grouped* list, not just a flat list.
        // For rendering, we iterate through the grouped list and mark the corresponding `global_rendered_row_index`.

        // To handle scrolling if the selected item is out of view, you'd usually
        // introduce a `scroll_offset` into `App` state and use it here.
        // For brevity, I'm omitting scroll offset logic here, assuming `global_rendered_row_index`
        // is reset or managed by an outer scroll state.
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
