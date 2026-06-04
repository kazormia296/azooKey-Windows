use std::fmt::Write as _;
use std::sync::OnceLock;
use tracing::field::{Field, Visit};
use tracing_core::LevelFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use windows::{core::PCWSTR, Win32::System::Diagnostics::Debug::OutputDebugStringW};

use crate::extension::StringExt as _;

struct StringVisitor<'a> {
    string: &'a mut String,
}

impl<'a> Visit for StringVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            write!(self.string, "{:?}", value).unwrap();
        }
    }
}

struct DebugOutputLayer;

impl<S> Layer<S> for DebugOutputLayer
where
    S: tracing::Subscriber,
    S: for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level().as_str();
        let file = metadata.file().unwrap_or("<unknown>");
        let line = metadata.line().unwrap_or(0);

        let mut message = String::new();
        event.record(&mut StringVisitor {
            string: &mut message,
        });

        let str = format!(
            "[AzooKey-Windows] [{}: {}:{}] {}",
            level, file, line, message
        );
        let wide: Vec<u16> = str.as_str().to_wide_16();
        unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
    }
}

static LOGGER_INIT: OnceLock<Result<(), ()>> = OnceLock::new();

pub fn setup_logger() {
    #[cfg(not(debug_assertions))]
    {
        return;
    }

    LOGGER_INIT.get_or_init(|| {
        let filter = Targets::new()
            .with_target("tsf_core", LevelFilter::DEBUG)
            .with_default(LevelFilter::OFF);

        tracing_subscriber::registry()
            .with(filter)
            .with(DebugOutputLayer)
            .try_init()
            .map_err(|_| ())
    });
}
