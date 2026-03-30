# cloud-print-rs

Small (unofficial) Rust CLI for [CloudPrinter](https://www.cloudprinter.com) workflows:

1. Request a quote, including shipping options.
2. Submit an order from a saved quote hash.

Copy `.env.example` to `.env`
and set `CLOUDPRINTER_API_KEY_LIVE` to your real CloudPrinter API key from the
CloudPrinter admin system. The usage examples below assume live mode. See
`## Development` for sandbox usage.

## Usage

In this example, the cover and book PDFs are uploaded as GitHub release assets.
The example order template points at the [v1.0.5](https://github.com/Sjors/nado-book/releases/tag/v1.0.5)
release of [Bitcoin: A Work in Progress](https://btcwip.com/),
and `--latest` tells the tool to resolve the PDF URLs and MD5 sums from that release automatically.

Request a quote:

```sh
cargo run --release -- quote \
  --template orders/btcwip-example.toml \
  --address addresses/domkerk-utrecht.toml \
  --reference domkerk-21 \
  --count 21 \
  --latest
```

Example quote output:

```text
Quote
  - reference: domkerk-21
  - expires: 2026-03-31 11:42:19 +02:00

Print details
  - Release tag: v1.0.5
  - Product: Textbook PB A5 P BW TNR (textbook_pb_a5_p_bw)
  - cover PDF: https://github.com/Sjors/nado-book/releases/download/v1.0.5/nado-cover-rgb-v1.0.5.pdf
  - book PDF: https://github.com/Sjors/nado-book/releases/download/v1.0.5/nado-paperback-v1.0.5.pdf
  - Total pages: 224
  - Pageblock paper: 90gsm Machine Coated Silk
  - Cover paper: 250gsm Gloss coated graphical board (250ECB)
  - Cover lamination: Matte finish

Order details
  - Quantity: 21
  - Domkerk (Domkerk Orders)
  - Achter de Dom 1, 3512 JN Utrecht, NL

+-----------------+----------+
| Product total   |   €70.80 |
| Fee             |    €0.00 |
| VAT             |    €0.00 |
+-----------------+----------+
| Per item ex VAT |    €3.37 |
+-----------------+----------+

Shipment group 1
  - total weight: 7.4 kg
Shipping option: DHL - Connect Europe (cp_saver)
Quote hash: a3a67f7383260b868f2b317572366d9ed7dd57f1e3c7a01e51d0095931af2a9a

+-------------------------------+----------+
| Shipping                      |    €7.01 |
| Per item ex VAT               |    €0.33 |
+-------------------------------+----------+
| Total ex VAT incl shipping    |   €77.81 |
| VAT                           |    €0.00 |
| Per item ex VAT incl shipping |    €3.71 |
+-------------------------------+----------+

To use this shipping option:
cargo run --release -- submit --quote-hash a3a67f7383260b868f2b317572366d9ed7dd57f1e3c7a01e51d0095931af2a9a
```

Submit the quote:

```sh
cargo run --release -- submit \
  --quote-hash <hash-from-quote>
```

## Notes

- CloudPrinter quotes are valid for 48 hours.
- `quote` stores one pending file per quote hash under `pending/`, including the chosen GitHub release source, so `submit --quote-hash ...` can reuse everything from the earlier quote.
- `quote --version <tag>` and `quote --latest` look up the release assets on GitHub and fail early if either PDF is missing.
- Without `--version` or `--latest`, submit falls back to the explicit file URLs in the config and still requires md5 sums.

## Development

- Set `CLOUDPRINTER_API_KEY_SANDBOX` in `.env` for sandbox development.
- Regular debug builds like `cargo run -- ...` use the sandbox key.
- Release builds like `cargo run --release -- ...` use `CLOUDPRINTER_API_KEY_LIVE`.
- The current CLI assumes each order contains a single CloudPrinter item.
- Before opening a pull request, run:
  `cargo fmt --check`
  `cargo clippy --all-targets --all-features -- -D warnings`
  `cargo test --locked`
