use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use colored::Colorize;
use forge_domain::{ConsoleWriter, Usage};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use rand::RngExt;

mod progress_bar;

pub use progress_bar::*;

const TICK_DURATION_MS: u64 = 60;
const TICKS: &[&str; 10] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Formats elapsed time into a compact string representation.
///
/// Returns a string like "01s", "1:01m", or "1:01h".
fn format_elapsed_time(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    if total_seconds < 60 {
        format!("{:02}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}:{:02}m", minutes, seconds)
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}:{:02}h", hours, minutes)
    }
}

/// Formats a usize with thousands separators using commas (e.g. 12450 →
/// "12,450").
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Builds the spinner prefix containing live token stats in parentheses.
///
/// Format when stats are available: `(↑12,450  ↓234 31% 47t/s) · Ctrl+C to
/// interrupt`
///
/// Cache percentage is omitted when zero. Tokens per second (`t/s`) is
/// calculated from `(token_delta, elapsed)` when provided, giving real-time
/// estimated throughput based on when usage updates arrive.
fn format_stats_prefix(
    prompt: usize,
    completion: usize,
    cached: usize,
    tps_data: Option<(usize, Duration)>,
) -> String {
    let cache_pct = if prompt > 0 {
        (cached as f64 / prompt as f64 * 100.0) as usize
    } else {
        0
    };

    let cache_part = if cache_pct > 0 {
        format!(" {}%", cache_pct)
    } else {
        String::new()
    };

    let tps_part = match tps_data {
        Some((token_delta, elapsed)) if token_delta > 0 && elapsed.as_secs_f64() > 0.0 => {
            let tps = token_delta as f64 / elapsed.as_secs_f64();
            format!(" {:.0}t/s", tps)
        }
        _ => String::new(),
    };

    format!(
        "(↑{}  ↓{}{}{}) · Ctrl+C to interrupt",
        format_number(prompt),
        format_number(completion),
        cache_part,
        tps_part,
    )
}

/// Manages spinner functionality for the UI.
///
/// Uses indicatif's built-in `{elapsed}` template for time display,
/// eliminating the need for a background task to update the message.
/// Accumulated time is preserved across start/stop cycles using
/// `with_elapsed()`. Spinner tick position is also preserved to maintain
/// smooth animation continuity.
pub struct SpinnerManager<P: ConsoleWriter> {
    spinner: Option<ProgressBar>,
    accumulated_elapsed: Duration,
    word_index: Option<usize>,
    message: Option<String>,
    /// Cached stats prefix set by `update_usage`, persisted across start/stop
    /// cycles.
    stats_prefix: Option<String>,
    /// Last known input token count, preserved across usage updates.
    /// This ensures we don't show 0 when estimated usage arrives before
    /// provider-sent input token count.
    last_known_prompt_tokens: usize,
    /// Last known cache token count, preserved across usage updates.
    last_known_cached_tokens: usize,
    /// Tracks when the first estimated usage arrived for tps calculation.
    /// tps is calculated as (current_tokens - first_tokens) /
    /// elapsed_since_first.
    first_usage_instant: Option<Instant>,
    /// Token count from the first usage update (for tps delta calculation).
    first_usage_tokens: usize,
    printer: Arc<P>,
}

impl<P: ConsoleWriter> SpinnerManager<P> {
    /// Creates a new SpinnerManager with the given output printer.
    pub fn new(printer: Arc<P>) -> Self {
        Self {
            spinner: None,
            accumulated_elapsed: Duration::ZERO,
            word_index: None,
            message: None,
            stats_prefix: None,
            last_known_prompt_tokens: 0,
            last_known_cached_tokens: 0,
            first_usage_instant: None,
            first_usage_tokens: 0,
            printer,
        }
    }

    /// Start the spinner with a message
    pub fn start(&mut self, message: Option<&str>) -> Result<()> {
        self.stop(None)?;

        let words = [
            "Thinking",
            "Processing",
            "Analyzing",
            "Forging",
            "Researching",
            "Synthesizing",
            "Reasoning",
            "Contemplating",
        ];

        // Priority: explicit message > random word (stats live in the prefix, not the
        // message)
        let word = match message {
            Some(msg) => msg.to_string(),
            None => {
                let idx = *self
                    .word_index
                    .get_or_insert_with(|| rand::rng().random_range(0..words.len()));
                words[idx].to_string()
            }
        };

        self.message = Some(word.clone());

        // Build the prefix: cached stats (with Ctrl+C) or plain Ctrl+C hint
        let prefix = self
            .stats_prefix
            .clone()
            .unwrap_or_else(|| "· Ctrl+C to interrupt".to_string());

        // Create the spinner with accumulated elapsed time
        // Use custom elapsed formatter for "01s", "1:01m", "1:01h" format
        let pb = ProgressBar::new_spinner()
            .with_elapsed(self.accumulated_elapsed)
            .with_style(
                ProgressStyle::default_spinner()
                    .tick_strings(TICKS)
                    .template("{spinner:.green} {msg} {elapsed_custom:.white} {prefix:.white.dim}")
                    .unwrap()
                    .with_key(
                        "elapsed_custom",
                        |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                            let _ = write!(w, "{}", format_elapsed_time(state.elapsed()));
                        },
                    ),
            )
            .with_message(word.green().bold().to_string())
            .with_prefix(prefix);

        // Preserve spinner tick position for visual continuity
        // The spinner has 10 tick positions cycling every 600ms (60ms per tick)
        let tick_count: usize = TICKS.len();
        let elapsed_ms = self.accumulated_elapsed.as_millis() as u64;
        let cycle_ms = TICK_DURATION_MS * tick_count as u64;
        let ticks_to_advance = (elapsed_ms % cycle_ms) / TICK_DURATION_MS;

        // Advance to the correct tick position
        (0..ticks_to_advance).for_each(|_| pb.tick());

        pb.enable_steady_tick(Duration::from_millis(TICK_DURATION_MS));

        self.spinner = Some(pb);

        Ok(())
    }

    /// Stop the active spinner if any
    pub fn stop(&mut self, message: Option<String>) -> Result<()> {
        if let Some(spinner) = self.spinner.take() {
            // Capture elapsed time before finishing
            self.accumulated_elapsed = spinner.elapsed();
            spinner.finish_and_clear();
            if let Some(msg) = message {
                self.println(&msg);
            }
        } else if let Some(message) = message {
            self.println(&message);
        }

        self.message = None;

        Ok(())
    }

    /// Updates the spinner's displayed message.
    pub fn set_message(&mut self, message: &str) -> Result<()> {
        self.message = Some(message.to_owned());
        if let Some(spinner) = &self.spinner {
            spinner.set_message(message.green().bold().to_string());
        }
        Ok(())
    }

    /// Resets the elapsed time to zero.
    /// Call this when starting a completely new task/conversation.
    pub fn reset(&mut self) {
        self.accumulated_elapsed = Duration::ZERO;
        self.word_index = None;
        self.message = None;
        self.stats_prefix = None;
        self.last_known_prompt_tokens = 0;
        self.last_known_cached_tokens = 0;
        self.first_usage_instant = None;
        self.first_usage_tokens = 0;
    }

    /// Updates the spinner prefix with live token usage statistics.
    ///
    /// Stats are shown in the `{prefix}` slot so the random word in `{msg}` is
    /// never overwritten. Format: `(↑12,450  ↓234 31% 47t/s) · Ctrl+C to
    /// interrupt`
    ///
    /// Tokens per second is calculated based on when usage updates arrive.
    /// On the first update, we record the instant and token count. On
    /// subsequent updates, tps = (current_tokens - first_tokens) /
    /// elapsed_since_first. This gives real-time estimated tps regardless
    /// of whether the usage is from the provider or estimated from content.
    pub fn update_usage(&mut self, usage: &Usage) -> Result<()> {
        let prompt = *usage.prompt_tokens;
        let completion = *usage.completion_tokens;
        let cached = *usage.cached_tokens;

        if prompt == 0 && completion == 0 {
            return Ok(());
        }

        // Update last known values when we receive non-zero data from provider
        // This preserves input tokens and cache hits across estimated updates
        if prompt > 0 {
            self.last_known_prompt_tokens = prompt;
        }
        if cached > 0 {
            self.last_known_cached_tokens = cached;
        }

        // Use last known values for display if current usage has zeros
        // (estimated usage may not have input/cache data yet)
        let display_prompt = if prompt > 0 {
            prompt
        } else {
            self.last_known_prompt_tokens
        };
        let display_cached = if cached > 0 {
            cached
        } else {
            self.last_known_cached_tokens
        };

        // Calculate tps based on when usage updates arrive
        let tps_data = if completion > 0 {
            if let Some(instant) = self.first_usage_instant {
                // Subsequent updates - calculate tps from delta
                let elapsed = instant.elapsed();
                let token_delta = completion.saturating_sub(self.first_usage_tokens);
                if elapsed > Duration::ZERO && token_delta > 0 {
                    Some((token_delta, elapsed))
                } else {
                    None
                }
            } else {
                // First usage update - record baseline
                self.first_usage_instant = Some(Instant::now());
                self.first_usage_tokens = completion;
                // First update, no tps yet
                None
            }
        } else {
            None
        };

        let prefix = format_stats_prefix(display_prompt, completion, display_cached, tps_data);
        self.stats_prefix = Some(prefix.clone());
        if let Some(pb) = &self.spinner {
            pb.set_prefix(prefix);
        }
        Ok(())
    }

    /// Writes a line to stdout, suspending the spinner if active.
    pub fn write_ln(&mut self, message: impl ToString) -> Result<()> {
        let msg = message.to_string();
        if let Some(spinner) = &self.spinner {
            spinner.suspend(|| self.println(&msg));
        } else {
            self.println(&msg);
        }
        Ok(())
    }

    /// Writes a line to stderr, suspending the spinner if active.
    pub fn ewrite_ln(&mut self, message: impl ToString) -> Result<()> {
        let msg = message.to_string();
        if let Some(spinner) = &self.spinner {
            spinner.suspend(|| self.eprintln(&msg));
        } else {
            self.eprintln(&msg);
        }
        Ok(())
    }

    /// Prints a line to stdout through the printer.
    fn println(&self, msg: &str) {
        let line = format!("{msg}\n");
        let _ = self.printer.write(line.as_bytes());
        let _ = self.printer.flush();
    }

    /// Prints a line to stderr through the printer.
    fn eprintln(&self, msg: &str) {
        let line = format!("{msg}\n");
        let _ = self.printer.write_err(line.as_bytes());
        let _ = self.printer.flush_err();
    }
}

impl<P: ConsoleWriter> Drop for SpinnerManager<P> {
    fn drop(&mut self) {
        // Stop spinner before flushing to ensure finish_and_clear() is called
        // This prevents the spinner from leaving the cursor at column 0 without a
        // newline
        let _ = self.stop(None);
        // Flush both stdout and stderr to ensure all output is visible
        // This prevents race conditions with shell prompt resets
        let _ = self.printer.flush();
        let _ = self.printer.flush_err();
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;
    use std::time::Duration;

    use forge_domain::ConsoleWriter;
    use pretty_assertions::assert_eq;

    use super::{SpinnerManager, format_elapsed_time};

    /// A simple printer that writes directly to stdout/stderr.
    /// Used for testing when synchronized output is not needed.
    #[derive(Clone, Copy)]
    struct DirectPrinter;

    impl ConsoleWriter for DirectPrinter {
        fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
            std::io::stdout().write(buf)
        }

        fn write_err(&self, buf: &[u8]) -> std::io::Result<usize> {
            std::io::stderr().write(buf)
        }

        fn flush(&self) -> std::io::Result<()> {
            std::io::stdout().flush()
        }

        fn flush_err(&self) -> std::io::Result<()> {
            std::io::stderr().flush()
        }
    }

    fn fixture_spinner() -> SpinnerManager<DirectPrinter> {
        SpinnerManager::new(Arc::new(DirectPrinter))
    }

    #[test]
    fn test_spinner_reset_clears_accumulated_time() {
        let mut fixture_spinner = fixture_spinner();

        // Simulate some accumulated time
        fixture_spinner.accumulated_elapsed = std::time::Duration::from_secs(100);

        // Reset should clear accumulated time
        fixture_spinner.reset();

        let actual = fixture_spinner.accumulated_elapsed;
        let expected = std::time::Duration::ZERO;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_spinner_reset_clears_word_index() {
        let mut fixture_spinner = fixture_spinner();

        // Set a word index
        fixture_spinner.word_index = Some(3);

        // Reset should clear it
        fixture_spinner.reset();

        let actual = fixture_spinner.word_index;
        let expected = None;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_spinner_reset_clears_message() {
        let mut fixture_spinner = fixture_spinner();

        // Set a message
        fixture_spinner.message = Some("Test".to_string());

        // Reset should clear it
        fixture_spinner.reset();

        let actual = fixture_spinner.message.clone();
        let expected = None;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_word_index_caching_behavior() {
        let mut fixture_spinner = fixture_spinner();

        // Start spinner without message multiple times
        fixture_spinner.start(None).unwrap();
        let first_index = fixture_spinner.word_index;
        fixture_spinner.stop(None).unwrap();

        fixture_spinner.start(None).unwrap();
        let second_index = fixture_spinner.word_index;
        fixture_spinner.stop(None).unwrap();

        // Word index should be identical because it's cached
        assert_eq!(first_index, second_index);
    }

    #[test]
    fn test_format_elapsed_time_seconds_only() {
        let actual = format_elapsed_time(Duration::from_secs(5));
        let expected = "05s";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(59));
        let expected = "59s";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_elapsed_time_minutes_and_seconds() {
        let actual = format_elapsed_time(Duration::from_secs(60));
        let expected = "1:00m";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(125));
        let expected = "2:05m";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(3599));
        let expected = "59:59m";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_elapsed_time_hours_and_minutes() {
        let actual = format_elapsed_time(Duration::from_secs(3600));
        let expected = "1:00h";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(3661));
        let expected = "1:01h";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(7200));
        let expected = "2:00h";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(9000));
        let expected = "2:30h";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_elapsed_time_zero() {
        let actual = format_elapsed_time(Duration::ZERO);
        let expected = "00s";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_stats_prefix_with_tps() {
        // Test with tps data - should show t/s
        let actual =
            super::format_stats_prefix(1000, 500, 100, Some((250, Duration::from_secs(5))));
        // tps = 250 / 5 = 50 t/s
        assert!(
            actual.contains("50t/s"),
            "Expected tps in output, got: {}",
            actual
        );
    }

    #[test]
    fn test_format_stats_prefix_without_tps() {
        // Test without tps data - should not show t/s
        let actual = super::format_stats_prefix(1000, 500, 100, None);
        assert!(
            !actual.contains("t/s"),
            "Should not show t/s when no tps data: {}",
            actual
        );
    }

    #[test]
    fn test_update_usage_calculates_tps_on_second_call() {
        use forge_domain::{TokenCount, Usage};

        let printer = Arc::new(DirectPrinter);
        let mut spinner = SpinnerManager::new(printer);

        // First update - records baseline, no tps yet
        let usage1 = Usage {
            prompt_tokens: TokenCount::Actual(100),
            completion_tokens: TokenCount::Actual(50),
            total_tokens: TokenCount::Actual(150),
            cached_tokens: TokenCount::Actual(0),
            cost: None,
        };
        spinner.update_usage(&usage1).unwrap();

        // stats_prefix should be set but without t/s
        let prefix1 = spinner.stats_prefix.clone().unwrap();
        assert!(
            !prefix1.contains("t/s"),
            "First update should not show t/s: {}",
            prefix1
        );

        // Wait a tiny bit and do second update
        std::thread::sleep(Duration::from_millis(100));

        let usage2 = Usage {
            prompt_tokens: TokenCount::Actual(100),
            completion_tokens: TokenCount::Actual(100), // 50 more tokens
            total_tokens: TokenCount::Actual(200),
            cached_tokens: TokenCount::Actual(0),
            cost: None,
        };
        spinner.update_usage(&usage2).unwrap();

        // Now should have t/s
        let prefix2 = spinner.stats_prefix.clone().unwrap();
        assert!(
            prefix2.contains("t/s"),
            "Second update should show t/s: {}",
            prefix2
        );
    }
}
