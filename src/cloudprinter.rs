use reqwest::Method;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use anyhow::{Context, Result, bail};

use crate::config::{Address, ItemOption, OrderFile};

#[derive(Clone, Debug)]
pub(crate) struct CloudPrinterClient {
    api_key: String,
    base_url: String,
    http: Client,
}

impl CloudPrinterClient {
    pub(crate) fn new(api_key: String, base_url: String) -> Result<Self> {
        let http = Client::builder().build()?;
        Ok(Self {
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }

    pub(crate) fn http(&self) -> Client {
        self.http.clone()
    }

    pub(crate) fn list_products(&self) -> Result<Vec<ProductSummary>> {
        self.post_json(
            "products",
            &ApiKeyRequest {
                apikey: self.api_key.clone(),
            },
        )
    }

    pub(crate) fn product_info(&self, product: &str) -> Result<ProductInfo> {
        self.post_json(
            "products/info",
            &ProductInfoRequest {
                apikey: self.api_key.clone(),
                reference: product.to_string(),
            },
        )
    }

    pub(crate) fn quote(&self, request: &QuoteRequest) -> Result<QuoteResponse> {
        self.post_json("orders/quote", request)
    }

    pub(crate) fn submit(&self, request: &AddOrderRequest) -> Result<SubmitResponse> {
        let url = format!("{}/orders/add", self.base_url);
        let response = self
            .http
            .request(Method::POST, &url)
            .json(request)
            .send()
            .with_context(|| format!("request to {url} failed"))?;
        let status = response.status();
        let body_text = response.text()?;

        if !status.is_success() {
            bail!("{}", format_submit_error(status, request, &body_text));
        }

        let body = if body_text.trim().is_empty() {
            None
        } else {
            Some(serde_json::from_str::<Value>(&body_text).unwrap_or(Value::String(body_text)))
        };

        Ok(SubmitResponse {
            status: status.as_u16(),
            body,
        })
    }

    fn post_json<TReq, TResp>(&self, endpoint: &str, request: &TReq) -> Result<TResp>
    where
        TReq: Serialize,
        TResp: DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self
            .http
            .request(Method::POST, &url)
            .json(request)
            .send()
            .with_context(|| format!("request to {url} failed"))?;
        let status = response.status();
        let body_text = response.text()?;

        if !status.is_success() {
            let error_text = if body_text.is_empty() {
                "<empty response body>".to_string()
            } else {
                body_text.clone()
            };
            bail!(
                "CloudPrinter call to {endpoint} failed with HTTP {}: {}",
                status,
                error_text
            );
        }

        serde_json::from_str::<TResp>(&body_text)
            .with_context(|| format!("failed to decode CloudPrinter response from {endpoint}"))
    }
}

pub(crate) fn format_submit_error(
    status: reqwest::StatusCode,
    request: &AddOrderRequest,
    body_text: &str,
) -> String {
    if let Ok(error) = serde_json::from_str::<CloudPrinterErrorEnvelope>(body_text)
        && error.error.error_type == "order_reference_not_unique"
    {
        return format!(
            "CloudPrinter submit failed: order reference {:?} already exists. Use a new --reference value. CloudPrinter says: {}. Raw API error: HTTP {}: {}",
            request.reference, error.error.info, status, body_text
        );
    }

    let error_text = if body_text.is_empty() {
        "<empty response body>".to_string()
    } else {
        body_text.to_string()
    };
    format!(
        "CloudPrinter submit failed with HTTP {}: {}",
        status, error_text
    )
}

impl Address {
    pub(crate) fn into_delivery_address(self) -> DeliveryAddress {
        DeliveryAddress {
            address_type: "delivery".to_string(),
            company: self.company,
            firstname: self.firstname,
            lastname: self.lastname,
            street1: self.street1,
            street2: self.street2,
            zip: self.zip,
            city: self.city,
            state: self.state,
            country: self.country,
            email: self.delivery_email,
            phone: self.phone,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiKeyRequest {
    apikey: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProductInfoRequest {
    apikey: String,
    reference: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct QuoteRequest {
    pub(crate) apikey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) currency: Option<String>,
    pub(crate) country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) state: Option<String>,
    pub(crate) items: Vec<QuoteItemRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct QuoteItemRequest {
    pub(crate) reference: String,
    pub(crate) product: String,
    pub(crate) count: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) options: Vec<ItemOption>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AddOrderRequest {
    pub(crate) apikey: String,
    pub(crate) reference: String,
    pub(crate) email: String,
    pub(crate) addresses: Vec<DeliveryAddress>,
    pub(crate) items: Vec<AddOrderItemRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeliveryAddress {
    #[serde(rename = "type")]
    pub(crate) address_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) company: Option<String>,
    pub(crate) firstname: String,
    pub(crate) lastname: String,
    pub(crate) street1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) street2: Option<String>,
    pub(crate) zip: String,
    pub(crate) city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) state: Option<String>,
    pub(crate) country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phone: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AddOrderItemRequest {
    pub(crate) reference: String,
    pub(crate) product: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    pub(crate) count: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hc: Option<String>,
    pub(crate) files: Vec<OrderFile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) options: Vec<ItemOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProductSummary {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) note: Option<String>,
    pub(crate) reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProductInfo {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) note: Option<String>,
    pub(crate) reference: String,
    #[serde(default)]
    pub(crate) options: Vec<Value>,
    #[serde(default)]
    pub(crate) specs: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuoteResponse {
    pub(crate) price: String,
    pub(crate) vat: String,
    pub(crate) currency: String,
    pub(crate) invoice_currency: String,
    pub(crate) invoice_exchange_rate: String,
    pub(crate) expire_date: String,
    pub(crate) subtotals: QuoteSubtotals,
    pub(crate) shipments: Vec<QuoteShipment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuoteSubtotals {
    pub(crate) items: String,
    pub(crate) fee: String,
    pub(crate) app_fee: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuoteShipment {
    pub(crate) total_weight: String,
    pub(crate) items: Vec<QuoteShipmentItem>,
    pub(crate) quotes: Vec<ShippingQuote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuoteShipmentItem {
    pub(crate) reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ShippingQuote {
    pub(crate) quote: String,
    pub(crate) service: String,
    pub(crate) shipping_level: String,
    pub(crate) shipping_option: String,
    pub(crate) price: String,
    pub(crate) vat: String,
    pub(crate) currency: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubmitResponse {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct CloudPrinterErrorEnvelope {
    error: CloudPrinterError,
}

#[derive(Debug, Clone, Deserialize)]
struct CloudPrinterError {
    #[serde(rename = "type")]
    error_type: String,
    info: String,
}
