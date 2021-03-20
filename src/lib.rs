use std::sync::Arc;
use std::thread::{self, JoinHandle};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::subscriber::Subscriber;
use tracing::{Event, Id};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

pub struct PbSubscriber {
    progress_style: ProgressStyle,
    description_key: String,
    steady_tick: u64,
    tick_all_spans: bool,
    mp: Arc<MultiProgress>,
    th: Option<JoinHandle<()>>, // thread to join `mp`
}

impl Default for PbSubscriber {
    fn default() -> Self {
        Self::new(
            ProgressStyle::default_spinner()
                .tick_strings(&["▹▹▹▹▹", "▸▹▹▹▹", "▹▸▹▹▹", "▹▹▸▹▹", "▹▹▹▸▹", "▹▹▹▹▸", "▪▪▪▪▪"])
                .template("{spinner:.blue} {msg}"),
            "",
            120,
        )
    }
}

impl Drop for PbSubscriber {
    fn drop(&mut self) {
        if let Some(th) = self.th.take() {
            let _ = th.join();
        }
    }
}

fn tick_all_spans<S: Subscriber + for<'a> LookupSpan<'a>>(ctx: &Context<'_, S>) {
    for span in ctx.scope() {
        if let Some(pb) = span.extensions().get::<ProgressBar>() {
            pb.tick();
        }
    }
}

impl<S> Layer<S> for PbSubscriber
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        struct DescriptionExtractor<'a> {
            key: &'a str,
            value: String,
        }

        impl<'a> Visit for DescriptionExtractor<'a> {
            fn record_debug(&mut self, _: &Field, _: &dyn ::std::fmt::Debug) {}

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == self.key {
                    self.value = value.to_string();
                }
            }
        }

        impl<'a> DescriptionExtractor<'a> {
            fn new(key: &'a str) -> Self {
                Self { key, value: Default::default() }
            }
        }

        if let Some(span) = ctx.span(id) {
            let mut v = DescriptionExtractor::new(&self.description_key);
            let description = if self.description_key.is_empty() {
                span.metadata().name()
            } else {
                attrs.record(&mut v);

                if v.value.is_empty() {
                    span.metadata().name()
                } else {
                    v.value.as_str()
                }
            };
            if !description.is_empty() {
                let indent = "-".repeat(span.parents().count());
                let message = format!("|{} {}", indent, description);

                span.extensions_mut().insert(message);
            }
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let pb = span.extensions().get::<String>().map(|s| {
                let pb = self.mp.add(ProgressBar::new_spinner());
                pb.set_style(self.progress_style.clone());
                pb.set_message(s.as_str());

                pb.enable_steady_tick(self.steady_tick);
                pb
            });

            if let Some(pb) = pb {
                span.extensions_mut().insert(pb);
            }
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let message = span.extensions().get::<String>().map(|s| format!("{} done!", s));

            if let Some(message) = message {
                if let Some(pb) = span.extensions_mut().remove::<ProgressBar>() {
                    pb.disable_steady_tick();

                    let depth = span.parents().count();
                    if depth > 0 {
                        pb.finish_and_clear();
                    } else {
                        pb.finish_with_message(&message);
                    }
                }
            }
        }
        if self.tick_all_spans {
            tick_all_spans(&ctx);
        }
    }

    fn on_event(&self, _: &Event<'_>, ctx: Context<'_, S>) {
        if self.tick_all_spans {
            tick_all_spans(&ctx);
        }
    }
}

impl PbSubscriber {
    pub fn new(
        progress_style: ProgressStyle,
        description_key: impl Into<String>,
        steady_tick: u64,
    ) -> Self {
        let mp = Arc::new(MultiProgress::new());
        let mp2 = Arc::clone(&mp);
        let th = Some(thread::spawn(move || {
            // hidden progress bar to prevent `mp2` being joined early
            let _dummy = mp2.add(ProgressBar::hidden());
            let _ = mp2.join();
        }));
        PbSubscriber {
            th,
            mp,
            progress_style,
            description_key: description_key.into(),
            steady_tick,
            tick_all_spans: false,
        }
    }

    pub fn with_steady_tick(mut self, n: u64) -> Self {
        self.steady_tick = n;
        self
    }

    pub fn with_description_key(mut self, s: String) -> Self {
        self.description_key = s;
        self
    }
}
