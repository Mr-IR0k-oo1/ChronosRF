use std::collections::VecDeque;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use crate::models::{AlertSeverity, IgorAssessment, TelemetryEvent};

const MAX_ASSESSMENTS: usize = 10;

#[derive(Clone, Debug)]
pub struct IgorPanel {
    assessments: VecDeque<IgorAssessment>,
    capacity: usize,
}

impl IgorPanel {
    pub fn from_snapshot(assessments: VecDeque<IgorAssessment>) -> Self {
        let capacity = assessments.len().max(MAX_ASSESSMENTS);

        Self {
            assessments,
            capacity,
        }
    }

    pub fn apply(&mut self, event: &TelemetryEvent) {
        if let TelemetryEvent::IgorAssessment(assessment) = event {
            self.assessments.push_back(assessment.clone());

            while self.assessments.len() > self.capacity {
                self.assessments.pop_front();
            }
        }
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, panel: &IgorPanel) {
    let title = format!("IGOR ({})", panel.assessments.len());

    if panel.assessments.is_empty() {
        frame.render_widget(
            Paragraph::new("No IGOR findings yet")
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let items = panel
        .assessments
        .iter()
        .rev()
        .take(MAX_ASSESSMENTS)
        .map(render_assessment_item)
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

fn render_assessment_item(assessment: &IgorAssessment) -> ListItem<'static> {
    let severity_style = severity_style(assessment.severity);
    let finding_style = match assessment.finding_kind {
        crate::models::IgorFindingKind::CoordinatedEmitter => {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        }
        crate::models::IgorFindingKind::PersistentEmitter => {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        }
        crate::models::IgorFindingKind::EscalatingBandActivity => {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        }
    };

    ListItem::new(Line::from(vec![
        Span::styled(format!("{:?}", assessment.severity), severity_style),
        Span::raw(" "),
        Span::styled(format!("{:?}", assessment.finding_kind), finding_style),
        Span::raw(" | score "),
        Span::styled(
            assessment.risk_score.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::raw(format!(
            "{}-{} MHz",
            assessment.frequency_start_hz / 1_000_000,
            assessment.frequency_end_hz / 1_000_000,
        )),
        Span::raw(" | "),
        Span::raw(truncate_message(&assessment.message, 72)),
    ]))
}

fn severity_style(severity: AlertSeverity) -> Style {
    match severity {
        AlertSeverity::Low => Style::default().fg(Color::Green),
        AlertSeverity::Medium => Style::default().fg(Color::Yellow),
        AlertSeverity::High => Style::default().fg(Color::Red),
        AlertSeverity::Critical => Style::default().fg(Color::Magenta),
    }
}

fn truncate_message(message: &str, max_chars: usize) -> String {
    let truncated = message.chars().take(max_chars).collect::<String>();
    if truncated.len() == message.len() {
        truncated
    } else {
        format!("{truncated}…")
    }
}