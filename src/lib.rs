mod cli;
mod cloudprinter;
mod config;
mod github;
mod order;
mod output;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::cloudprinter::CloudPrinterClient;
use crate::order::{PreparedOrder, save_pending_quotes};

const DEFAULT_API_BASE_URL: &str = "https://api.cloudprinter.com/cloudcore/1.0";

pub fn run() -> Result<()> {
    load_dotenv_from_current_dir_ancestors().ok();
    let cli = Cli::parse();

    match cli.command {
        Command::Products(cmd) => {
            let client = cloudprinter_client_from_env(cmd.api_base_url)?;
            let products = client.list_products()?;
            output::print_output(&products, cmd.json)?;
        }
        Command::ProductInfo(cmd) => {
            let client = cloudprinter_client_from_env(cmd.api.api_base_url)?;
            let product = client.product_info(&cmd.product)?;
            output::print_output(&product, cmd.api.json)?;
        }
        Command::Quote(cmd) => {
            let order = PreparedOrder::load(&cmd.shared)?;
            let resolved_files = order.resolve_submit_files()?;
            let response = order.client.quote(&order.quote_request())?;
            save_pending_quotes(&order, &response)?;
            output::print_quote_response(&order, &resolved_files, &response, cmd.json)?;
        }
        Command::Submit(cmd) => {
            let order = PreparedOrder::load_for_submit(&cmd)?;
            let request = order.submit_request(&cmd.quote_hash)?;

            if cmd.dry_run {
                output::print_output(&request, cmd.json)?;
            } else {
                let response = order.client.submit(&request)?;
                output::print_submit_response(&response, cmd.json)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn load_dotenv_from_current_dir_ancestors() -> Result<()> {
    let cwd = std::env::current_dir()?;
    for dir in cwd.ancestors() {
        let candidate = dir.join(".env");
        if candidate.is_file() {
            dotenvy::from_path_override(&candidate)?;
            return Ok(());
        }
    }

    Ok(())
}

pub(crate) fn cloudprinter_api_key_env() -> &'static str {
    if cfg!(debug_assertions) {
        "CLOUDPRINTER_API_KEY_SANDBOX"
    } else {
        "CLOUDPRINTER_API_KEY_LIVE"
    }
}

pub(crate) fn cargo_run_prefix() -> &'static str {
    if cfg!(debug_assertions) {
        "cargo run --"
    } else {
        "cargo run --release --"
    }
}

pub(crate) fn cloudprinter_client_from_env(
    api_base_url: Option<String>,
) -> Result<CloudPrinterClient> {
    let api_key_env = cloudprinter_api_key_env();
    let api_key = std::env::var(api_key_env).with_context(|| {
        format!(
            "missing {} in environment; load it via .env or export it before running",
            api_key_env
        )
    })?;
    let base_url = api_base_url
        .or_else(|| std::env::var("CLOUDPRINTER_API_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string());

    CloudPrinterClient::new(api_key, base_url)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::QuoteOrderArgs;
    use crate::cloudprinter::{AddOrderRequest, format_submit_error};
    use crate::config::{
        Address, BookConfig, BookItemConfig, GithubReleaseConfig, ItemOption, OrderFile,
    };
    use crate::github::ReleaseRequest;
    use crate::order::{PreparedOrder, ReleaseSelector};

    #[test]
    fn builds_quote_request_from_fixture_values() {
        let prepared = PreparedOrder {
            client: CloudPrinterClient::new("test-key".to_string(), "http://localhost".to_string())
                .expect("client"),
            book: BookConfig {
                api_base_url: None,
                currency: Some("EUR".to_string()),
                github_release: GithubReleaseConfig {
                    owner: "Sjors".to_string(),
                    repo: "nado-book".to_string(),
                    cover_asset_name: "nado-cover-rgb.pdf".to_string(),
                    book_asset_name: "nado-paperback.pdf".to_string(),
                },
                item: BookItemConfig {
                    product: "textbook_pb_digest_p_bw".to_string(),
                    title: Some("Bitcoin - A Work in Progress".to_string()),
                    price: None,
                    currency: None,
                    harmonized_code: None,
                    options: vec![
                        ItemOption {
                            option_type: "total_pages".to_string(),
                            count: "224".to_string(),
                        },
                        ItemOption {
                            option_type: "pageblock_90mcs".to_string(),
                            count: "224".to_string(),
                        },
                    ],
                    files: vec![],
                },
            },
            address: Address {
                company: Some("Domkerk".to_string()),
                firstname: "Domkerk".to_string(),
                lastname: "Orders".to_string(),
                street1: "Achter de Dom 1".to_string(),
                street2: None,
                zip: "3512 JN".to_string(),
                city: "Utrecht".to_string(),
                country: "NL".to_string(),
                state: None,
                order_email: Some("orders@domkerk.example".to_string()),
                delivery_email: Some("delivery@domkerk.example".to_string()),
                phone: None,
            },
            template: PathBuf::from("orders/btcwip-example.toml"),
            address_source: PathBuf::from("addresses/domkerk-utrecht.toml"),
            reference: "domkerk-21".to_string(),
            count: 21,
            release_selector: ReleaseSelector {
                version: None,
                latest: false,
            },
        };

        let request = prepared.quote_request();

        assert_eq!(request.country, "NL");
        assert_eq!(request.currency.as_deref(), Some("EUR"));
        assert_eq!(request.items.len(), 1);
        assert_eq!(request.items[0].reference, "domkerk-21-1");
        assert_eq!(request.items[0].count, "21");
        assert_eq!(request.items[0].options[0].option_type, "total_pages");
    }

    #[test]
    fn submit_requires_order_email() {
        let prepared = PreparedOrder {
            client: CloudPrinterClient::new("test-key".to_string(), "http://localhost".to_string())
                .expect("client"),
            book: BookConfig {
                api_base_url: None,
                currency: Some("EUR".to_string()),
                github_release: GithubReleaseConfig {
                    owner: "Sjors".to_string(),
                    repo: "nado-book".to_string(),
                    cover_asset_name: "nado-cover-rgb.pdf".to_string(),
                    book_asset_name: "nado-paperback.pdf".to_string(),
                },
                item: BookItemConfig {
                    product: "textbook_pb_digest_p_bw".to_string(),
                    title: Some("Bitcoin - A Work in Progress".to_string()),
                    price: None,
                    currency: None,
                    harmonized_code: None,
                    options: vec![],
                    files: vec![OrderFile {
                        file_type: "book".to_string(),
                        url: "https://example.com/book.pdf".to_string(),
                        md5sum: "abc".to_string(),
                        path: None,
                    }],
                },
            },
            address: Address {
                company: None,
                firstname: "Domkerk".to_string(),
                lastname: "Orders".to_string(),
                street1: "Achter de Dom 1".to_string(),
                street2: None,
                zip: "3512 JN".to_string(),
                city: "Utrecht".to_string(),
                country: "NL".to_string(),
                state: None,
                order_email: None,
                delivery_email: None,
                phone: None,
            },
            template: PathBuf::from("orders/btcwip-example.toml"),
            address_source: PathBuf::from("addresses/domkerk-utrecht.toml"),
            reference: "domkerk-21".to_string(),
            count: 21,
            release_selector: ReleaseSelector {
                version: None,
                latest: false,
            },
        };

        let err = prepared
            .submit_request("quote-domkerk-postal")
            .expect_err("should fail");
        assert!(
            err.to_string()
                .contains("submit requires address.order_email")
        );
    }

    #[test]
    fn release_selector_prefers_latest_when_requested() {
        let args = QuoteOrderArgs {
            template: PathBuf::from("orders/btcwip-example.toml"),
            address: PathBuf::from("address.toml"),
            reference: "ref".to_string(),
            count: 1,
            version: None,
            latest: true,
        };

        let selector = ReleaseSelector::from_quote_args(&args);
        assert!(matches!(
            selector.requested_release(),
            Some(ReleaseRequest::Latest)
        ));
    }

    #[test]
    fn formats_duplicate_reference_submit_error() {
        let request = AddOrderRequest {
            apikey: "test-key".to_string(),
            reference: "domkerk-21".to_string(),
            email: "orders@domkerk.example".to_string(),
            addresses: vec![],
            items: vec![],
        };

        let rendered = format_submit_error(
            reqwest::StatusCode::CONFLICT,
            &request,
            r#"{"error":{"type":"order_reference_not_unique","info":"The order reference is not unique"}}"#,
        );

        assert!(rendered.contains("order reference \"domkerk-21\" already exists"));
        assert!(rendered.contains("Use a new --reference value"));
    }

    #[test]
    fn release_selector_passes_through_version_tag() {
        let args = QuoteOrderArgs {
            template: PathBuf::from("orders/btcwip-example.toml"),
            address: PathBuf::from("address.toml"),
            reference: "ref".to_string(),
            count: 1,
            version: Some("v1.2.3".to_string()),
            latest: false,
        };

        let selector = ReleaseSelector::from_quote_args(&args);
        assert!(matches!(
            selector.requested_release(),
            Some(ReleaseRequest::Tag("v1.2.3"))
        ));
    }

    #[test]
    fn rejects_unexpected_address_fields() {
        let err = toml::from_str::<Address>(
            r#"
firstname = "Domkerk"
lastname = "Orders"
street1 = "Achter de Dom 1"
zip = "3512 JN"
city = "Utrecht"
country = "NL"
unexpected_field = "orders@domkerk.example"
"#,
        )
        .expect_err("unexpected address field should be rejected");

        assert!(err.to_string().contains("unknown field `unexpected_field`"));
    }

    #[test]
    fn formats_weight_in_kg_rounded_up_to_tenth() {
        assert_eq!(crate::output::format_weight_kg("7345").unwrap(), "7.4 kg");
    }
}
