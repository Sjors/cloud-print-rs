use std::fs;

use predicates::prelude::*;
use serde_json::json;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn quote_command_uses_domkerk_fixture_shape() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/orders/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "price": "210.0000",
            "vat": "0.0000",
            "currency": "EUR",
            "invoice_currency": "EUR",
            "invoice_exchange_rate": "1.0000",
            "expire_date": "2026-03-31T12:00:00+00:00",
            "subtotals": {
                "items": "205.0000",
                "fee": "5.0000",
                "app_fee": "0.0000"
            },
            "shipments": [{
                "total_weight": "14700",
                "items": [{"reference": "domkerk-21-1"}],
                "quotes": [{
                    "quote": "quote-domkerk-postal",
                    "service": "Postal - Untracked",
                    "shipping_level": "cp_postal",
                    "shipping_option": "National Post - Int. Postal Untracked",
                    "price": "12.5000",
                    "vat": "0.0000",
                    "currency": "EUR"
                }]
            }]
        })))
        .mount(&server)
        .await;

    let fixture_dir = tempdir()?;
    let config_path = fixture_dir.path().join("order.toml");
    let address_path = fixture_dir.path().join("domkerk.toml");
    let pending_path = fixture_dir.path().join("pending/quote-domkerk-postal.toml");

    fs::write(
        &config_path,
        format!(
            r#"
api_base_url = "{base_url}"
currency = "EUR"

[github_release]
owner = "Sjors"
repo = "nado-book"
cover_asset_name = "nado-cover-rgb-v1.0.5.pdf"
book_asset_name = "nado-paperback-v1.0.5.pdf"

[item]
product = "textbook_pb_a5_p_bw"
title = "Bitcoin - A Work in Progress"

[[item.files]]
type = "cover"
url = "https://example.com/nado-cover-rgb-v1.0.5.pdf"
md5sum = "covermd5"

[[item.files]]
type = "book"
url = "https://example.com/nado-paperback-v1.0.5.pdf"
md5sum = "bookmd5"

[[item.options]]
type = "total_pages"
count = "224"

[[item.options]]
type = "pageblock_90mcs"
count = "224"

[[item.options]]
type = "cover_250ecb"
count = "1"

[[item.options]]
type = "cover_finish_matte"
count = "1"
"#,
            base_url = server.uri()
        ),
    )?;

    fs::write(
        &address_path,
        r#"
company = "Domkerk"
firstname = "Domkerk"
lastname = "Orders"
street1 = "Achter de Dom 1"
zip = "3512 JN"
city = "Utrecht"
country = "NL"
order_email = "orders@domkerk.example"
delivery_email = "delivery@domkerk.example"
"#,
    )?;

    let mut command = assert_cmd::Command::cargo_bin("cloud-print-rs")?;
    command
        .current_dir(fixture_dir.path())
        .env("CLOUDPRINTER_API_KEY_SANDBOX", "test-key")
        .env("TZ", "Europe/Amsterdam")
        .arg("quote")
        .arg("--template")
        .arg(&config_path)
        .arg("--address")
        .arg(&address_path)
        .arg("--reference")
        .arg("domkerk-21")
        .arg("--count")
        .arg("21");

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("Quote\n  - reference: domkerk-21"))
        .stdout(predicate::str::contains(
            "  - expires: 2026-03-31 14:00:00 +02:00",
        ))
        .stdout(predicate::str::contains("Print details"))
        .stdout(predicate::str::contains(
            "  - Product: Textbook PB A5 P BW TNR (textbook_pb_a5_p_bw)",
        ))
        .stdout(predicate::str::contains(
            "  - cover PDF: https://example.com/nado-cover-rgb-v1.0.5.pdf",
        ))
        .stdout(predicate::str::contains(
            "  - book PDF: https://example.com/nado-paperback-v1.0.5.pdf",
        ))
        .stdout(predicate::str::contains("Order details"))
        .stdout(predicate::str::contains("  - Quantity: 21"))
        .stdout(predicate::str::contains("  - Domkerk (Domkerk Orders)"))
        .stdout(predicate::str::contains(
            "  - Achter de Dom 1, 3512 JN Utrecht, NL",
        ))
        .stdout(predicate::str::contains("\n+-----------------+-----------+"))
        .stdout(predicate::str::contains("| Product total   |   €210.00 |"))
        .stdout(predicate::str::contains("| Fee             |     €5.00 |"))
        .stdout(predicate::str::contains("| VAT             |     €0.00 |"))
        .stdout(predicate::str::contains("| Per item ex VAT |    €10.24 |"))
        .stdout(predicate::str::contains(
            "Shipment group 1\n  - total weight: 14.7 kg",
        ))
        .stdout(predicate::str::contains(
            "Shipping option: National Post - Int. Postal Untracked (cp_postal)",
        ))
        .stdout(predicate::str::contains("Quote hash: quote-domkerk-postal\n\n+"))
        .stdout(predicate::str::contains(
            "| Shipping                      |    €12.50 |",
        ))
        .stdout(predicate::str::contains(
            "| Per item ex VAT               |     €0.60 |",
        ))
        .stdout(predicate::str::contains(
            "| Total ex VAT incl shipping    |   €227.50 |",
        ))
        .stdout(predicate::str::contains(
            "+-------------------------------+-----------+\n| Total ex VAT incl shipping    |   €227.50 |",
        ))
        .stdout(predicate::str::contains(
            "| VAT                           |     €0.00 |",
        ))
        .stdout(predicate::str::contains(
            "| Per item ex VAT incl shipping |    €10.83 |",
        ))
        .stdout(predicate::str::contains(
            "\nTo use this shipping option:\ncargo run -- submit --quote-hash quote-domkerk-postal",
        ));

    assert!(pending_path.exists());

    Ok(())
}

#[test]
fn submit_dry_run_embeds_quote_hash() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = tempdir()?;
    let config_path = fixture_dir.path().join("order.toml");
    let address_path = fixture_dir.path().join("domkerk.toml");
    let pending_dir = fixture_dir.path().join("pending");
    fs::create_dir_all(&pending_dir)?;
    let pending_path = pending_dir.join("quote-domkerk-postal.toml");

    fs::write(
        &config_path,
        r#"
[github_release]
owner = "Sjors"
repo = "nado-book"
cover_asset_name = "nado-cover-rgb-v1.0.5.pdf"
book_asset_name = "nado-paperback-v1.0.5.pdf"

[item]
product = "textbook_pb_a5_p_bw"
title = "Bitcoin - A Work in Progress"

[[item.files]]
type = "cover"
url = "https://example.com/nado-cover.pdf"
md5sum = "covermd5"

[[item.files]]
type = "book"
url = "https://example.com/nado-interior.pdf"
md5sum = "bookmd5"
"#,
    )?;

    fs::write(
        &address_path,
        r#"
company = "Domkerk"
firstname = "Domkerk"
lastname = "Orders"
street1 = "Achter de Dom 1"
zip = "3512 JN"
city = "Utrecht"
country = "NL"
order_email = "orders@domkerk.example"
delivery_email = "delivery@domkerk.example"
"#,
    )?;

    fs::write(
        &pending_path,
        format!(
            r#"
template = "{}"
address = "{}"
reference = "domkerk-21"
count = 21
product = "textbook_pb_a5_p_bw"
quote_hash = "quote-domkerk-postal"
shipping_level = "cp_postal"
shipping_option = "National Post - Int. Postal Untracked"
shipping_price = "12.5000"
currency = "EUR"

[release_selector]
latest = false
"#,
            config_path.display(),
            address_path.display()
        ),
    )?;

    let mut command = assert_cmd::Command::cargo_bin("cloud-print-rs")?;
    command
        .current_dir(fixture_dir.path())
        .env("CLOUDPRINTER_API_KEY_SANDBOX", "test-key")
        .env("TZ", "Europe/Amsterdam")
        .arg("submit")
        .arg("--quote-hash")
        .arg("quote-domkerk-postal")
        .arg("--dry-run")
        .arg("--json");

    command
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"quote\": \"quote-domkerk-postal\"",
        ))
        .stdout(predicate::str::contains(
            "\"email\": \"orders@domkerk.example\"",
        ))
        .stdout(predicate::str::contains(
            "\"email\": \"delivery@domkerk.example\"",
        ));

    Ok(())
}
