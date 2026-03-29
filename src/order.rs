use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::cli::{QuoteOrderArgs, SubmitArgs};
use crate::cloudprinter::{
    AddOrderItemRequest, AddOrderRequest, CloudPrinterClient, QuoteItemRequest, QuoteRequest,
    QuoteResponse,
};
use crate::config::{Address, BookConfig, absolutize, load_toml};
use crate::github::{GithubClient, ReleaseRequest, ResolvedSubmitFiles};

#[derive(Clone, Debug)]
pub(crate) struct PreparedOrder {
    pub(crate) client: CloudPrinterClient,
    pub(crate) book: BookConfig,
    pub(crate) address: Address,
    pub(crate) template: PathBuf,
    pub(crate) address_source: PathBuf,
    pub(crate) reference: String,
    pub(crate) count: u32,
    pub(crate) release_selector: ReleaseSelector,
}

impl PreparedOrder {
    pub(crate) fn load(args: &QuoteOrderArgs) -> Result<Self> {
        Self::load_parts(
            &args.template,
            &args.address,
            &args.reference,
            args.count,
            ReleaseSelector::from_quote_args(args),
        )
    }

    pub(crate) fn load_for_submit(args: &SubmitArgs) -> Result<Self> {
        let pending = load_pending_quote(&args.quote_hash)?;

        Self::load_parts(
            &pending.template,
            &pending.address,
            &pending.reference,
            pending.count,
            pending.release_selector,
        )
    }

    fn load_parts(
        template: &Path,
        address: &Path,
        reference: &str,
        count: u32,
        release_selector: ReleaseSelector,
    ) -> Result<Self> {
        let book = load_toml::<BookConfig>(template)
            .with_context(|| format!("failed to load book config from {}", template.display()))?;
        let address_data = load_toml::<Address>(address)
            .with_context(|| format!("failed to load address config from {}", address.display()))?;

        let base_dir = template
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let client = crate::cloudprinter_client_from_env(book.api_base_url.clone())?;
        let book = book.resolve_relative_files(&base_dir);

        Ok(Self {
            client,
            book,
            address: address_data,
            template: template.to_path_buf(),
            address_source: address.to_path_buf(),
            reference: reference.to_string(),
            count,
            release_selector,
        })
    }

    pub(crate) fn quote_request(&self) -> QuoteRequest {
        QuoteRequest {
            apikey: self.client.api_key().to_string(),
            currency: self.book.currency.clone(),
            country: self.address.country.clone(),
            state: self.address.state.clone(),
            items: vec![QuoteItemRequest {
                reference: self.item_reference(),
                product: self.book.item.product.clone(),
                count: self.count.to_string(),
                options: self.book.item.options.clone(),
            }],
        }
    }

    pub(crate) fn submit_request(&self, quote_hash: &str) -> Result<AddOrderRequest> {
        let email = self
            .address
            .order_email
            .clone()
            .ok_or_else(|| anyhow!("submit requires address.order_email"))?;
        let files = self.resolve_submit_files()?.files;

        Ok(AddOrderRequest {
            apikey: self.client.api_key().to_string(),
            reference: self.reference.clone(),
            email,
            addresses: vec![self.address.clone().into_delivery_address()],
            items: vec![AddOrderItemRequest {
                reference: self.item_reference(),
                product: self.book.item.product.clone(),
                quote: Some(quote_hash.to_string()),
                title: self.book.item.title.clone(),
                count: self.count.to_string(),
                price: self.book.item.price.clone(),
                currency: self.book.item.currency.clone(),
                hc: self.book.item.harmonized_code.clone(),
                files,
                options: self.book.item.options.clone(),
            }],
        })
    }

    pub(crate) fn resolve_submit_files(&self) -> Result<ResolvedSubmitFiles> {
        if let Some(request) = self.release_selector.requested_release() {
            return GithubClient::new(self.client.http())
                .resolve_release_files(&self.book.github_release, request);
        }

        self.book.validate_submit_prerequisites()?;
        Ok(ResolvedSubmitFiles {
            release_tag: None,
            files: self.book.item.files.clone(),
        })
    }

    fn item_reference(&self) -> String {
        format!("{}-1", self.reference)
    }

    fn template_path(&self) -> Result<PathBuf> {
        absolutize(&self.template)
    }

    fn address_path(&self) -> Result<PathBuf> {
        absolutize(&self.address_source)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReleaseSelector {
    pub(crate) version: Option<String>,
    pub(crate) latest: bool,
}

impl ReleaseSelector {
    pub(crate) fn from_quote_args(args: &QuoteOrderArgs) -> Self {
        Self {
            version: args.version.clone(),
            latest: args.latest,
        }
    }

    pub(crate) fn requested_release(&self) -> Option<ReleaseRequest<'_>> {
        if self.latest {
            Some(ReleaseRequest::Latest)
        } else {
            self.version.as_deref().map(ReleaseRequest::Tag)
        }
    }
}

fn pending_dir() -> PathBuf {
    PathBuf::from("pending")
}

fn pending_quote_path(quote_hash: &str) -> PathBuf {
    pending_dir().join(format!("{quote_hash}.toml"))
}

pub(crate) fn save_pending_quotes(order: &PreparedOrder, response: &QuoteResponse) -> Result<()> {
    fs::create_dir_all(pending_dir())?;
    for shipment in &response.shipments {
        for quote in &shipment.quotes {
            let pending = PendingQuote {
                template: order.template_path()?,
                address: order.address_path()?,
                reference: order.reference.clone(),
                count: order.count,
                product: order.book.item.product.clone(),
                quote_hash: quote.quote.clone(),
                shipping_level: quote.shipping_level.clone(),
                shipping_option: quote.shipping_option.clone(),
                shipping_price: quote.price.clone(),
                currency: quote.currency.clone(),
                release_selector: order.release_selector.clone(),
            };
            fs::write(
                pending_quote_path(&quote.quote),
                toml::to_string_pretty(&pending)?,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn load_pending_quote(quote_hash: &str) -> Result<PendingQuote> {
    let path = pending_quote_path(quote_hash);
    load_toml::<PendingQuote>(&path)
        .with_context(|| format!("failed to load pending quote from {}", path.display()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingQuote {
    pub(crate) template: PathBuf,
    pub(crate) address: PathBuf,
    pub(crate) reference: String,
    pub(crate) count: u32,
    pub(crate) product: String,
    pub(crate) quote_hash: String,
    pub(crate) shipping_level: String,
    pub(crate) shipping_option: String,
    pub(crate) shipping_price: String,
    pub(crate) currency: String,
    pub(crate) release_selector: ReleaseSelector,
}
