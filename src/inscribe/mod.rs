use tracing::instrument;

use crate::Response;

#[instrument(level = "trace", skip(response))]
pub fn json_from_response(response: &Response) -> String {
    serde_json::to_string(response).unwrap()
}
