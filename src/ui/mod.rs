pub mod alerts;
pub mod igor;
pub mod occupancy;
pub mod spectrum;
pub mod status;
pub mod waterfall;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;
use tokio::sync::broadcast;
use tokio::time::interval;

use crate::core::logger;
use crate::models::{Event, TelemetryEvent};
use crate::state::{AppState, SnapshotStore};

use self::alerts::AlertsPanel;
use self::igor::IgorPanel;
use self::occupancy::OccupancyPanel;
use self::spectrum::SpectrumPanel;
use self::status::StatusPanel;
use self::waterfall::WaterfallPanel;

const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_millis(120);

pub struct UiRuntime {
    event_receiver: broadcast::Receiver<Event>,
    status_receiver: broadcast::Receiver<TelemetryEvent>,
    model: DashboardModel,
    refresh_interval: Duration,
}

struct DashboardModel {
    status: StatusPanel,
    spectrum: SpectrumPanel,
    waterfall: WaterfallPanel,
    alerts: AlertsPanel,
    occupancy: OccupancyPanel,
    igor: IgorPanel,
}

impl DashboardModel {
    fn from_snapshot(snapshot: SnapshotStore) -> Self {
        Self {
            status: StatusPanel::from_snapshot(&snapshot),
            spectrum: SpectrumPanel::default(),
            waterfall: WaterfallPanel::default(),
            alerts: AlertsPanel::from_snapshot(snapshot.alerts),
            occupancy: OccupancyPanel::from_snapshot(snapshot.occupancy),
            igor: IgorPanel::from_snapshot(snapshot.igor_assessments),
        }
    }

    fn apply(&mut self, event: &TelemetryEvent) {
        self.status.apply(event);
        self.spectrum.apply(event);
        self.waterfall.apply(event);
        self.alerts.apply(event);
        self.occupancy.apply(event);
        self.igor.apply(event);
    }
}

impl UiRuntime {
    pub async fn new(app_state: AppState) -> Result<Self> {
        let event_receiver = app_state.event_bus().subscribe();
        let status_receiver = app_state.telemetry_tx.subscribe();
        let snapshot = app_state.snapshots().await;

        Ok(Self {
            event_receiver,
            status_receiver,
            model: DashboardModel::from_snapshot(snapshot),
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
        })
    }

    pub async fn run(self) -> Result<()> {
        let _guard = TerminalGuard::enter()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
            .context("initialize terminal backend")?;
        let mut ticker = interval(self.refresh_interval);
        let mut event_receiver = self.event_receiver;
        let mut status_receiver = self.status_receiver;
        let mut model = self.model;

        terminal
            .draw(|frame| render_dashboard(frame, &model))
            .context("draw initial dashboard")?;

        loop {
            tokio::select! {
                received = event_receiver.recv() => {
                    match received {
                        Ok(event) => {
                            let projected = TelemetryEvent::from(event);
                            model.apply(&projected);
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                received = status_receiver.recv() => {
                    match received {
                        Ok(event) if matches!(
                            event,
                            TelemetryEvent::Health(_)
                                | TelemetryEvent::Status(_)
                                | TelemetryEvent::RecordingStatus(_)
                                | TelemetryEvent::PlaybackStatus(_)
                        ) => model.apply(&event),
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = ticker.tick() => {
                    terminal
                        .draw(|frame| render_dashboard(frame, &model))
                        .context("draw dashboard frame")?;
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.context("wait for ctrl-c")?;
                    break;
                }
            }
        }

        terminal.show_cursor().context("restore terminal cursor")?;
        Ok(())
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("enable raw mode")?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("enter alternate screen")?;

        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn render_dashboard(frame: &mut Frame<'_>, model: &DashboardModel) {
    let root = frame.area();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(10), Constraint::Length(2)])
        .split(root);

    status::render(frame, areas[0], &model.status);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(areas[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(13), Constraint::Min(10)])
        .split(body[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Length(11), Constraint::Min(10)])
        .split(body[1]);

    spectrum::render(frame, left[0], &model.spectrum);
    waterfall::render(frame, left[1], &model.waterfall);
    alerts::render(frame, right[0], &model.alerts);
    occupancy::render(frame, right[1], &model.occupancy);
    igor::render(frame, right[2], &model.igor);

    let footer = Paragraph::new("Ctrl-C to exit | Live telemetry rendered from the broadcast bus")
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, areas[2]);

    let _ = logger::now_ms();
}
