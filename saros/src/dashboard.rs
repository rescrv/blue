//! Tools for building dashboards on top of a saros query engine.
//!
//! It starts with a single [Dashboard].  A dashboard is the logical unit of rendering.  A
//! dashboard composes many [Panel] objects, displaying each one under its own label.  A panel
//! composes many [Plot] objects, displaying each one as a line, or set of lines.

use std::io::Write;

use super::{Error, Query, QueryEngine, Window};

const STEPS: &[char] = &['_', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/////////////////////////////////////////////// Plot ///////////////////////////////////////////////

/// Plot captures a single time-series query.  If the query returns multiple series, they will be
/// displayed in order, one-per-line.
#[derive(Debug)]
pub struct Plot {
    label: String,
    query: Query,
    y_min: Option<f64>,
    y_max: Option<f64>,
}

impl Plot {
    /// Create a new time-series and label it.  The query can return multiple series, in which
    /// case they will be returned one-per-line.
    pub fn new<S: AsRef<str>>(label: S, query: Query) -> Self {
        Self {
            label: label.as_ref().to_string(),
            query,
            y_min: None,
            y_max: None,
        }
    }

    /// Set y-min for this plot.
    pub fn with_y_min(mut self, y_min: f64) -> Self {
        self.y_min = Some(y_min);
        self
    }

    /// Set y-max for this plot.
    pub fn with_y_max(mut self, y_max: f64) -> Self {
        self.y_max = Some(y_max);
        self
    }

    /// Plot this plot using the provided dashboard, panel, query engine, window, and output
    /// stream.
    pub fn plot<W: Write>(
        &self,
        dash: &Dashboard,
        panel: &Panel,
        qe: &QueryEngine,
        window: Window,
        out: &mut W,
    ) -> Result<(), Error> {
        let label_width = panel.label_width.unwrap_or(dash.label_width);
        let graph_width = panel.graph_width.unwrap_or(dash.graph_width);
        let series = qe.query(&self.query, window, graph_width)?;
        let once = series.len() == 1;
        if !once {
            writeln!(out, "{:>label_width$}", self.label)?;
        }
        for series in series {
            let label = if once {
                self.label.clone()
            } else {
                series.label_with_tags()
            };
            if series.series.is_empty() {
                writeln!(out, "{label:>label_width$} no data")?;
                continue;
            }
            let y_min = self.y_min.unwrap_or(0.0);
            let y_max = self.y_max.unwrap_or(
                series
                    .series
                    .points
                    .iter()
                    .map(|(_, p)| *p)
                    .max_by(f64::total_cmp)
                    .unwrap(),
            );
            let mut graph = String::new();
            for point in series.series.points.iter() {
                let c = if point.1 < y_min {
                    '_'
                } else if point.1 > y_max {
                    '▇'
                } else {
                    let point = (point.1 - y_min) / (y_max - y_min);
                    STEPS[((STEPS.len() - 1) as f64 * point).round() as usize]
                };
                graph.push(c);
            }
            writeln!(out, "{label:>label_width$} {graph:>graph_width$}")?;
        }
        Ok(())
    }
}

/////////////////////////////////////////////// Panel //////////////////////////////////////////////

/// Panel captures a set of related plots.
pub struct Panel {
    label: String,
    plots: Vec<Plot>,
    label_width: Option<usize>,
    graph_width: Option<usize>,
}

impl Panel {
    /// Create a new panel with the provided label.
    pub fn new<S: AsRef<str>>(label: S) -> Self {
        Self {
            label: label.as_ref().to_string(),
            plots: vec![],
            label_width: None,
            graph_width: None,
        }
    }

    /// Set the label width for this panel.
    pub fn with_label_width(mut self, label_width: usize) -> Self {
        self.label_width = Some(label_width);
        self
    }

    /// Set the graph width for this panel.
    pub fn with_graph_width(mut self, graph_width: usize) -> Self {
        self.graph_width = Some(graph_width);
        self
    }

    /// Add a plot to this panel.
    pub fn with_plot(mut self, plot: Plot) -> Self {
        self.plots.push(plot);
        self
    }

    /// Plot this panel using the provided dashboard, query engine, window, and output stream.
    pub fn plot<W: Write>(
        &self,
        dash: &Dashboard,
        qe: &QueryEngine,
        window: Window,
        out: &mut W,
    ) -> Result<(), Error> {
        let label_width = self.label_width.unwrap_or(dash.label_width);
        let graph_width = self.graph_width.unwrap_or(dash.graph_width);
        let width = label_width + graph_width;
        writeln!(out, "{:^width$}", self.label)?;
        writeln!(out, "{:^width$}", fill('─', self.label.len()))?;
        for plot in self.plots.iter() {
            plot.plot(dash, self, qe, window, out)?;
        }
        Ok(())
    }
}

///////////////////////////////////////////// Dashboard ////////////////////////////////////////////

/// Dashboard is the entry-point into visualizations in saros.  All visualizations descend from a
/// dashboard.
pub struct Dashboard {
    label: String,
    panels: Vec<Panel>,
    label_width: usize,
    graph_width: usize,
}

impl Dashboard {
    /// Create a new dashboard with the provided label.
    pub fn new<S: AsRef<str>>(label: S) -> Self {
        Self {
            label: label.as_ref().to_string(),
            panels: vec![],
            label_width: 20,
            graph_width: 80,
        }
    }

    /// Add a panel to this dashboard.
    pub fn with_panel(mut self, panel: Panel) -> Self {
        self.panels.push(panel);
        self
    }

    /// The the width of labels on this dashboard.  May be overridden on a panel-by-panel basis.
    pub fn with_label_width(mut self, label_width: usize) -> Self {
        self.label_width = label_width;
        self
    }

    /// The the width of graphs on this dashboard.  May be overridden on a panel-by-panel basis.
    pub fn with_graph_width(mut self, graph_width: usize) -> Self {
        self.graph_width = graph_width;
        self
    }

    /// Plot the dashboard using the provided query engine, window, and output stream.
    pub fn plot<W: Write>(
        &self,
        qe: &QueryEngine,
        window: Window,
        out: &mut W,
    ) -> Result<(), Error> {
        let width = self.label_width + self.graph_width;
        writeln!(out, "{:^width$}", self.label)?;
        writeln!(out, "{:^width$}", fill('━', self.label.len()))?;
        for (idx, panel) in self.panels.iter().enumerate() {
            if idx > 0 {
                writeln!(out)?;
            }
            panel.plot(self, qe, window, out)?;
        }
        Ok(())
    }
}

/////////////////////////////////////////// RewindWriter ///////////////////////////////////////////

/// RewindWriter tracks the number of newlines written to the writer that it wraps and clears those
/// lines upon a call to rewind.
pub struct RewindWriter<W: Write> {
    write: W,
    count: usize,
}

impl<W: Write> RewindWriter<W> {
    /// Rewind the terminal, erasing lines as it goes.
    pub fn rewind(&mut self) -> Result<(), std::io::Error> {
        while self.count > 0 {
            let esc = 27 as char;
            write!(self.write, "{esc}[2K\r{esc}[1A")?;
            self.count -= 1;
        }
        Ok(())
    }
}

impl<W: Write> From<W> for RewindWriter<W> {
    fn from(write: W) -> Self {
        Self { write, count: 0 }
    }
}

impl<W: Write> Write for RewindWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.count += buf.iter().filter(|b| **b == b'\n').count();
        self.write.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.write.flush()
    }
}

/////////////////////////////////////////////// utils //////////////////////////////////////////////

fn fill(c: char, sz: usize) -> String {
    [c].iter()
        .cycle()
        .take(sz)
        .copied()
        .map(String::from)
        .collect::<Vec<_>>()
        .join("")
}
