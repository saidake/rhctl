/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * SSH operation utils.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain::cmd_params::ServerMetadata;
use crate::domain::constants::REMOTE_CR_PROGRESS_INTERVAL_SECS;
use crate::log_remote;

/// Collapse embedded `\r` updates to the latest segment (curl-style progress).
pub fn collapse_cr(line: &str) -> &str {
    let mut line_clean = line.trim_matches('\r');
    if let Some(pos) = line_clean.rfind('\r') {
        line_clean = &line_clean[pos + 1..];
    }
    line_clean
}

pub fn execution_print(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    line: &str,
) -> Result<(), String> {
    let line_clean = collapse_cr(line);
    if line_clean.is_empty() {
        return Ok(());
    }
    log_remote!(server_metadata, task_name, "{}", line_clean);
    Ok(())
}

/// Append-only `\r` progress snapshots: print at most once per interval; flush on end.
pub struct CrProgressPrinter {
    pending: Option<String>,
    last_printed: Option<String>,
    last_print_at: Option<Instant>,
}

impl CrProgressPrinter {
    pub fn new() -> Self {
        Self {
            pending: None,
            last_printed: None,
            last_print_at: None,
        }
    }

    /// New snapshot from a `\r`-terminated segment.
    pub fn on_cr(
        &mut self,
        segment: &str,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
    ) -> Result<(), String> {
        let text = collapse_cr(segment);
        if text.is_empty() {
            return Ok(());
        }
        self.pending = Some(text.to_string());
        self.emit(false, server_metadata, task_name)
    }

    /// Newline-terminated line: flush any pending `\r` snapshot, then print the line.
    pub fn on_lf(
        &mut self,
        segment: &str,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
    ) -> Result<(), String> {
        self.flush(server_metadata, task_name)?;
        let text = collapse_cr(segment);
        if text.is_empty() {
            return Ok(());
        }
        if self.last_printed.as_deref() == Some(text) {
            return Ok(());
        }
        execution_print(server_metadata, task_name, text)?;
        self.last_printed = Some(text.to_string());
        Ok(())
    }

    /// End of stream / end of `\r` sequence: print pending immediately.
    pub fn flush(
        &mut self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
    ) -> Result<(), String> {
        self.emit(true, server_metadata, task_name)
    }

    fn emit(
        &mut self,
        force: bool,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
    ) -> Result<(), String> {
        let Some(text) = self.pending.clone() else {
            return Ok(());
        };
        if self.last_printed.as_ref() == Some(&text) {
            if force {
                self.pending = None;
            }
            return Ok(());
        }

        let interval = Duration::from_secs(REMOTE_CR_PROGRESS_INTERVAL_SECS);
        let due = force
            || self
                .last_print_at
                .map(|t| t.elapsed() >= interval)
                .unwrap_or(true);
        if !due {
            return Ok(());
        }

        let line = if force {
            text.clone()
        } else {
            format!(
                "{} (next update in {}s)",
                text, REMOTE_CR_PROGRESS_INTERVAL_SECS
            )
        };
        execution_print(server_metadata, task_name, &line)?;
        self.last_printed = Some(text);
        self.last_print_at = Some(Instant::now());
        if force {
            self.pending = None;
        }
        Ok(())
    }
}
/// Feed a stream chunk; split on `\r` / `\n` and drive [CrProgressPrinter].
pub fn feed_remote_stream(
    buffer: &mut String,
    progress: &mut CrProgressPrinter,
    chunk: &str,
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    skip_sudo_blank: &mut bool,
) -> Result<(), String> {
    buffer.push_str(chunk);
    loop {
        let cr = buffer.find('\r');
        let lf = buffer.find('\n');
        let (idx, is_lf) = match (cr, lf) {
            (Some(c), Some(l)) if c <= l => (c, false),
            (Some(_), Some(l)) => (l, true),
            (Some(c), None) => (c, false),
            (None, Some(l)) => (l, true),
            (None, None) => break,
        };

        let segment = buffer[..idx].to_string();
        buffer.drain(..=idx);

        if is_lf {
            if *skip_sudo_blank && segment.trim().is_empty() {
                *skip_sudo_blank = false;
                continue;
            }
            *skip_sudo_blank = false;
            progress.on_lf(&segment, server_metadata, task_name)?;
        } else {
            progress.on_cr(&segment, server_metadata, task_name)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_cr_keeps_latest_segment() {
        assert_eq!(collapse_cr("a\rb\rc"), "c");
        assert_eq!(collapse_cr("only"), "only");
        assert_eq!(collapse_cr("\r\r"), "");
    }
}
