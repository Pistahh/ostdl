use std::collections::BTreeMap;

use xmlrpc::{Request, Value};

use crate::error::{Error, E_INV_RESP};

/// opensubtitles XML-RPC API entry point
pub(crate) const OST_API_URL: &str = "https://api.opensubtitles.org/xml-rpc";

/// To simplify definitions using the XML-RPC "struct" type
type OstDataMap = BTreeMap<String, Value>;

/// Converts an XML-RPC response into an OstDatamap
pub(crate) fn val_to_response(v: &Value) -> Result<&OstDataMap, Error> {
    let resp = v.as_struct().ok_or(E_INV_RESP)?;

    let status = resp
        .get("status")
        .and_then(Value::as_str)
        .ok_or(E_INV_RESP)?;

    if status.starts_with("200") {
        Ok(resp)
    } else {
        Err(Error::Ost(
            format!("xmlrpc request failed: {}", status).into(),
        ))
    }
}

/// Creates the body of the search request
pub(crate) fn make_req(lang: &str, size: u64, hash: u64) -> Value {
    let mut m = BTreeMap::new();
    m.insert("sublanguageid".into(), Value::String(lang.to_string()));
    m.insert("moviehash".into(), Value::String(format!("{:x}", hash)));
    m.insert("moviebytesize".into(), Value::String(size.to_string()));

    Value::Struct(m)
}

/// logs into OpenSubtitles API and returns the access token
pub(crate) fn get_token() -> Result<String, Error> {
    let resp = Request::new("LogIn")
        .arg("")
        .arg("")
        .arg("en")
        .arg("opensubtitles-download 1.0")
        .call_url(OST_API_URL)?;

    val_to_response(&resp)?
        .get("token")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or(E_INV_RESP)
}
