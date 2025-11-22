use crate::Response;

pub fn json_from_response(response: &Response) -> String {
    serde_json::to_string(response).unwrap()
}
