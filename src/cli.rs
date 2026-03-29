use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "CloudPrinter helper for quoting and submitting book orders"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// List products enabled for the current CloudPrinter account.
    Products(GlobalApiArgs),
    /// Fetch detailed information for a single CloudPrinter product.
    ProductInfo(ProductInfoArgs),
    /// Request a quote including shipping options.
    Quote(QuoteArgs),
    /// Submit an order from a saved quote hash.
    Submit(SubmitArgs),
}

#[derive(Args, Debug)]
pub(crate) struct GlobalApiArgs {
    /// Override the CloudPrinter API base URL.
    #[arg(long)]
    pub(crate) api_base_url: Option<String>,

    /// Print the raw JSON response.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ProductInfoArgs {
    #[command(flatten)]
    pub(crate) api: GlobalApiArgs,

    /// CloudPrinter product reference, for example `textbook_pb_a4_p_bw`.
    #[arg(long)]
    pub(crate) product: String,
}

#[derive(Args, Debug)]
pub(crate) struct QuoteOrderArgs {
    /// Book/product configuration in TOML format.
    #[arg(long, default_value = "orders/btcwip-example.toml")]
    pub(crate) template: PathBuf,

    /// Delivery address in TOML format.
    #[arg(long)]
    pub(crate) address: PathBuf,

    /// Order reference visible in CloudPrinter.
    #[arg(long)]
    pub(crate) reference: String,

    /// Number of books to order.
    #[arg(long)]
    pub(crate) count: u32,

    /// GitHub release tag to source `cover` and `book` PDFs from.
    #[arg(long, conflicts_with = "latest")]
    pub(crate) version: Option<String>,

    /// Use the latest GitHub release assets for the `cover` and `book` PDFs.
    #[arg(long, conflicts_with = "version")]
    pub(crate) latest: bool,
}

#[derive(Args, Debug)]
pub(crate) struct QuoteArgs {
    #[command(flatten)]
    pub(crate) shared: QuoteOrderArgs,

    /// Print the raw JSON response.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct SubmitArgs {
    /// Quote hash returned by `quote`.
    #[arg(long)]
    pub(crate) quote_hash: String,

    /// Only print the request JSON instead of posting it.
    #[arg(long)]
    pub(crate) dry_run: bool,

    /// Print the raw JSON response or request payload.
    #[arg(long)]
    pub(crate) json: bool,
}
