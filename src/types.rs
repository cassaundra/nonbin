use serde::Serialize;

#[derive(Serialize)]
pub struct UploadPaste {
    pub id: String,
    pub url: String,
    pub delete_key: String,
}
