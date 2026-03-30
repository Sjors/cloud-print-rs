use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::Serialize;

use crate::cargo_run_prefix;
use crate::cloudprinter::{QuoteResponse, SubmitResponse};
use crate::config::{Address, ItemOption, OrderFile};
use crate::github::ResolvedSubmitFiles;
use crate::order::PreparedOrder;

pub(crate) fn print_output<T: Serialize>(value: &T, json: bool) -> Result<()> {
    let _ = json;
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub(crate) fn print_submit_response(response: &SubmitResponse, json: bool) -> Result<()> {
    if json {
        return print_output(response, true);
    }

    println!("Order accepted with HTTP {}", response.status);
    if let Some(body) = &response.body {
        println!("{}", serde_json::to_string_pretty(body)?);
    }

    Ok(())
}

pub(crate) fn print_quote_response(
    order: &PreparedOrder,
    resolved_files: &ResolvedSubmitFiles,
    response: &QuoteResponse,
    json: bool,
) -> Result<()> {
    if json {
        return print_output(response, true);
    }

    let product_total = parse_money(&response.price)?;
    let fee = parse_money(&response.subtotals.fee)?;
    let vat = parse_money(&response.vat)?;
    let order_per_item_ex_vat = (product_total + fee) / order.count as f64;

    println!("Quote");
    println!("  - reference: {}", order.reference);
    println!(
        "  - expires: {}",
        format_local_timestamp(&response.expire_date)?
    );
    println!();

    println!("Print details");
    if let Some(tag) = &resolved_files.release_tag {
        println!("  - Release tag: {tag}");
    }
    println!(
        "  - Product: {}",
        format_product(&order.book.item.product)
    );
    for file in &resolved_files.files {
        println!(
            "  - {}: {}",
            format_file_label(&file.file_type),
            format_file_source(file)?
        );
    }
    for option in &order.book.item.options {
        println!("  - {}", format_option_detail(option));
    }
    println!();

    println!("Order details");
    println!("  - Quantity: {}", order.count);
    println!("  - {}", format_destination_contact(&order.address));
    println!("  - {}", format_destination_address(&order.address));
    println!();
    print_amounts_table(
        &response.currency,
        &[
            ("Product total", product_total),
            ("Fee", fee),
            ("VAT", vat),
            ("Per item ex VAT", order_per_item_ex_vat),
        ],
        Some("Per item ex VAT"),
    );

    for (shipment_index, shipment) in response.shipments.iter().enumerate() {
        let total_weight_kg = format_weight_kg(&shipment.total_weight)?;
        println!();
        println!("Shipment group {}", shipment_index + 1);
        println!("  - total weight: {}", total_weight_kg);

        for quote in &shipment.quotes {
            let shipping = parse_money(&quote.price)?;
            let shipping_vat = parse_money(&quote.vat)?;
            let total_ex_vat = product_total + fee + shipping;
            let per_item_ex_vat = total_ex_vat / order.count as f64;
            let shipping_per_item_ex_vat = shipping / order.count as f64;
            println!(
                "Shipping option: {} ({})",
                quote.shipping_option, quote.shipping_level
            );
            println!("Quote hash: {}", quote.quote);
            println!();
            print_amounts_table(
                &quote.currency,
                &[
                    ("Shipping", shipping),
                    ("Per item ex VAT", shipping_per_item_ex_vat),
                    ("Total ex VAT incl shipping", total_ex_vat),
                    ("VAT", shipping_vat),
                    ("Per item ex VAT incl shipping", per_item_ex_vat),
                ],
                Some("Total ex VAT incl shipping"),
            );
            println!();
            println!("To use this shipping option:");
            println!("{} submit --quote-hash {}", cargo_run_prefix(), quote.quote);
        }
        println!();
    }

    Ok(())
}

fn print_amounts_table(currency: &str, rows: &[(&str, f64)], divider_before: Option<&str>) {
    let symbol = currency_symbol(currency);
    let rendered_rows = rows
        .iter()
        .map(|(label, value)| {
            (
                *label,
                format!("{symbol}{}", format_money(*value, currency)),
            )
        })
        .collect::<Vec<_>>();
    let amount_width = rendered_rows
        .iter()
        .map(|(_, value)| value.len())
        .max()
        .unwrap_or(0);
    let label_width = rendered_rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0);
    let border = format!(
        "+-{}-+-{}-+",
        "-".repeat(label_width),
        "-".repeat(amount_width)
    );
    let total_index = rows
        .iter()
        .position(|(label, _)| Some(*label) == divider_before);

    println!("{border}");
    for (index, (label, value)) in rendered_rows.iter().enumerate() {
        if Some(index) == total_index {
            println!("{border}");
        }
        println!("| {:<label_width$} | {:>amount_width$} |", label, value);
    }
    println!("{border}");
}

fn parse_money(value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .with_context(|| format!("failed to parse money amount {value}"))
}

fn format_local_timestamp(timestamp: &str) -> Result<String> {
    let parsed = DateTime::parse_from_rfc3339(timestamp)
        .with_context(|| format!("failed to parse timestamp {timestamp}"))?;
    Ok(parsed
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string())
}

pub(crate) fn format_weight_kg(grams: &str) -> Result<String> {
    let grams = grams
        .parse::<u64>()
        .with_context(|| format!("failed to parse weight in grams: {grams}"))?;
    let tenths_kg = grams.div_ceil(100);
    Ok(format!("{}.{} kg", tenths_kg / 10, tenths_kg % 10))
}

fn format_file_label(file_type: &str) -> &str {
    match file_type {
        "cover" => "cover PDF",
        "book" => "book PDF",
        _ => file_type,
    }
}

fn format_file_source(file: &OrderFile) -> Result<String> {
    if let Some(path) = &file.path {
        return Ok(crate::config::absolutize(path)?.display().to_string());
    }

    Ok(file.url.clone())
}

fn format_destination_contact(address: &Address) -> String {
    match &address.company {
        Some(company) => format!("{company} ({})", format_person_name(address)),
        None => format_person_name(address),
    }
}

fn format_person_name(address: &Address) -> String {
    format!("{} {}", address.firstname, address.lastname)
}

fn format_destination_address(address: &Address) -> String {
    let mut parts = vec![address.street1.clone()];
    if let Some(street2) = &address.street2
        && !street2.trim().is_empty()
    {
        parts.push(street2.clone());
    }
    parts.push(format!("{} {}", address.zip, address.city));
    parts.push(address.country.clone());
    parts.join(", ")
}

fn format_option_detail(option: &ItemOption) -> String {
    match option.option_type.as_str() {
        "total_pages" => format!("Total pages: {}", option.count),
        "pageblock_90mcs" => "Pageblock paper: 90gsm Machine Coated Silk".to_string(),
        "cover_250ecb" => "Cover paper: 250gsm Gloss coated graphical board (250ECB)".to_string(),
        "cover_finish_matte" => "Cover lamination: Matte finish".to_string(),
        _ => format!("{} x {}", option.option_type, option.count),
    }
}

fn format_product(product: &str) -> String {
    let name = match product {
        "textbook_pb_digest_p_bw" => Some("Textbook PB Digest P BW TNR"),
        "textbook_pb_a5_p_bw" => Some("Textbook PB A5 P BW TNR"),
        _ => None,
    };

    match name {
        Some(name) => format!("{name} ({product})"),
        None => product.to_string(),
    }
}

fn format_money(value: f64, currency: &str) -> String {
    let precision = currency_fraction_digits(currency);
    format!("{value:.precision$}")
}

fn currency_fraction_digits(currency: &str) -> usize {
    match currency {
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        _ => 2,
    }
}

fn currency_symbol(currency: &str) -> &'static str {
    match currency {
        "EUR" => "€",
        "USD" => "$",
        "GBP" => "£",
        "JPY" => "¥",
        _ => "",
    }
}
