#![deny(rust_2018_idioms)]
#[path = "fmt/yak_shave.rs"]
mod yak_shave;

use tracing_indicatif::PbSubscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use yak_shave::YakShaver;

fn main() {
    let _ =
        tracing::subscriber::set_global_default(Registry::default().with(PbSubscriber::default()));

    let number_of_yaks = 3;
    // this creates a new event, outside of any spans.
    tracing::info!(number_of_yaks, "preparing to shave yaks");

    let shaver = YakShaver::default();
    let number_shaved = shaver.shave_all(number_of_yaks);
    tracing::info!(all_yaks_shaved = number_shaved == number_of_yaks, "yak shaving completed.");
}
